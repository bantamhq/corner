mod helpers;

use crossterm::event::KeyCode;
use helpers::TestContext;

use caliber::app::{InputMode, PromptContext};

#[test]
fn open_command_allows_continued_editing() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char(':'));
    ctx.type_str("open");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Enter);
    ctx.type_str("Test after open");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Test after open"));
}

#[test]
fn open_command_handles_invalid_target() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char(':'));
    ctx.type_str("open invalid");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Enter);
    ctx.type_str("Test after invalid open");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Test after invalid open"));
}

#[test]
fn invalid_command_returns_to_normal_mode() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char(':'));
    ctx.type_str("invalidcommand");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('k'));

    ctx.press(KeyCode::Enter);
    ctx.type_str("Test after invalid command");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Test after invalid command"));
}

#[test]
fn escape_exits_command_mode() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char(':'));
    assert!(matches!(ctx.app.input_mode, InputMode::Prompt(PromptContext::Command { .. })));

    ctx.type_str("goto");

    ctx.press(KeyCode::Esc);
    assert!(matches!(ctx.app.input_mode, InputMode::Normal));
}

#[test]
fn question_mark_toggles_help_overlay() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char('?'));
    assert!(ctx.app.show_help);

    ctx.press(KeyCode::Char('?'));
    assert!(!ctx.app.show_help);
}

#[test]
fn help_overlay_scrolls_with_j_k() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char('?'));
    assert!(ctx.app.show_help);

    let initial_offset = ctx.app.help_scroll;
    ctx.press(KeyCode::Char('j'));
    assert!(ctx.app.help_scroll > initial_offset);

    ctx.press(KeyCode::Char('k'));
    assert_eq!(ctx.app.help_scroll, initial_offset);

    ctx.press(KeyCode::Esc);
    assert!(!ctx.app.show_help);
}
