use std::fs;
use std::io;
use std::path::PathBuf;

use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use caliber::app::{
    App, ConfirmContext, DAILY_HEADER_LINES, DATE_SUFFIX_WIDTH, EditContext, FILTER_HEADER_LINES,
    InputMode, InterfaceContext, PromptContext, ViewMode,
};
use caliber::config::{self, Config, resolve_path};
use caliber::cursor::cursor_position_in_wrap;
use caliber::storage::{JournalContext, JournalSlot, Line};
use caliber::registry::{KeyActionId, KeyContext};
use caliber::ui::{CursorContext, ensure_selected_visible, format_key_for_display, set_edit_cursor};
use caliber::{handlers, storage, ui};

fn main() -> Result<(), io::Error> {
    let args: Vec<String> = std::env::args().collect();

    if args.get(1).map(String::as_str) == Some("init") {
        return init_config();
    }

    let cli_file = args.get(1).map(PathBuf::from);
    let (project_path, active_slot) = if let Some(path) = cli_file.clone() {
        // CLI path overrides project slot
        (
            Some(resolve_path(&path.to_string_lossy())),
            JournalSlot::Project,
        )
    } else if let Some(path) = storage::detect_project_journal() {
        (Some(path), JournalSlot::Project)
    } else {
        (None, JournalSlot::Hub)
    };

    let config = match active_slot {
        JournalSlot::Hub => Config::load_hub().unwrap_or_default(),
        JournalSlot::Project => Config::load_merged().unwrap_or_default(),
    };

    let hub_path = config.get_hub_journal_path();

    let journal_context = JournalContext::new(hub_path, project_path.clone(), active_slot);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, config, journal_context);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {err}");
    }

    Ok(())
}

