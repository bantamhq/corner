mod helpers;

use crossterm::event::{KeyCode, KeyModifiers};
use helpers::TestContext;

use caliber::app::InputMode;

#[test]
fn home_end_keys_move_cursor_to_boundaries() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("hello world");

    ctx.press(KeyCode::Home);
    ctx.type_str("X");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Xhello world"));

    ctx.press(KeyCode::Char('i'));
    ctx.press(KeyCode::End);
    ctx.type_str("Y");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Xhello worldY"));
}

#[test]
fn ctrl_a_e_move_cursor_to_boundaries() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("test content");

    ctx.press_with_modifiers(KeyCode::Char('a'), KeyModifiers::CONTROL);
    ctx.type_str("X");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Xtest content"));

    ctx.press(KeyCode::Char('i'));
    ctx.press_with_modifiers(KeyCode::Char('e'), KeyModifiers::CONTROL);
    ctx.type_str("Y");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Xtest contentY"));
}

#[test]
fn ctrl_w_deletes_word_before_cursor() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("hello beautiful world");

    ctx.press_with_modifiers(KeyCode::Char('w'), KeyModifiers::CONTROL);
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("hello beautiful"));
    assert!(!ctx.screen_contains("world"));
}

#[test]
fn ctrl_u_deletes_to_line_start() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("hello world");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Char('i'));
    ctx.press(KeyCode::End);
    ctx.press_with_modifiers(KeyCode::Char('u'), KeyModifiers::CONTROL);
    ctx.type_str("new content");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("new content"));
    assert!(!ctx.screen_contains("hello"));
}

#[test]
fn ctrl_k_deletes_to_line_end() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("hello world");
    ctx.press(KeyCode::Home);
    for _ in 0..5 {
        ctx.press(KeyCode::Right);
    }
    ctx.press_with_modifiers(KeyCode::Char('k'), KeyModifiers::CONTROL);
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("hello"));
    assert!(!ctx.screen_contains("world"));
}

#[test]
fn tab_saves_and_starts_new_entry() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("First entry");
    ctx.press(KeyCode::Tab);

    assert!(matches!(ctx.app.input_mode, InputMode::Edit(_)));

    ctx.type_str("Second entry");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("First entry"));
    assert!(ctx.screen_contains("Second entry"));
}

#[test]
fn escape_cancels_edit_and_restores_original() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("Original content");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Char('i'));
    ctx.press_with_modifiers(KeyCode::Char('u'), KeyModifiers::CONTROL);
    ctx.type_str("Modified content");
    ctx.press(KeyCode::Esc);

    assert!(ctx.screen_contains("Original content"));
    assert!(!ctx.screen_contains("Modified content"));
}

#[test]
fn backtab_cycles_entry_type() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("Test entry");

    ctx.press(KeyCode::BackTab);
    ctx.press(KeyCode::BackTab);
    ctx.press(KeyCode::BackTab);
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("[ ]"));
}

#[test]
fn backtab_creates_note_type() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("Note entry");
    ctx.press(KeyCode::BackTab);
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Note entry"));
    let line = ctx.find_line("Note entry");
    assert!(line.map_or(true, |l| !l.contains("[ ]")));
}

#[test]
fn empty_entry_discarded_on_save() {
    let mut ctx = TestContext::new();

    let initial_count = ctx.app.entry_indices.len();

    ctx.press(KeyCode::Enter);
    ctx.press(KeyCode::Enter);

    assert_eq!(ctx.app.entry_indices.len(), initial_count);
}

#[test]
fn backspace_deletes_characters() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("hello");
    ctx.press(KeyCode::Backspace);
    ctx.press(KeyCode::Backspace);
    ctx.type_str("p!");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("help!"));
}

#[test]
fn cursor_position_decrements_after_backspace() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("hello");
    assert_eq!(ctx.cursor_position(), Some(5));

    ctx.press(KeyCode::Backspace);
    assert_eq!(ctx.cursor_position(), Some(4));

    ctx.press(KeyCode::Backspace);
    assert_eq!(ctx.cursor_position(), Some(3));
}

#[test]
fn cursor_stops_at_text_end() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("abc");
    assert_eq!(ctx.cursor_position(), Some(3));

    for _ in 0..5 {
        ctx.press(KeyCode::Right);
    }
    assert_eq!(ctx.cursor_position(), Some(3));
}

#[test]
fn cursor_stops_at_text_start() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("abc");
    ctx.press(KeyCode::Home);
    assert_eq!(ctx.cursor_position(), Some(0));

    for _ in 0..5 {
        ctx.press(KeyCode::Left);
    }
    assert_eq!(ctx.cursor_position(), Some(0));
}

#[test]
fn emoji_characters_handled_as_single_units() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("Task ");

    ctx.press(KeyCode::Char('🎉'));
    ctx.type_str(" done");
    assert_eq!(ctx.cursor_position(), Some(11));

    ctx.press(KeyCode::Backspace);
    ctx.press(KeyCode::Backspace);
    ctx.press(KeyCode::Backspace);
    ctx.press(KeyCode::Backspace);
    ctx.press(KeyCode::Backspace);
    ctx.press(KeyCode::Backspace);
    assert_eq!(ctx.cursor_position(), Some(5));

    ctx.press(KeyCode::Enter);
    assert!(ctx.screen_contains("Task"));
}

#[test]
fn cursor_tracks_position_in_long_text() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    let long_text = "This is a very long entry that will definitely wrap across multiple lines when displayed in the terminal interface";
    ctx.type_str(long_text);

    assert_eq!(ctx.cursor_position(), Some(long_text.len()));

    ctx.press(KeyCode::Home);
    assert_eq!(ctx.cursor_position(), Some(0));

    ctx.press_with_modifiers(KeyCode::Char('f'), KeyModifiers::ALT);
    assert!(ctx.cursor_position().unwrap() > 0);

    ctx.type_str("INSERTED");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("INSERTED"));
}
