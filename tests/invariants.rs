mod helpers;

use chrono::NaiveDate;
use crossterm::event::{KeyCode, KeyModifiers};
use helpers::TestContext;

use caliber::app::InputMode;

#[test]
fn selection_valid_after_delete_operations() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A\n- [ ] B\n- [ ] C\n- [ ] D\n- [ ] E\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    // Delete first entry
    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('d'));
    ctx.verify_invariants();

    // Delete middle entry
    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('d'));
    ctx.verify_invariants();

    // Delete last entry
    ctx.press(KeyCode::Char('G'));
    ctx.press(KeyCode::Char('d'));
    ctx.verify_invariants();

    // Delete remaining entries one by one
    while ctx.entry_count() > 0 {
        ctx.press(KeyCode::Char('d'));
        ctx.verify_invariants();
    }
}

#[test]
fn selection_valid_after_hide_completed() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A\n- [x] B\n- [ ] C\n- [x] D\n- [ ] E\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    // Select a completed entry
    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('j')); // On B (completed)
    ctx.verify_invariants();

    // Hide completed - selection should adjust to visible entry
    ctx.press(KeyCode::Char('z'));
    ctx.verify_invariants();

    // Navigate around while hidden
    ctx.press(KeyCode::Char('j'));
    ctx.verify_invariants();
    ctx.press(KeyCode::Char('G'));
    ctx.verify_invariants();
    ctx.press(KeyCode::Char('g'));
    ctx.verify_invariants();

    // Unhide
    ctx.press(KeyCode::Char('z'));
    ctx.verify_invariants();
}

#[test]
fn cursor_valid_through_all_edit_operations() {
    let mut ctx = TestContext::new();

    // Enter edit mode
    ctx.press(KeyCode::Enter);
    ctx.verify_invariants();

    // Type text
    ctx.type_str("hello world");
    ctx.verify_invariants();

    // Home/End
    ctx.press(KeyCode::Home);
    ctx.verify_invariants();
    ctx.press(KeyCode::End);
    ctx.verify_invariants();

    // Arrow keys at boundaries
    for _ in 0..20 {
        ctx.press(KeyCode::Left);
    }
    ctx.verify_invariants();
    for _ in 0..20 {
        ctx.press(KeyCode::Right);
    }
    ctx.verify_invariants();

    // Ctrl-A/E
    ctx.press_with_modifiers(KeyCode::Char('a'), KeyModifiers::CONTROL);
    ctx.verify_invariants();
    ctx.press_with_modifiers(KeyCode::Char('e'), KeyModifiers::CONTROL);
    ctx.verify_invariants();

    // Word movement
    ctx.press_with_modifiers(KeyCode::Char('b'), KeyModifiers::ALT);
    ctx.verify_invariants();
    ctx.press_with_modifiers(KeyCode::Char('f'), KeyModifiers::ALT);
    ctx.verify_invariants();

    // Backspace at start (should be no-op)
    ctx.press(KeyCode::Home);
    ctx.press(KeyCode::Backspace);
    ctx.verify_invariants();

    // Ctrl-W (delete word backward)
    ctx.press(KeyCode::End);
    ctx.press_with_modifiers(KeyCode::Char('w'), KeyModifiers::CONTROL);
    ctx.verify_invariants();

    // Ctrl-U (delete to start)
    ctx.press_with_modifiers(KeyCode::Char('u'), KeyModifiers::CONTROL);
    ctx.verify_invariants();

    // Type more and Ctrl-K (delete to end)
    ctx.type_str("new text");
    ctx.press(KeyCode::Home);
    ctx.press(KeyCode::Right);
    ctx.press_with_modifiers(KeyCode::Char('k'), KeyModifiers::CONTROL);
    ctx.verify_invariants();

    ctx.press(KeyCode::Enter);
    ctx.verify_invariants();
}

#[test]
fn escape_returns_to_normal_from_all_modes() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A\n- [ ] B\n- [ ] C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    // Edit mode
    ctx.press(KeyCode::Char('i'));
    assert!(matches!(ctx.app.input_mode, InputMode::Edit(_)));
    ctx.press(KeyCode::Esc);
    assert!(matches!(ctx.app.input_mode, InputMode::Normal));
    ctx.verify_invariants();

    // Reorder mode
    ctx.press(KeyCode::Char('r'));
    assert!(matches!(ctx.app.input_mode, InputMode::Reorder));
    ctx.press(KeyCode::Esc);
    assert!(matches!(ctx.app.input_mode, InputMode::Normal));
    ctx.verify_invariants();

    // Selection mode
    ctx.press(KeyCode::Char('v'));
    assert!(matches!(ctx.app.input_mode, InputMode::Selection(_)));
    ctx.press(KeyCode::Esc);
    assert!(matches!(ctx.app.input_mode, InputMode::Normal));
    ctx.verify_invariants();

    // Filter prompt mode
    ctx.press(KeyCode::Char('/'));
    assert!(matches!(ctx.app.input_mode, InputMode::FilterPrompt));
    ctx.press(KeyCode::Esc);
    assert!(matches!(ctx.app.input_mode, InputMode::Normal));
    ctx.verify_invariants();

    // Command palette mode (q key)
    ctx.press(KeyCode::Char('q'));
    assert!(matches!(ctx.app.input_mode, InputMode::CommandPalette(_)));
    ctx.press(KeyCode::Esc);
    assert!(matches!(ctx.app.input_mode, InputMode::Normal));
    ctx.verify_invariants();
}

#[test]
fn rendered_entries_exist_in_journal() {
    let mut ctx = TestContext::new();

    // Create several entries
    ctx.press(KeyCode::Enter);
    ctx.type_str("Entry one");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Enter);
    ctx.type_str("Entry two");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Enter);
    ctx.type_str("Entry three");
    ctx.press(KeyCode::Enter);

    // Verify each rendered entry exists in journal
    let journal = ctx.read_journal();
    let rendered = ctx.render_current();

    for line in &rendered {
        if line.contains("Entry one") {
            assert!(journal.contains("Entry one"), "Entry one not persisted");
        }
        if line.contains("Entry two") {
            assert!(journal.contains("Entry two"), "Entry two not persisted");
        }
        if line.contains("Entry three") {
            assert!(journal.contains("Entry three"), "Entry three not persisted");
        }
    }

    ctx.verify_invariants();
}
