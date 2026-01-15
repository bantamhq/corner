mod helpers;

use crossterm::event::KeyCode;
use helpers::TestContext;

#[test]
fn all_operations_safe_on_empty_journal() {
    let mut ctx = TestContext::new();

    // Navigation should not crash
    ctx.press(KeyCode::Char('j'));
    ctx.verify_invariants();
    ctx.press(KeyCode::Char('k'));
    ctx.verify_invariants();
    ctx.press(KeyCode::Char('g'));
    ctx.verify_invariants();
    ctx.press(KeyCode::Char('G'));
    ctx.verify_invariants();

    // Delete should not crash
    ctx.press(KeyCode::Char('d'));
    ctx.verify_invariants();

    // Toggle should not crash
    ctx.press(KeyCode::Char(' '));
    ctx.verify_invariants();

    // Edit mode enter and exit
    ctx.press(KeyCode::Char('i'));
    ctx.press(KeyCode::Esc);
    ctx.verify_invariants();

    // Create and immediately cancel
    ctx.press(KeyCode::Enter);
    ctx.press(KeyCode::Esc);
    ctx.verify_invariants();

    // Verify still empty
    assert_eq!(ctx.entry_count(), 0);
}

#[test]
fn single_entry_delete_and_recreation() {
    let mut ctx = TestContext::new();

    // Create one entry
    ctx.press(KeyCode::Enter);
    ctx.type_str("Single entry");
    ctx.press(KeyCode::Enter);
    assert_eq!(ctx.entry_count(), 1);
    ctx.verify_invariants();

    // Delete it
    ctx.press(KeyCode::Char('d'));
    assert_eq!(ctx.entry_count(), 0);
    ctx.verify_invariants();

    // Create new entry
    ctx.press(KeyCode::Enter);
    ctx.type_str("New entry");
    ctx.press(KeyCode::Enter);
    assert_eq!(ctx.entry_count(), 1);
    assert!(ctx.screen_contains("New entry"));

    ctx.verify_invariants();
}

#[test]
fn unicode_emoji_cursor_handling() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);

    // Type mixed content with emoji
    ctx.type_str("Task ");
    ctx.press(KeyCode::Char('🎉'));
    ctx.type_str(" done");

    // Cursor should count characters, not bytes
    // "Task 🎉 done" = 11 characters
    assert_eq!(ctx.cursor_position(), Some(11));

    // Backspace should delete one character at a time
    ctx.press(KeyCode::Backspace); // removes 'e'
    ctx.press(KeyCode::Backspace); // removes 'n'
    ctx.press(KeyCode::Backspace); // removes 'o'
    ctx.press(KeyCode::Backspace); // removes 'd'
    ctx.press(KeyCode::Backspace); // removes ' '
    ctx.press(KeyCode::Backspace); // removes '🎉'
    assert_eq!(ctx.cursor_position(), Some(5)); // "Task "

    ctx.press(KeyCode::Enter);
    assert!(ctx.screen_contains("Task"));

    ctx.verify_invariants();
}

#[test]
fn entry_types_round_trip_through_persist() {
    let mut ctx = TestContext::new();

    // Create task (default)
    ctx.press(KeyCode::Enter);
    ctx.type_str("A task");
    ctx.press(KeyCode::Enter);

    // Create note (BackTab once)
    ctx.press(KeyCode::Enter);
    ctx.type_str("A note");
    ctx.press(KeyCode::BackTab);
    ctx.press(KeyCode::Enter);

    // Create event (BackTab twice from task)
    ctx.press(KeyCode::Enter);
    ctx.type_str("An event");
    ctx.press(KeyCode::BackTab);
    ctx.press(KeyCode::BackTab);
    ctx.press(KeyCode::Enter);

    // Verify journal has all types
    let journal = ctx.read_journal();
    assert!(journal.contains("- [ ] A task"));
    assert!(journal.contains("- A note"));
    assert!(journal.contains("* An event"));

    // Verify screen shows correct markers
    assert!(ctx.screen_contains("[ ] A task"));

    ctx.verify_invariants();
}
