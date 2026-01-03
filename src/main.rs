mod app;
mod config;
mod cursor;
mod handlers;
mod storage;
mod ui;

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
    widgets::{Block, Borders, Clear, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use app::{
    App, DAILY_HEADER_LINES, DATE_SUFFIX_WIDTH, EditContext, FILTER_HEADER_LINES, InputMode,
    ViewMode,
};
use config::{Config, resolve_path};
use cursor::cursor_position_in_wrap;
use storage::Line;

fn ensure_selected_visible(
    scroll_offset: &mut usize,
    selected: usize,
    entry_count: usize,
    visible_height: usize,
) {
    if entry_count == 0 {
        *scroll_offset = 0;
        return;
    }
    if selected < *scroll_offset {
        *scroll_offset = selected;
    }
    if selected >= *scroll_offset + visible_height {
        *scroll_offset = selected - visible_height + 1;
    }
}

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

    let journal_path = if let Some(path) = cli_file {
        resolve_path(&path.to_string_lossy())
    } else {
        config.get_journal_path()
    };

    storage::set_journal_path(journal_path);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, config);

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
    config: config::Config,
) -> io::Result<()> {
    let mut app = App::new(config)?;

    loop {
        let is_filter_context = matches!(app.view, ViewMode::Filter(_))
            || matches!(
                app.input_mode,
                InputMode::Edit(EditContext::FilterEdit { .. })
                    | InputMode::Edit(EditContext::FilterQuickAdd { .. })
                    | InputMode::QueryInput
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
            let content_width = content_area.width as usize;

            let filter_visual_line = app.filter_visual_line();
            let filter_total_lines = app.filter_total_lines();
            let daily_entry_count = app.daily_entry_count();

            match &mut app.view {
                ViewMode::Filter(state) => {
                    ensure_selected_visible(
                        &mut state.scroll_offset,
                        filter_visual_line,
                        filter_total_lines,
                        visible_height,
                    );
                }
                ViewMode::Daily(state) => {
                    ensure_selected_visible(
                        &mut state.scroll_offset,
                        state.selected + DAILY_HEADER_LINES,
                        daily_entry_count + DAILY_HEADER_LINES,
                        visible_height,
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
                match ctx {
                    EditContext::FilterQuickAdd { entry_type, .. } => {
                        let ViewMode::Filter(state) = &mut app.view else {
                            unreachable!()
                        };
                        let prefix_width = entry_type.prefix().len();
                        let text_width = content_width.saturating_sub(prefix_width);

                        let (cursor_row, cursor_col) = cursor_position_in_wrap(
                            buffer.content(),
                            buffer.cursor_display_pos(),
                            text_width,
                        );

                        let entry_start_line = state.entries.len() + FILTER_HEADER_LINES;
                        let cursor_line = entry_start_line + cursor_row;

                        if cursor_line >= state.scroll_offset + visible_height {
                            state.scroll_offset = cursor_line - visible_height + 1;
                        }

                        if cursor_line >= state.scroll_offset {
                            let screen_row = cursor_line - state.scroll_offset;

                            #[allow(clippy::cast_possible_truncation)]
                            let cursor_x = content_area.x + (prefix_width + cursor_col) as u16;
                            #[allow(clippy::cast_possible_truncation)]
                            let cursor_y = content_area.y + screen_row as u16;

                            if cursor_x < content_area.x + content_area.width
                                && cursor_y < content_area.y + content_area.height
                            {
                                f.set_cursor_position((cursor_x, cursor_y));
                            }
                        }
                    }
                    EditContext::FilterEdit { filter_index, .. } => {
                        let ViewMode::Filter(state) = &mut app.view else {
                            unreachable!()
                        };
                        let Some(filter_entry) = state.entries.get(*filter_index) else {
                            return;
                        };
                        let prefix_width = filter_entry.entry_type.prefix().len();
                        let text_width =
                            content_width.saturating_sub(prefix_width + DATE_SUFFIX_WIDTH);

                        let (cursor_row, cursor_col) = cursor_position_in_wrap(
                            buffer.content(),
                            buffer.cursor_display_pos(),
                            text_width,
                        );

                        let entry_start_line = *filter_index + FILTER_HEADER_LINES;
                        let cursor_line = entry_start_line + cursor_row;

                        if cursor_line >= state.scroll_offset + visible_height {
                            state.scroll_offset = cursor_line - visible_height + 1;
                        }

                        if cursor_line >= state.scroll_offset {
                            let screen_row = cursor_line - state.scroll_offset;

                            #[allow(clippy::cast_possible_truncation)]
                            let cursor_x = content_area.x + (prefix_width + cursor_col) as u16;
                            #[allow(clippy::cast_possible_truncation)]
                            let cursor_y = content_area.y + screen_row as u16;

                            if cursor_x < content_area.x + content_area.width
                                && cursor_y < content_area.y + content_area.height
                            {
                                f.set_cursor_position((cursor_x, cursor_y));
                            }
                        }
                    }
                    EditContext::Daily { entry_index } => {
                        let ViewMode::Daily(state) = &mut app.view else {
                            unreachable!()
                        };
                        let Some(entry_type) = app.entry_indices.get(*entry_index).and_then(|&i| {
                            if let Line::Entry(entry) = &app.lines[i] {
                                Some(&entry.entry_type)
                            } else {
                                None
                            }
                        }) else {
                            return;
                        };
                        let prefix_width = entry_type.prefix().width();
                        let text_width = content_width.saturating_sub(prefix_width);

                        let (cursor_row, cursor_col) = cursor_position_in_wrap(
                            buffer.content(),
                            buffer.cursor_display_pos(),
                            text_width,
                        );

                        let entry_start_line =
                            state.later_entries.len() + *entry_index + DAILY_HEADER_LINES;
                        let cursor_line = entry_start_line + cursor_row;

                        if cursor_line >= state.scroll_offset + visible_height {
                            state.scroll_offset = cursor_line - visible_height + 1;
                        }

                        if cursor_line >= state.scroll_offset {
                            let screen_row = cursor_line - state.scroll_offset;

                            #[allow(clippy::cast_possible_truncation)]
                            let cursor_x = content_area.x + (prefix_width + cursor_col) as u16;
                            #[allow(clippy::cast_possible_truncation)]
                            let cursor_y = content_area.y + screen_row as u16;

                            if cursor_x < content_area.x + content_area.width
                                && cursor_y < content_area.y + content_area.height
                            {
                                f.set_cursor_position((cursor_x, cursor_y));
                            }
                        }
                    }
                    EditContext::LaterEdit { later_index, .. } => {
                        let ViewMode::Daily(state) = &mut app.view else {
                            unreachable!()
                        };
                        let Some(later_entry) = state.later_entries.get(*later_index) else {
                            return;
                        };
                        let prefix_width = later_entry.entry_type.prefix().width();
                        let text_width =
                            content_width.saturating_sub(prefix_width + DATE_SUFFIX_WIDTH);

                        let (cursor_row, cursor_col) = cursor_position_in_wrap(
                            buffer.content(),
                            buffer.cursor_display_pos(),
                            text_width,
                        );

                        let entry_start_line = *later_index + DAILY_HEADER_LINES;
                        let cursor_line = entry_start_line + cursor_row;

                        if cursor_line >= state.scroll_offset + visible_height {
                            state.scroll_offset = cursor_line - visible_height + 1;
                        }

                        if cursor_line >= state.scroll_offset {
                            let screen_row = cursor_line - state.scroll_offset;

                            #[allow(clippy::cast_possible_truncation)]
                            let cursor_x = content_area.x + (prefix_width + cursor_col) as u16;
                            #[allow(clippy::cast_possible_truncation)]
                            let cursor_y = content_area.y + screen_row as u16;

                            if cursor_x < content_area.x + content_area.width
                                && cursor_y < content_area.y + content_area.height
                            {
                                f.set_cursor_position((cursor_x, cursor_y));
                            }
                        }
                    }
                }
            }

            #[allow(clippy::cast_possible_truncation)]
            let scroll_offset = match &app.view {
                ViewMode::Filter(state) => state.scroll_offset,
                ViewMode::Daily(state) => state.scroll_offset,
            };
            let content = Paragraph::new(lines).scroll((scroll_offset as u16, 0));
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
                    InputMode::Order => handlers::handle_order_key(&mut app, key.code),
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
