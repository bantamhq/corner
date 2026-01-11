use std::fs;
use std::io;
use std::path::PathBuf;

use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use caliber::app::{App, InputMode};
use caliber::config::{self, Config, resolve_path};
use caliber::storage::{JournalContext, JournalSlot};
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

    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| io::Error::other(format!("Failed to create tokio runtime: {e}")))?;
    let runtime_handle = Some(runtime.handle().clone());

    let mut app = App::new_with_context(config, date, journal_context, runtime_handle)?;

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

        terminal.draw(|f| ui::render_app(f, &mut app))?;

        app.poll_calendar_results();

        if event::poll(std::time::Duration::from_millis(16))?
            && let Event::Key(key) = event::read()?
        {
            app.status_message = None;

            if app.help_visible {
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
