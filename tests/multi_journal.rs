mod helpers;

use chrono::NaiveDate;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

use caliber::app::{App, InputMode};
use caliber::config::Config;
use caliber::storage::{self, JournalSlot, Line};

/// MJ-4: Journal isolation - entries don't cross journals
#[test]
fn test_journal_isolation() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let temp_dir = TempDir::new().unwrap();

    let global_path = temp_dir.path().join("global.md");
    let project_path = temp_dir.path().join("project.md");

    fs::write(&global_path, "# 2026/01/15\n- [ ] Global entry\n").unwrap();
    fs::write(&project_path, "# 2026/01/15\n- [ ] Project entry\n").unwrap();

    let context =
        storage::JournalContext::new(global_path, Some(project_path), JournalSlot::Global);

    let config = Config::default();
    let app = App::new_with_context(config, date, context).unwrap();

    // Should see global entry
    let has_global = app.entry_indices.iter().any(|&i| {
        if let Line::Entry(e) = &app.lines[i] {
            e.content.contains("Global entry")
        } else {
            false
        }
    });
    assert!(has_global, "Global entry should be visible");

    // Should NOT see project entry
    let has_project = app.entry_indices.iter().any(|&i| {
        if let Line::Entry(e) = &app.lines[i] {
            e.content.contains("Project entry")
        } else {
            false
        }
    });
    assert!(
        !has_project,
        "Project entry should not be visible in global"
    );
}

/// MJ-4: Switch to project journal sees project entries
#[test]
fn test_project_journal_switch() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let temp_dir = TempDir::new().unwrap();

    let global_path = temp_dir.path().join("global.md");
    let project_path = temp_dir.path().join("project.md");

    fs::write(&global_path, "# 2026/01/15\n- [ ] Global entry\n").unwrap();
    fs::write(&project_path, "# 2026/01/15\n- [ ] Project entry\n").unwrap();

    // Start in project
    let context =
        storage::JournalContext::new(global_path, Some(project_path), JournalSlot::Project);

    let config = Config::default();
    let app = App::new_with_context(config, date, context).unwrap();

    // Should see project entry
    let has_project = app.entry_indices.iter().any(|&i| {
        if let Line::Entry(e) = &app.lines[i] {
            e.content.contains("Project entry")
        } else {
            false
        }
    });
    assert!(has_project, "Project entry should be visible");

    // Should NOT see global entry
    let has_global = app.entry_indices.iter().any(|&i| {
        if let Line::Entry(e) = &app.lines[i] {
            e.content.contains("Global entry")
        } else {
            false
        }
    });
    assert!(!has_global, "Global entry should not be visible in project");
}

/// MJ-1: Toggle between journals with backtick key
#[test]
fn test_journal_toggle_key() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let temp_dir = TempDir::new().unwrap();

    let global_path = temp_dir.path().join("global.md");
    let project_path = temp_dir.path().join("project.md");

    fs::write(&global_path, "# 2026/01/15\n- [ ] Global entry\n").unwrap();
    fs::write(&project_path, "# 2026/01/15\n- [ ] Project entry\n").unwrap();

    let context =
        storage::JournalContext::new(global_path, Some(project_path), JournalSlot::Global);

    let config = Config::default();
    let mut app = App::new_with_context(config, date, context).unwrap();

    // Verify we're in global
    assert_eq!(
        app.active_journal(),
        JournalSlot::Global,
        "Should start in global journal"
    );

    // Press backtick to toggle (simulating the handler)
    let event = KeyEvent::new(KeyCode::Char('`'), KeyModifiers::NONE);
    let _ = caliber::handlers::handle_normal_key(&mut app, event);

    // Should now be in project
    assert_eq!(
        app.active_journal(),
        JournalSlot::Project,
        "Should switch to project journal"
    );

    // Toggle back
    let _ = caliber::handlers::handle_normal_key(&mut app, event);
    assert_eq!(
        app.active_journal(),
        JournalSlot::Global,
        "Should switch back to global journal"
    );
}

/// MJ-2: Project journal creation confirmation flow
#[test]
fn test_project_journal_creation() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let temp_dir = TempDir::new().unwrap();

    // Initialize a git repo in the temp directory
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init git repo");

    // Create global journal but NOT a project journal
    let global_path = temp_dir.path().join("global.md");
    fs::write(&global_path, "# 2026/01/15\n- [ ] Global entry\n").unwrap();

    // Set up with no project journal - will trigger creation flow
    let context = storage::JournalContext::new(global_path.clone(), None, JournalSlot::Global);

    let config = Config::default();
    let mut app = App::new_with_context(config, date, context).unwrap();

    // Press backtick to try switching to project journal
    let event = KeyEvent::new(KeyCode::Char('`'), KeyModifiers::NONE);
    let _ = caliber::handlers::handle_normal_key(&mut app, event);

    // Should be in Confirm mode (asking to create project journal)
    assert!(
        matches!(app.input_mode, InputMode::Confirm(_)),
        "Should enter confirm mode to create project journal"
    );

    // Press 'y' to confirm creation
    let _ = caliber::handlers::handle_confirm_key(&mut app, KeyCode::Char('y'));

    // After first confirmation, may be in another Confirm (gitignore) or done
    // The flow depends on whether .gitignore handling is needed
    // For now, verify we can complete the flow without crashing

    // If still in confirm mode (gitignore question), press 'n' to skip
    if matches!(app.input_mode, InputMode::Confirm(_)) {
        let _ = caliber::handlers::handle_confirm_key(&mut app, KeyCode::Char('n'));
    }

    // Should now be in project journal mode (or remain functional)
    // The app should not crash and should be in a valid state
    assert!(
        matches!(app.input_mode, InputMode::Normal),
        "Should return to normal mode after confirmation flow"
    );
}
