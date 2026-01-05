mod helpers;

use crossterm::event::KeyCode;
use helpers::TestContext;

use caliber::app::InputMode;

/// CM-1: :open without args shows usage
#[test]
fn test_open_command_usage() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char(':'));
    ctx.type_str("open");
    ctx.press(KeyCode::Enter);

    // App should remain functional
    ctx.press(KeyCode::Enter);
    ctx.type_str("Test after open");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Test after open"));
}

/// CM-2: :open with invalid target shows error
#[test]
fn test_open_command_invalid_target() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char(':'));
    ctx.type_str("open invalid");
    ctx.press(KeyCode::Enter);

    // App should remain functional
    ctx.press(KeyCode::Enter);
    ctx.type_str("Test after invalid open");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Test after invalid open"));
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
