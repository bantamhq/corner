mod helpers;

use chrono::NaiveDate;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

use caliber::app::{App, InputMode};
use caliber::config::Config;
use caliber::storage::{self, JournalSlot, Line};
use caliber::ui::surface::Surface;

#[test]
fn journals_remain_isolated_when_loading() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let temp_dir = TempDir::new().unwrap();

    let global_path = temp_dir.path().join("global.md");
    let project_path = temp_dir.path().join("project.md");

    fs::write(&global_path, "# 2026/01/15\n- [ ] Global entry\n").unwrap();
    fs::write(&project_path, "# 2026/01/15\n- [ ] Project entry\n").unwrap();

    let context = storage::JournalContext::new(global_path, Some(project_path), JournalSlot::Hub);

    let config = Config::default();
    let app = App::new_with_context(config, date, context, None, Surface::default()).unwrap();

    let has_global = app.entry_indices.iter().any(|&i| {
        if let Line::Entry(e) = &app.lines[i] {
            e.content.contains("Global entry")
        } else {
            false
        }
    });
    assert!(has_global);

    let has_project = app.entry_indices.iter().any(|&i| {
        if let Line::Entry(e) = &app.lines[i] {
            e.content.contains("Project entry")
        } else {
            false
        }
    });
    assert!(!has_project);
}

#[test]
fn project_journal_loads_project_entries() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let temp_dir = TempDir::new().unwrap();

    let global_path = temp_dir.path().join("global.md");
    let project_path = temp_dir.path().join("project.md");

    fs::write(&global_path, "# 2026/01/15\n- [ ] Global entry\n").unwrap();
    fs::write(&project_path, "# 2026/01/15\n- [ ] Project entry\n").unwrap();

    let context =
        storage::JournalContext::new(global_path, Some(project_path), JournalSlot::Project);

    let config = Config::default();
    let app = App::new_with_context(config, date, context, None, Surface::default()).unwrap();

    let has_project = app.entry_indices.iter().any(|&i| {
        if let Line::Entry(e) = &app.lines[i] {
            e.content.contains("Project entry")
        } else {
            false
        }
    });
    assert!(has_project);

    let has_global = app.entry_indices.iter().any(|&i| {
        if let Line::Entry(e) = &app.lines[i] {
            e.content.contains("Global entry")
        } else {
            false
        }
    });
    assert!(!has_global);
}

#[test]
fn backtick_toggles_between_journals() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let temp_dir = TempDir::new().unwrap();

    let global_path = temp_dir.path().join("global.md");
    let project_path = temp_dir.path().join("project.md");

    fs::write(&global_path, "# 2026/01/15\n- [ ] Global entry\n").unwrap();
    fs::write(&project_path, "# 2026/01/15\n- [ ] Project entry\n").unwrap();

    let context = storage::JournalContext::new(global_path, Some(project_path), JournalSlot::Hub);

    let config = Config::default();
    let mut app = App::new_with_context(config, date, context, None, Surface::default()).unwrap();

    assert_eq!(app.active_journal(), JournalSlot::Hub);

    let event = KeyEvent::new(KeyCode::Char('`'), KeyModifiers::NONE);
    let _ = caliber::handlers::handle_normal_key(&mut app, event);

    assert_eq!(app.active_journal(), JournalSlot::Project);

    let _ = caliber::handlers::handle_normal_key(&mut app, event);
    assert_eq!(app.active_journal(), JournalSlot::Hub);
}

#[test]
fn backtick_prompts_project_journal_creation() {
    // SAFETY: Tests run single-threaded per test file
    unsafe { std::env::set_var("CALIBER_SKIP_REGISTRY", "1") };

    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let temp_dir = TempDir::new().unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init git repo");

    let global_path = temp_dir.path().join("global.md");
    fs::write(&global_path, "# 2026/01/15\n- [ ] Global entry\n").unwrap();

    let context = storage::JournalContext::new(global_path.clone(), None, JournalSlot::Hub);

    let config = Config::default();
    let mut app = App::new_with_context(config, date, context, None, Surface::default()).unwrap();

    let event = KeyEvent::new(KeyCode::Char('`'), KeyModifiers::NONE);
    let _ = caliber::handlers::handle_normal_key(&mut app, event);

    assert!(matches!(app.input_mode, InputMode::Confirm(_)));

    let _ = caliber::handlers::handle_confirm_key(&mut app, KeyCode::Char('y'));

    if matches!(app.input_mode, InputMode::Confirm(_)) {
        let _ = caliber::handlers::handle_confirm_key(&mut app, KeyCode::Char('n'));
    }

    assert!(matches!(app.input_mode, InputMode::Normal));
}

#[test]
fn project_creation_preserves_existing_journal() {
    // SAFETY: Tests run single-threaded per test file
    unsafe { std::env::set_var("CALIBER_SKIP_REGISTRY", "1") };

    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let temp_dir = TempDir::new().unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init git repo");

    let caliber_dir = temp_dir.path().join(".caliber");
    fs::create_dir_all(&caliber_dir).unwrap();
    let project_journal = caliber_dir.join("journal.md");
    fs::write(&project_journal, "# 2026/01/15\n- [ ] Existing entry\n").unwrap();

    let global_path = temp_dir.path().join("global.md");
    fs::write(&global_path, "").unwrap();

    let context = storage::JournalContext::new(global_path, None, JournalSlot::Hub);

    let config = Config::default();
    let mut app = App::new_with_context(config, date, context, None, Surface::default()).unwrap();
    app.in_git_repo = true;

    std::env::set_current_dir(temp_dir.path()).unwrap();

    let event = KeyEvent::new(KeyCode::Char('`'), KeyModifiers::NONE);
    let _ = caliber::handlers::handle_normal_key(&mut app, event);

    if matches!(app.input_mode, InputMode::Confirm(_)) {
        let _ = caliber::handlers::handle_confirm_key(&mut app, KeyCode::Char('y'));
    }
    if matches!(app.input_mode, InputMode::Confirm(_)) {
        let _ = caliber::handlers::handle_confirm_key(&mut app, KeyCode::Char('n'));
    }

    let content = fs::read_to_string(&project_journal).unwrap();
    assert!(
        content.contains("Existing entry"),
        "Journal was overwritten! Content: {content}"
    );
}
