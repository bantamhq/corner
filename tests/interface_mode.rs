mod helpers;

use caliber::app::{InputMode, InterfaceContext};
use crossterm::event::KeyCode;
use helpers::TestContext;

#[test]
fn date_interface_opens_with_backslash() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('\\'));

    assert!(matches!(
        ctx.app.input_mode,
        InputMode::Interface(InterfaceContext::Date(_))
    ));
}

#[test]
fn date_interface_closes_with_backslash() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('\\'));
    assert!(matches!(
        ctx.app.input_mode,
        InputMode::Interface(InterfaceContext::Date(_))
    ));

    ctx.press(KeyCode::Char('\\'));
    assert!(matches!(ctx.app.input_mode, InputMode::Normal));
}

#[test]
fn date_interface_closes_with_esc() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('\\'));
    assert!(matches!(
        ctx.app.input_mode,
        InputMode::Interface(InterfaceContext::Date(_))
    ));

    ctx.press(KeyCode::Esc);
    assert!(matches!(ctx.app.input_mode, InputMode::Normal));
}

#[test]
fn date_interface_closes_with_backspace_on_empty_input() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('\\'));
    ctx.press(KeyCode::Tab);
    ctx.press(KeyCode::Backspace);

    assert!(matches!(ctx.app.input_mode, InputMode::Normal));
}

#[test]
fn date_interface_backspace_deletes_char_when_input_has_text() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('\\'));
    ctx.press(KeyCode::Tab);
    ctx.type_str("1/15");
    ctx.press(KeyCode::Backspace);

    assert!(matches!(
        ctx.app.input_mode,
        InputMode::Interface(InterfaceContext::Date(_))
    ));
    if let InputMode::Interface(InterfaceContext::Date(ref state)) = ctx.app.input_mode {
        assert_eq!(state.query.content(), "1/1");
    }
}

#[test]
fn date_interface_toggle_closes_even_with_input_focus_and_text() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('\\'));
    ctx.press(KeyCode::Tab);
    ctx.type_str("1/15");
    ctx.press(KeyCode::Char('\\'));

    assert!(matches!(ctx.app.input_mode, InputMode::Normal));
}

#[test]
fn project_interface_opens_with_plus() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('+'));

    assert!(matches!(
        ctx.app.input_mode,
        InputMode::Interface(InterfaceContext::Project(_))
    ));
}

#[test]
fn project_interface_closes_with_plus() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('+'));
    assert!(matches!(
        ctx.app.input_mode,
        InputMode::Interface(InterfaceContext::Project(_))
    ));

    ctx.press(KeyCode::Char('+'));
    assert!(matches!(ctx.app.input_mode, InputMode::Normal));
}

#[test]
fn project_interface_closes_with_esc() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('+'));
    assert!(matches!(
        ctx.app.input_mode,
        InputMode::Interface(InterfaceContext::Project(_))
    ));

    ctx.press(KeyCode::Esc);
    assert!(matches!(ctx.app.input_mode, InputMode::Normal));
}

#[test]
fn project_interface_navigation_keeps_interface_open() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('+'));
    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('k'));

    assert!(matches!(
        ctx.app.input_mode,
        InputMode::Interface(InterfaceContext::Project(_))
    ));
}
