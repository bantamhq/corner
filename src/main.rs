use std::io;
use std::path::PathBuf;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use caliber::app::{
    App, ConfirmContext, DAILY_HEADER_LINES, DATE_SUFFIX_WIDTH, EditContext, FILTER_HEADER_LINES,
    InputMode, ViewMode,
};
use caliber::config::{self, Config, resolve_path};
use caliber::cursor::cursor_position_in_wrap;
use caliber::storage::{JournalContext, JournalSlot, Line};
use caliber::ui::{CursorContext, ensure_selected_visible, set_edit_cursor};
use caliber::{handlers, storage, ui};

fn main() -> Result<(), io::Error> {
    let args: Vec<String> = std::env::args().collect();

    if args.get(1).map(String::as_str) == Some("init") {
        return match Config::init() {
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
        };
    }

    let cli_file = args.get(1).map(PathBuf::from);
    let config = Config::load().unwrap_or_default();

    let global_path = config.get_global_journal_path();
    let (project_path, active_slot) = if let Some(path) = cli_file {
        // CLI path overrides project slot
        (
            Some(resolve_path(&path.to_string_lossy())),
            JournalSlot::Project,
        )
    } else if let Some(path) = storage::detect_project_journal() {
        // Existing project journal detected
        (Some(path), JournalSlot::Project)
    } else {
        // No project journal, start in Global
        (None, JournalSlot::Global)
    };

    let journal_context = JournalContext::new(global_path, project_path.clone(), active_slot);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, config, journal_context);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {err}");
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    config: Config,
    journal_context: JournalContext,
) -> io::Result<()> {
    let date = chrono::Local::now().date_naive();
    let mut app = App::new_with_context(config, date, journal_context)?;

    // If in git repo without project journal, prompt to create one
    if app.in_git_repo && app.journal_context.project_path().is_none() {
        app.input_mode = InputMode::Confirm(ConfirmContext::CreateProjectJournal);
    }

    loop {
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
                                entry_start_line: app.visible_later_count()
                                    + app.visible_entries_before(*entry_index)
                                    + DAILY_HEADER_LINES,
                            }
                        }),
                    EditContext::LaterEdit { later_index, .. } => {
                        let ViewMode::Daily(state) = &app.view else {
                            unreachable!()
                        };
                        state.later_entries.get(*later_index).map(|later_entry| {
                            let prefix_width = later_entry.entry_type.prefix().width();
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
                                entry_start_line: app.visible_later_before(*later_index)
                                    + DAILY_HEADER_LINES,
                            }
                        })
                    }
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
            let content = Paragraph::new(lines).scroll((app.scroll_offset() as u16, 0));
            f.render_widget(content, content_area);

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

            // Render hint overlay above footer (if active)
            ui::render_hint_overlay(f, &app.hint_state, chunks[1]);

            let (indicator, indicator_color) = match app.active_journal() {
                JournalSlot::Global => ("[GLOBAL]", Color::Green),
                JournalSlot::Project => ("[PROJECT]", Color::Blue),
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

            match &app.input_mode {
                InputMode::Command => {
                    let prefix_width = 1;
                    let cursor_x =
                        chunks[1].x + prefix_width + app.command_buffer.cursor_display_pos() as u16;
                    let cursor_y = chunks[1].y;
                    f.set_cursor_position((cursor_x, cursor_y));
                }
                InputMode::QueryInput => {
                    let prefix_width = 1;
                    let cursor_pos = match &app.view {
                        ViewMode::Filter(state) => state.query_buffer.cursor_display_pos(),
                        ViewMode::Daily(_) => app.command_buffer.cursor_display_pos(),
                    };
                    let cursor_x = chunks[1].x + prefix_width + cursor_pos as u16;
                    let cursor_y = chunks[1].y;
                    f.set_cursor_position((cursor_x, cursor_y));
                }
                _ => {}
            }

            if app.show_help {
                let popup_area = ui::centered_rect(75, 70, size);
                app.help_visible_height = popup_area.height.saturating_sub(3) as usize;

                f.render_widget(Clear, popup_area);

                let total = ui::get_help_total_lines();
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
                    ui::render_help_content(app.help_scroll, app.help_visible_height);
                let help_paragraph = Paragraph::new(help_content);
                f.render_widget(help_paragraph, inner_area);

                let footer_area = ratatui::layout::Rect {
                    x: inner_area.x,
                    y: inner_area.y + inner_area.height.saturating_sub(1),
                    width: inner_area.width,
                    height: 1,
                };
                let footer_line = if arrows.is_empty() {
                    ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled("?", Style::default().fg(Color::White)),
                        ratatui::text::Span::styled(
                            " close ",
                            Style::default().fg(Color::DarkGray),
                        ),
                    ])
                } else {
                    ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(arrows, Style::default().fg(Color::White)),
                        ratatui::text::Span::styled(
                            " scroll  ",
                            Style::default().fg(Color::DarkGray),
                        ),
                        ratatui::text::Span::styled("?", Style::default().fg(Color::White)),
                        ratatui::text::Span::styled(
                            " close ",
                            Style::default().fg(Color::DarkGray),
                        ),
                    ])
                };
                let footer =
                    Paragraph::new(footer_line).alignment(ratatui::layout::Alignment::Right);
                f.render_widget(footer, footer_area);
            }

            // Render confirm dialog if active
            if let InputMode::Confirm(context) = &app.input_mode {
                let (title, messages): (&str, &[&str]) = match context {
                    ConfirmContext::CreateProjectJournal => (
                        " Create Project Journal ",
                        &["No project journal found.", "Create .caliber/journal.md?"],
                    ),
                    ConfirmContext::AddToGitignore => {
                        (" Add to .gitignore ", &["Add .caliber/ to .gitignore?"])
                    }
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
        })?;

        if event::poll(std::time::Duration::from_millis(16))?
            && let Event::Key(key) = event::read()?
        {
            app.status_message = None;

            if app.show_help {
                handlers::handle_help_key(&mut app, key.code);
            } else {
                match &app.input_mode {
                    InputMode::Command => handlers::handle_command_key(&mut app, key)?,
                    InputMode::Normal => handlers::handle_normal_key(&mut app, key.code)?,
                    InputMode::Edit(_) => handlers::handle_edit_key(&mut app, key),
                    InputMode::QueryInput => handlers::handle_query_input_key(&mut app, key)?,
                    InputMode::Reorder => handlers::handle_reorder_key(&mut app, key.code),
                    InputMode::Confirm(_) => handlers::handle_confirm_key(&mut app, key.code)?,
                    InputMode::Selection(_) => handlers::handle_selection_key(&mut app, key)?,
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
