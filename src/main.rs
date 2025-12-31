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
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};

use app::{App, Mode};
use config::Config;

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
        let is_tasks_mode = app.mode == Mode::Tasks;

        terminal.draw(|f| {
            let size = f.area();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(1)])
                .split(size);

            let main_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White));

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

            let mut lines = if is_tasks_mode {
                ui::render_tasks_view(&app)
            } else {
                ui::render_daily_view(&app)
            };

            if !is_tasks_mode {
                ui::render_editing_cursor(&app, &mut lines);

                if app.mode == Mode::Edit
                    && let Some(ref buffer) = app.edit_buffer
                    && let Some(entry) = app.get_selected_entry()
                {
                    let prefix_len = entry.prefix().chars().count();
                    let cursor_col = prefix_len + buffer.cursor_char_pos();

                    #[allow(clippy::cast_possible_truncation)]
                    let cursor_x = content_area.x + cursor_col as u16;
                    #[allow(clippy::cast_possible_truncation)]
                    let cursor_y = content_area.y + (app.selected + 1) as u16;
                    if cursor_x < content_area.x + content_area.width
                        && cursor_y < content_area.y + content_area.height
                    {
                        f.set_cursor_position((cursor_x, cursor_y));
                    }
                }
            }

            let content = Paragraph::new(lines);
            f.render_widget(content, content_area);

            let footer = Paragraph::new(ui::render_footer(&app));
            f.render_widget(footer, chunks[1]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            app.status_message = None;
            match app.mode {
                Mode::Command => handlers::handle_command_key(&mut app, key.code)?,
                Mode::Daily => handlers::handle_normal_key(&mut app, key.code)?,
                Mode::Edit => handlers::handle_editing_key(&mut app, key.code),
                Mode::Tasks => handlers::handle_tasks_key(&mut app, key.code)?,
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