fn init_config() -> io::Result<()> {
    match Config::init() {
        Ok(true) => {
            println!(
                "Created config file at: {}",
                config::get_config_path().display()
            );
            Ok(())
        }
        Ok(false) => {
            println!(
                "Config file already exists at: {}",
                config::get_config_path().display()
            );
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to create config file: {e}");
            Err(e)
        }
    }
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    config: Config,
    journal_context: JournalContext,
) -> io::Result<()> {
    let date = chrono::Local::now().date_naive();
    let mut app = App::new_with_context(config, date, journal_context)?;

    // Project initialization flow for git repositories
    if app.in_git_repo
        && let Some(git_root) = storage::find_git_root()
    {
        let caliber_dir = git_root.join(".caliber");

        // Auto-register existing projects
        if caliber_dir.exists() {
            let mut registry = storage::ProjectRegistry::load();
            if registry.find_by_path(&caliber_dir).is_none() {
                let _ = registry.register(caliber_dir.clone());
                let _ = registry.save();
            }
        }

        // Auto-init new projects if enabled
        if app.journal_context.project_path().is_none() && app.config.auto_init_project {
            fs::create_dir_all(&caliber_dir)?;

            let journal_path = app.config.get_project_journal_path(&git_root);
            if !journal_path.exists() {
                // Create parent directories if journal is at custom location
                if let Some(parent) = journal_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&journal_path, "")?;
            }

            let mut registry = storage::ProjectRegistry::load();
            if registry.find_by_path(&caliber_dir).is_none() {
                let _ = registry.register(caliber_dir.clone());
                let _ = registry.save();
            }

            app.journal_context.set_project_path(journal_path);
            app.switch_to_project()?;
            app.set_status("Project initialized");
        }
    }

    loop {
        // Check if we need a full terminal redraw (e.g., after returning from external editor)
        if app.needs_redraw {
            terminal.clear()?;
            app.needs_redraw = false;
        }

        let is_filter_context = matches!(app.view, ViewMode::Filter(_))
            || matches!(
                app.input_mode,
                InputMode::Edit(EditContext::FilterEdit { .. })
                    | InputMode::Edit(EditContext::FilterQuickAdd { .. })
            );

        terminal.draw(|f| {
            let size = f.area();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(1)])
                .split(size);

            let main_block = if is_filter_context {
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta))
            } else {
                let date_title = app.current_date.format(" %m/%d/%y ").to_string();
                Block::default()
                    .title_top(ratatui::text::Line::from(date_title).alignment(Alignment::Right))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White))
            };

            let inner = main_block.inner(chunks[0]);

            let content_area = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(inner)[1];

            f.render_widget(main_block, chunks[0]);

            let visible_height = content_area.height as usize;
            let scroll_height = visible_height.saturating_sub(ui::HINT_OVERLAY_HEIGHT as usize - 1);
            let content_width = content_area.width as usize;

            let filter_visual_line = app.filter_visual_line();
            let filter_total_lines = app.filter_total_lines();
            let visible_entry_count = app.visible_entry_count();

            match &mut app.view {
                ViewMode::Filter(state) => {
                    ensure_selected_visible(
                        &mut state.scroll_offset,
                        filter_visual_line,
                        filter_total_lines,
                        scroll_height,
                    );
                    if state.selected == 0 {
                        state.scroll_offset = 0;
                    }
                }
                ViewMode::Daily(state) => {
                    ensure_selected_visible(
                        &mut state.scroll_offset,
                        state.selected + DAILY_HEADER_LINES,
                        visible_entry_count + DAILY_HEADER_LINES,
                        scroll_height,
                    );
                    if state.selected == 0 {
                        state.scroll_offset = 0;
                    }
                }
            }

            let lines = if is_filter_context {
                ui::render_filter_view(&app, content_width)
            } else {
                ui::render_daily_view(&app, content_width)
            };

            if let InputMode::Edit(ref ctx) = app.input_mode
                && let Some(ref buffer) = app.edit_buffer
            {
                let cursor_ctx = match ctx {
                    EditContext::FilterQuickAdd { entry_type, .. } => {
                        let ViewMode::Filter(state) = &app.view else {
                            unreachable!()
                        };
                        let prefix_width = entry_type.prefix().len();
                        let text_width = content_width.saturating_sub(prefix_width);
                        let (cursor_row, cursor_col) = cursor_position_in_wrap(
                            buffer.content(),
                            buffer.cursor_display_pos(),
                            text_width,
                        );
                        Some(CursorContext {
                            prefix_width,
                            cursor_row,
                            cursor_col,
                            entry_start_line: state.entries.len() + FILTER_HEADER_LINES,
                        })
                    }
                    EditContext::FilterEdit { filter_index, .. } => {
                        let ViewMode::Filter(state) = &app.view else {
                            unreachable!()
                        };
                        state.entries.get(*filter_index).map(|filter_entry| {
                            let prefix_width = filter_entry.entry_type.prefix().len();
                            let text_width =
                                content_width.saturating_sub(prefix_width + DATE_SUFFIX_WIDTH);
                            let (cursor_row, cursor_col) = cursor_position_in_wrap(
                                buffer.content(),
                                buffer.cursor_display_pos(),
                                text_width,
                            );
                            CursorContext {
                                prefix_width,
                                cursor_row,
                                cursor_col,
                                entry_start_line: *filter_index + FILTER_HEADER_LINES,
                            }
                        })
                    }
                    EditContext::Daily { entry_index } => app
                        .entry_indices
                        .get(*entry_index)
                        .and_then(|&i| {
                            if let Line::Entry(entry) = &app.lines[i] {
                                Some(&entry.entry_type)
                            } else {
                                None
                            }
                        })
                        .map(|entry_type| {
                            let prefix_width = entry_type.prefix().width();
                            let text_width = content_width.saturating_sub(prefix_width);
                            let (cursor_row, cursor_col) = cursor_position_in_wrap(
                                buffer.content(),
                                buffer.cursor_display_pos(),
                                text_width,
                            );
                            CursorContext {
                                prefix_width,
                                cursor_row,
                                cursor_col,
                                entry_start_line: app.visible_projected_count()
                                    + app.visible_entries_before(*entry_index)
                                    + DAILY_HEADER_LINES,
                            }
                        }),
                };

                if let Some(ctx) = cursor_ctx {
                    set_edit_cursor(
                        f,
                        &ctx,
                        app.scroll_offset_mut(),
                        scroll_height,
                        content_area,
                    );
                }
            }

            #[allow(clippy::cast_possible_truncation)]
            let content = Paragraph::new(lines.clone()).scroll((app.scroll_offset() as u16, 0));
            f.render_widget(content, content_area);

            let total_lines = lines.len();
            let scroll_offset = app.scroll_offset();
            let can_scroll_up = scroll_offset > 0;
            let can_scroll_down = scroll_offset + visible_height < total_lines;

            if can_scroll_up || can_scroll_down {
                let arrows = match (can_scroll_up, can_scroll_down) {
                    (true, true) => " ▲▼ scroll ",
                    (true, false) => " ▲ scroll ",
                    (false, true) => " ▼ scroll ",
                    (false, false) => "",
                };
                let indicator_width = arrows.width() as u16;
                let indicator_area = ratatui::layout::Rect {
                    x: chunks[0].x + chunks[0].width.saturating_sub(indicator_width + 1),
                    y: chunks[0].y + chunks[0].height.saturating_sub(1),
                    width: indicator_width,
                    height: 1,
                };
                let scroll_indicator = Paragraph::new(Span::styled(arrows, Style::default().dim()));
                f.render_widget(scroll_indicator, indicator_area);
            }

            if let Some(ref msg) = app.status_message {
                let msg_width = msg.len() as u16 + 2;
                let status_area = ratatui::layout::Rect {
                    x: content_area.x,
                    y: content_area.y + content_area.height.saturating_sub(1),
                    width: msg_width.min(content_area.width),
                    height: 1,
                };
                let status = Paragraph::new(ratatui::text::Span::styled(
                    format!(" {msg} "),
                    Style::default().fg(Color::Black).bg(Color::Yellow),
                ));
                f.render_widget(status, status_area);
            }

            let footer = Paragraph::new(ui::render_footer(&app));
            f.render_widget(footer, chunks[1]);

            ui::render_hint_overlay(f, &app.hint_state, chunks[1]);

            let (indicator, indicator_color) = match app.active_journal() {
                JournalSlot::Hub => ("[HUB]".to_string(), Color::Green),
                JournalSlot::Project => {
                    let id = app
                        .current_project_id()
                        .map(|id| format!("[{}]", id.to_uppercase()))
                        .unwrap_or_else(|| "[PROJECT]".to_string());
                    (id, Color::Blue)
                }
            };
            let indicator_width = indicator.len() as u16;
            let indicator_area = ratatui::layout::Rect {
                x: chunks[1].x + chunks[1].width.saturating_sub(indicator_width),
                y: chunks[1].y,
                width: indicator_width,
                height: 1,
            };
            let indicator_widget = Paragraph::new(Span::styled(
                indicator,
                Style::default().fg(indicator_color),
            ));
            f.render_widget(indicator_widget, indicator_area);

            if let InputMode::Prompt(ref ctx) = app.input_mode {
                let prefix_width = 1;
                let cursor_pos = match ctx {
                    PromptContext::Command { buffer } => buffer.cursor_display_pos(),
                    PromptContext::Filter { buffer } => buffer.cursor_display_pos(),
                };
                let cursor_x = chunks[1].x + prefix_width + cursor_pos as u16;
                let cursor_y = chunks[1].y;
                f.set_cursor_position((cursor_x, cursor_y));
            }

            if app.show_help {
                let popup_area = ui::centered_rect(75, 70, size);
                app.help_visible_height = popup_area.height.saturating_sub(3) as usize;

                f.render_widget(Clear, popup_area);

                let total = ui::get_help_total_lines(&app.keymap);
                let max_scroll = total.saturating_sub(app.help_visible_height);
                let can_scroll_up = app.help_scroll > 0;
                let can_scroll_down = app.help_scroll < max_scroll;

                let arrows = match (can_scroll_up, can_scroll_down) {
                    (true, true) => "▲▼",
                    (true, false) => "▲",
                    (false, true) => "▼",
                    (false, false) => "",
                };

                let help_block = Block::default()
                    .title(" Keybindings ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan));

                let inner_area = help_block.inner(popup_area);
                f.render_widget(help_block, popup_area);

                let help_content =
                    ui::render_help_content(&app.keymap, app.help_scroll, app.help_visible_height);
                let help_paragraph = Paragraph::new(help_content);
                f.render_widget(help_paragraph, inner_area);

                let footer_area = ratatui::layout::Rect {
                    x: inner_area.x,
                    y: inner_area.y + inner_area.height.saturating_sub(1),
                    width: inner_area.width,
                    height: 1,
                };

                let close_keys = app.keymap.keys_for_action_ordered(KeyContext::Help, KeyActionId::ToggleHelp);
                let close_key = close_keys
                    .first()
                    .map(|k| format_key_for_display(k))
                    .unwrap_or_else(|| "?".to_string());

                let footer_line = if arrows.is_empty() {
                    ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(close_key.clone(), Style::default().fg(Color::White)),
                        ratatui::text::Span::styled(" close ", Style::default().dim()),
                    ])
                } else {
                    ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(arrows, Style::default().fg(Color::White)),
                        ratatui::text::Span::styled(" scroll  ", Style::default().dim()),
                        ratatui::text::Span::styled(close_key, Style::default().fg(Color::White)),
                        ratatui::text::Span::styled(" close ", Style::default().dim()),
                    ])
                };
                let footer =
                    Paragraph::new(footer_line).alignment(ratatui::layout::Alignment::Right);
                f.render_widget(footer, footer_area);
            }

            if let InputMode::Confirm(context) = &app.input_mode {
                let (title, messages): (&str, &[&str]) = match context {
                    ConfirmContext::CreateProjectJournal => (
                        " Create Project Journal ",
                        &["No project journal found.", "Create .caliber/journal.md?"],
                    ),
                };

                let popup_area = ui::centered_rect(50, 30, size);
                f.render_widget(Clear, popup_area);

                let confirm_block = Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue));

                let inner_area = confirm_block.inner(popup_area);
                f.render_widget(confirm_block, popup_area);

                let mut lines = vec![ratatui::text::Line::raw("")];
                for msg in messages {
                    lines.push(ratatui::text::Line::raw(*msg));
                }
                lines.push(ratatui::text::Line::raw(""));
                lines.push(ratatui::text::Line::from(vec![
                    Span::styled("[Y]", Style::default().fg(Color::Green)),
                    Span::raw(" Yes    "),
                    Span::styled("[N]", Style::default().fg(Color::Red)),
                    Span::raw(" No"),
                ]));
                let content = ratatui::text::Text::from(lines);
                let paragraph = Paragraph::new(content).alignment(Alignment::Center);
                f.render_widget(paragraph, inner_area);
            }

            let current_project_id = app.current_project_id();
            if let InputMode::Interface(ref mut ctx) = app.input_mode {
                match ctx {
                    InterfaceContext::Date(state) => ui::render_date_interface(f, state, size),
                    InterfaceContext::Project(state) => {
                        let visible_height =
                            (ui::POPUP_HEIGHT.saturating_sub(4) as usize).min(size.height as usize);
                        ensure_selected_visible(
                            &mut state.scroll_offset,
                            state.selected,
                            state.filtered_indices.len(),
                            visible_height,
                        );
                        ui::render_project_interface(f, state, size, current_project_id.as_deref());
                    }
                    InterfaceContext::Tag(_) => {}
                }
            }
        })?;

        if event::poll(std::time::Duration::from_millis(16))?
            && let Event::Key(key) = event::read()?
        {
            app.status_message = None;

            if app.show_help {
                handlers::handle_help_key(&mut app, key);
            } else {
                match &app.input_mode {
                    InputMode::Prompt(_) => handlers::handle_prompt_key(&mut app, key)?,
                    InputMode::Normal => handlers::handle_normal_key(&mut app, key)?,
                    InputMode::Edit(_) => handlers::handle_edit_key(&mut app, key),
                    InputMode::Reorder => handlers::handle_reorder_key(&mut app, key),
                    InputMode::Confirm(_) => handlers::handle_confirm_key(&mut app, key.code)?,
                    InputMode::Selection(_) => handlers::handle_selection_key(&mut app, key)?,
                    InputMode::Interface(_) => handlers::handle_interface_key(&mut app, key)?,
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
