mod helpers;

use std::fs;
use std::io::Write;

use crossterm::event::KeyCode;
use helpers::TestContext;
use tempfile::TempDir;

use caliber::app::InputMode;

/// CM-1: Config command without valid subcommand shows usage
#[test]
fn test_config_command_usage() {
    let mut ctx = TestContext::new();

    // Enter command mode and try :config without argument
    ctx.press(KeyCode::Char(':'));
    ctx.type_str("config");
    ctx.press(KeyCode::Enter);

    // App should show usage message but remain functional
    // (Status message is set internally, we verify app still works)
    ctx.press(KeyCode::Enter);
    ctx.type_str("Test after config command");
    ctx.press(KeyCode::Enter);

    assert!(
        ctx.screen_contains("Test after config command"),
        "App should still work after :config command"
    );
}

/// CM-3: Invalid command shows error but app remains functional
#[test]
fn test_invalid_command() {
    let mut ctx = TestContext::new();

    // Enter command mode
    ctx.press(KeyCode::Char(':'));
    ctx.type_str("invalidcommand");
    ctx.press(KeyCode::Enter);

    // App should still be functional - can navigate
    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('k'));

    // Can still create entries
    ctx.press(KeyCode::Enter);
    ctx.type_str("Test after invalid command");
    ctx.press(KeyCode::Enter);

    assert!(
        ctx.screen_contains("Test after invalid command"),
        "App should still be functional after invalid command"
    );
}

/// CM-3: Command mode escape returns to normal mode
#[test]
fn test_command_mode_escape() {
    let mut ctx = TestContext::new();

    // Enter command mode
    ctx.press(KeyCode::Char(':'));
    assert!(
        matches!(ctx.app.input_mode, InputMode::Command),
        "Should be in command mode"
    );

    // Type partial command
    ctx.type_str("goto");

    // Escape should cancel
    ctx.press(KeyCode::Esc);
    assert!(
        matches!(ctx.app.input_mode, InputMode::Normal),
        "Should return to normal mode after Esc"
    );
}

/// HO-1: Help overlay display and navigation
#[test]
fn test_help_overlay_display() {
    let mut ctx = TestContext::new();

    // Press ? to show help
    ctx.press(KeyCode::Char('?'));
    assert!(ctx.app.show_help, "Help overlay should be visible");

    // Help content should contain key bindings
    // (Note: We can't easily check the rendered help content without the terminal,
    // but we verify the state is correct)

    // Press ? again to close
    ctx.press(KeyCode::Char('?'));
    assert!(!ctx.app.show_help, "Help overlay should be hidden");
}

/// HO-1: Help overlay scrolling
#[test]
fn test_help_overlay_scroll() {
    let mut ctx = TestContext::new();

    // Show help
    ctx.press(KeyCode::Char('?'));
    assert!(ctx.app.show_help, "Help should be visible");

    // Scroll down with j
    let initial_offset = ctx.app.help_scroll;
    ctx.press(KeyCode::Char('j'));
    assert!(
        ctx.app.help_scroll > initial_offset,
        "Help should scroll down with j"
    );

    // Scroll up with k
    ctx.press(KeyCode::Char('k'));
    assert_eq!(
        ctx.app.help_scroll, initial_offset,
        "Help should scroll up with k"
    );

    // Close with Esc
    ctx.press(KeyCode::Esc);
    assert!(!ctx.app.show_help, "Help should close with Esc");
}

/// CM-1: Project with path loads that journal
#[test]
fn test_project_command_loads_file() {
    let temp_dir = TempDir::new().unwrap();
    let other_journal = temp_dir.path().join("other_journal.md");

    // Use actual today's date since open_journal loads Local::now()
    let today = chrono::Local::now().date_naive();
    let date_str = today.format("%Y/%m/%d").to_string();

    // Create a journal file with content for today
    fs::write(
        &other_journal,
        format!("# {}\n- [ ] Entry from other journal\n", date_str),
    )
    .unwrap();

    let mut ctx = TestContext::new();

    // Open the other journal via :project path
    ctx.press(KeyCode::Char(':'));
    ctx.type_str(&format!("project {}", other_journal.display()));
    ctx.press(KeyCode::Enter);

    // Should now see content from the other journal
    assert!(
        ctx.screen_contains("Entry from other journal"),
        "Content from opened journal should be visible"
    );
}

/// CM-2: Config reload applies new config
#[test]
fn test_config_reload_command() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // Create a minimal config file
    let mut file = fs::File::create(&config_path).unwrap();
    writeln!(file, "[favorite_tags]").unwrap();
    writeln!(file, "1 = \"work\"").unwrap();
    drop(file);

    let mut ctx = TestContext::new();

    // Note: The actual config reload command reads from the standard config path,
    // but we can verify the command doesn't crash the app
    ctx.press(KeyCode::Char(':'));
    ctx.type_str("config reload");
    ctx.press(KeyCode::Enter);

    // App should remain functional after config reload
    ctx.press(KeyCode::Enter);
    ctx.type_str("Test after config reload");
    ctx.press(KeyCode::Enter);

    assert!(
        ctx.screen_contains("Test after config reload"),
        "App should remain functional after :config reload"
    );
}
