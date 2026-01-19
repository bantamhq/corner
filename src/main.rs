use std::fs;
use std::io;
use std::path::PathBuf;

use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use caliber::app::{App, InputMode};
use caliber::config::{self, Config, get_profile_project_root, has_custom_profile, init_profile};
use caliber::storage::{JournalContext, JournalSlot};
use caliber::ui::surface::Surface;
use caliber::{handlers, storage, testrun, ui};

fn main() -> Result<(), io::Error> {
    let args: Vec<String> = std::env::args().collect();
    let (testrun_path, remaining_args) = testrun::parse_arg(&args);
    let (recorder_name, remaining_args) = parse_record_arg(&remaining_args);
    let recorder = recorder_name.map(|name| {
        let name = name.strip_suffix(".tape").unwrap_or(&name).to_string();
        caliber::recorder::Recorder::new(name)
    });

    let temp_dir = testrun_path
        .as_ref()
        .map(|source| testrun::create_temp_profile(source))
        .transpose()?;

    init_profile(temp_dir.as_deref().or(testrun_path.as_deref()));

    if remaining_args.first().map(String::as_str) == Some("init") {
        return init_config();
    }

    let (project_path, active_slot) = if let Some(path) = detect_project_with_profile() {
        (Some(path), JournalSlot::Project)
    } else {
        (None, JournalSlot::Hub)
    };

    let config_load = match active_slot {
        JournalSlot::Hub => Config::load_hub().unwrap_or_default(),
        JournalSlot::Project => Config::load_merged().unwrap_or_default(),
    };

    let hub_path = config_load.config.get_hub_journal_path();

    let journal_context = JournalContext::new(hub_path, project_path.clone(), active_slot);

    let surface = Surface::from_terminal();

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableBracketedPaste);
        original_hook(panic_info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(
        &mut terminal,
        config_load.config,
        journal_context,
        surface,
        config_load.warning,
        recorder,
    );

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableBracketedPaste
    )?;
    terminal.show_cursor()?;

    if let Some(temp) = temp_dir {
        testrun::cleanup(temp);
    }

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
    surface: Surface,
    config_warning: Option<String>,
    mut recorder: Option<caliber::recorder::Recorder>,
) -> io::Result<()> {
    let date = chrono::Local::now().date_naive();

    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| io::Error::other(format!("Failed to create tokio runtime: {e}")))?;
    let runtime_handle = Some(runtime.handle().clone());

    let mut app = App::new_with_context(config, date, journal_context, runtime_handle, surface)?;

    if let Some(warning) = config_warning {
        app.set_error(warning);
    }

    if recorder.is_some() {
        app.set_status("Recording to tape...");
    }

    if !has_custom_profile()
        && app.in_git_repo
        && let Some(git_root) = storage::find_git_root()
    {
        let caliber_dir = git_root.join(".caliber");

        if caliber_dir.exists() {
            let mut registry = storage::ProjectRegistry::load();
            if registry.find_by_path(&caliber_dir).is_none() {
                let _ = registry.register(caliber_dir.clone());
                let _ = registry.save();
            }
        }

        if app.journal_context.project_path().is_none() && app.config.auto_init_project {
            fs::create_dir_all(&caliber_dir)?;

            let journal_path = app.config.get_project_journal_path(&git_root);
            if !journal_path.exists() {
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

    // Check for external file changes approximately every second (60 * 16ms)
    let mut tick_counter = 0u32;

    loop {
        if app.needs_redraw {
            terminal.clear()?;
            app.needs_redraw = false;
        }

        terminal.draw(|f| ui::render_app(f, &mut app))?;

        app.poll_calendar_results();

        // Periodically check for external file changes (~1 second intervals)
        tick_counter = tick_counter.wrapping_add(1);
        if tick_counter.is_multiple_of(60) {
            app.check_external_changes();
        }

        if event::poll(std::time::Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => {
                    if let Some(ref mut rec) = recorder {
                        rec.record(key);
                    }

                    app.status_message = None;

                    match &app.input_mode {
                        InputMode::Normal => handlers::handle_normal_key(&mut app, key)?,
                        InputMode::Edit(_) => handlers::handle_edit_key(&mut app, key),
                        InputMode::Reorder => handlers::handle_reorder_key(&mut app, key),
                        InputMode::Confirm(_) => handlers::handle_confirm_key(&mut app, key.code)?,
                        InputMode::Selection(_) => handlers::handle_selection_key(&mut app, key)?,
                        InputMode::CommandPalette(_) => {
                            handlers::handle_command_palette_key(&mut app, key)?;
                        }
                        InputMode::FilterPrompt => {
                            handlers::handle_filter_prompt_key(&mut app, key)?;
                        }
                        InputMode::DatePicker(_) => {
                            handlers::handle_date_picker_key(&mut app, key)?;
                        }
                    }
                }
                Event::Paste(text) => {
                    if matches!(app.input_mode, InputMode::Edit(_)) {
                        let first_line = text.lines().next().unwrap_or(&text);
                        if let Some(ref mut buffer) = app.edit_buffer {
                            buffer.insert_str(first_line);
                        }
                        app.update_hints();
                    }
                }
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }

    if let Some(rec) = recorder {
        rec.save()?;
    }

    Ok(())
}

fn detect_project_with_profile() -> Option<PathBuf> {
    if let Some(project_root) = get_profile_project_root() {
        let config_load = Config::load_merged_from(project_root).ok()?;
        let journal_path = config_load.config.get_project_journal_path(project_root);
        if journal_path.exists() {
            return Some(journal_path);
        }
    }

    storage::detect_project_journal()
}

fn parse_record_arg(args: &[String]) -> (Option<String>, Vec<String>) {
    let Some(record_pos) = args.iter().position(|a| a == "--record") else {
        return (None, args.to_vec());
    };

    let next_arg = args.get(record_pos + 1).filter(|s| !s.starts_with('-'));
    let output = next_arg
        .cloned()
        .unwrap_or_else(|| "recording.tape".to_string());
    let skip_count = if next_arg.is_some() { 2 } else { 1 };

    let remaining = args
        .iter()
        .enumerate()
        .filter(|&(i, _)| i < record_pos || i >= record_pos + skip_count)
        .map(|(_, a)| a.clone())
        .collect();

    (Some(output), remaining)
}
