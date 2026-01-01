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

use app::{App, Mode};
use config::Config;

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

fn cursor_position_in_wrap(
    text: &str,
    cursor_display_pos: usize,
    max_width: usize,
) -> (usize, usize) {
    use unicode_width::UnicodeWidthStr;

    if max_width == 0 {
        return (0, cursor_display_pos);
    }

    let mut row = 0;
    let mut line_width = 0;
    let mut total_width = 0;

    for word in text.split_inclusive(' ') {
        let word_width = word.width();
        let word_start = total_width;

        // Determine if this word fits or needs new line
        let (word_row, word_line_start) = if line_width + word_width <= max_width {
            // Word fits on current line
            let start_col = line_width;
            line_width += word_width;
            (row, start_col)
        } else if line_width == 0 {
            // Word is too long but line is empty - will be broken by character
            (row, 0)
        } else {
            // Start new line with this word
            row += 1;
            line_width = word_width;
            (row, 0)
        };

        // Check if cursor is within this word
        let word_end = word_start + word_width;
        if cursor_display_pos >= word_start && cursor_display_pos < word_end {
            // Cursor is in this word - but we need to handle character breaking for long words
            if line_width == 0 || word_width <= max_width || word_line_start > 0 {
                // Word fits on a line normally
                return (word_row, word_line_start + (cursor_display_pos - word_start));
            } else {
                // Word is being broken by character - simulate character-by-character
                let mut char_row = word_row;
                let mut char_col = word_line_start;
                let mut char_pos = word_start;

                for ch in word.chars() {
                    let ch_width = ch.to_string().width();

                    if char_pos == cursor_display_pos {
                        return (char_row, char_col);
                    }

                    if char_col + ch_width > max_width && char_col > 0 {
                        char_row += 1;
                        char_col = 0;
                    }

                    char_col += ch_width;
                    char_pos += ch_width;
                }
            }
        }

        // Handle character breaking for overly long words (update row/line_width)
        if word_width > max_width && word_line_start == 0 {
            let mut char_col = 0;
            for ch in word.chars() {
                let ch_width = ch.to_string().width();
                if char_col + ch_width > max_width && char_col > 0 {
                    row += 1;
                    char_col = 0;
                }
                char_col += ch_width;
            }
            line_width = char_col;
        }

        total_width = word_end;
    }

    // Cursor is at the end of text - stays on current line
    (row, line_width)
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
        if path.is_absolute() {
            path
        } else {
            std::env::current_dir()?.join(path)
        }
    } else {
        config.get_journal_path()
    };

    storage::set_journal_path(journal_path);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal);

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

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    let mut app = App::new()?;

    loop {
        let is_filter_mode = app.mode == Mode::Filter
            || (app.mode == Mode::Edit && app.filter_edit_target.is_some());

        terminal.draw(|f| {
            let size = f.area();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(1)])
                .split(size);

            let main_block = if is_filter_mode {
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

            if is_filter_mode {
                let visual_line = app.filter_visual_line();
                let total_lines = app.filter_total_lines();
                ensure_selected_visible(
                    &mut app.filter_scroll_offset,
                    visual_line,
                    total_lines,
                    visible_height,
                );
            } else {
                ensure_selected_visible(
                    &mut app.scroll_offset,
                    app.selected + 1,
                    app.entry_indices.len() + 1,
                    visible_height,
                );
                if app.selected == 0 {
                    app.scroll_offset = 0;
                }
            }

            let lines = if is_filter_mode {
                ui::render_filter_view(&app, content_width)
            } else {
                ui::render_daily_view(&app, content_width)
            };

            if app.mode == Mode::Edit
                && let Some(ref buffer) = app.edit_buffer
            {
                if let Some(item) = app.filter_edit_target.as_ref().and_then(|_| {
                    app.filter_items.get(app.filter_selected)
                }) {
                    let prefix_width = match &item.entry_type {
                        storage::EntryType::Task { .. } => 6,
                        storage::EntryType::Note => 2,
                        storage::EntryType::Event => 2,
                    };
                    let date_suffix_width = 8;
                    let text_width = content_width.saturating_sub(prefix_width + date_suffix_width);

                    let (cursor_row, cursor_col) = cursor_position_in_wrap(
                        buffer.content(),
                        buffer.cursor_display_pos(),
                        text_width,
                    );

                    let entry_start_line = app.filter_selected + 1;
                    let cursor_line = entry_start_line + cursor_row;

                    if cursor_line >= app.filter_scroll_offset + visible_height {
                        app.filter_scroll_offset = cursor_line - visible_height + 1;
                    }

                    if cursor_line >= app.filter_scroll_offset {
                        let screen_row = cursor_line - app.filter_scroll_offset;

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
                } else if let Some(entry) = app.get_selected_entry() {
                    let prefix = entry.prefix();
                    let prefix_width = prefix.width();
                    let text_width = content_width.saturating_sub(prefix_width);

                    let (cursor_row, cursor_col) = cursor_position_in_wrap(
                        buffer.content(),
                        buffer.cursor_display_pos(),
                        text_width,
                    );

                    let entry_start_line = app.selected + 1;
                    let cursor_line = entry_start_line + cursor_row;

                    if cursor_line >= app.scroll_offset + visible_height {
                        app.scroll_offset = cursor_line - visible_height + 1;
                    }

                    if cursor_line >= app.scroll_offset {
                        let screen_row = cursor_line - app.scroll_offset;
                        let col_offset = prefix_width;

                        #[allow(clippy::cast_possible_truncation)]
                        let cursor_x = content_area.x + (col_offset + cursor_col) as u16;
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

            #[allow(clippy::cast_possible_truncation)]
            let scroll_offset = if is_filter_mode {
                app.filter_scroll_offset
            } else {
                app.scroll_offset
            };
            let content = Paragraph::new(lines).scroll((scroll_offset as u16, 0));
            f.render_widget(content, content_area);

            let footer = Paragraph::new(ui::render_footer(&app));
            f.render_widget(footer, chunks[1]);

            if app.show_help {
                let popup_area = ui::centered_rect(50, 70, size);
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

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            app.status_message = None;

            if app.show_help {
                handlers::handle_help_key(&mut app, key.code);
            } else {
                match app.mode {
                    Mode::Command => handlers::handle_command_key(&mut app, key.code)?,
                    Mode::Daily => handlers::handle_daily_key(&mut app, key.code)?,
                    Mode::Edit => handlers::handle_editing_key(&mut app, key.code),
                    Mode::Filter => handlers::handle_filter_key(&mut app, key.code)?,
                    Mode::FilterInput => handlers::handle_filter_input_key(&mut app, key.code)?,
                    Mode::Order => handlers::handle_order_key(&mut app, key.code),
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
