mod helpers;

use chrono::NaiveDate;
use crossterm::event::{KeyCode, KeyModifiers};
use helpers::TestContext;

use caliber::app::InputMode;

/// SM-1: Enter and exit selection mode
#[test]
fn test_enter_exit_selection_mode() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Entry A\n- [ ] Entry B\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    // Enter selection mode with 'v'
    ctx.press(KeyCode::Char('v'));
    assert!(
        matches!(ctx.app.input_mode, InputMode::Selection(_)),
        "Should enter selection mode"
    );

    // Exit with Esc
    ctx.press(KeyCode::Esc);
    assert!(
        matches!(ctx.app.input_mode, InputMode::Normal),
        "Should exit to normal mode"
    );
}

/// SM-2: Toggle selection with space (v key toggles)
#[test]
fn test_toggle_selection() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Entry A\n- [ ] Entry B\n- [ ] Entry C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g')); // Go to first
    ctx.press(KeyCode::Char('v')); // Enter selection mode

    // Initially at first entry, it should be selected
    let state = ctx.app.get_selection_state().unwrap();
    assert!(
        state.is_selected(0),
        "First entry should be initially selected"
    );
    assert_eq!(state.count(), 1);

    // Move down and toggle second entry
    ctx.press(KeyCode::Char('j')); // Move to second
    ctx.press(KeyCode::Char('v')); // Toggle selection on current

    let state = ctx.app.get_selection_state().unwrap();
    assert!(state.is_selected(0), "First should still be selected");
    assert!(state.is_selected(1), "Second should now be selected");
    assert_eq!(state.count(), 2);
}

/// SM-3: Range selection with Shift+V
#[test]
fn test_range_selection() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Entry A\n- [ ] Entry B\n- [ ] Entry C\n- [ ] Entry D\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g')); // Go to first
    ctx.press(KeyCode::Char('v')); // Enter selection mode at A

    // Move to last
    ctx.press(KeyCode::Char('G'));

    // Extend selection from anchor to cursor with Shift+V
    ctx.press_with_modifiers(KeyCode::Char('V'), KeyModifiers::SHIFT);

    let state = ctx.app.get_selection_state().unwrap();
    assert_eq!(state.count(), 4, "All entries should be selected via range");
    assert!(state.is_selected(0), "A should be selected");
    assert!(state.is_selected(1), "B should be selected");
    assert!(state.is_selected(2), "C should be selected");
    assert!(state.is_selected(3), "D should be selected");
}

/// SM-4: Batch delete
#[test]
fn test_batch_delete() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Keep\n- [ ] Delete A\n- [ ] Delete B\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    // Go to second entry
    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('j'));

    // Enter selection mode at "Delete A"
    ctx.press(KeyCode::Char('v'));

    // Move to "Delete B" and toggle to select it too
    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('v'));

    // Delete selected
    ctx.press(KeyCode::Char('d'));

    // Should be back in normal mode
    assert!(
        matches!(ctx.app.input_mode, InputMode::Normal),
        "Should exit selection mode after delete"
    );

    // Verify deletion
    let journal = ctx.read_journal();
    assert!(journal.contains("Keep"), "Keep should remain");
    assert!(!journal.contains("Delete A"), "Delete A should be removed");
    assert!(!journal.contains("Delete B"), "Delete B should be removed");
}

/// SM-5: Batch toggle completion
#[test]
fn test_batch_toggle() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Task A\n- [ ] Task B\n- Note\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    // Select first two entries
    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('v')); // Enter selection at A

    ctx.press(KeyCode::Char('j')); // Move to B
    ctx.press(KeyCode::Char('v')); // Toggle B into selection

    // Toggle selected (only tasks)
    ctx.press(KeyCode::Char('c'));

    // Verify both tasks toggled
    let journal = ctx.read_journal();
    assert!(journal.contains("[x] Task A"), "Task A should be completed");
    assert!(journal.contains("[x] Task B"), "Task B should be completed");
}

/// SM-6: Batch yank (verifies operation completes)
#[test]
fn test_batch_yank() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Entry A\n- [ ] Entry B\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('v')); // Enter selection

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('v')); // Select second

    ctx.press(KeyCode::Char('y')); // Yank

    // Should stay in selection mode (only delete exits)
    assert!(
        matches!(ctx.app.input_mode, InputMode::Selection(_)),
        "Should stay in selection mode after yank"
    );

    // Entries should still exist
    assert!(ctx.screen_contains("Entry A"));
    assert!(ctx.screen_contains("Entry B"));
}

/// SM-7: Selection mode with hidden completed entries
#[test]
fn test_selection_with_hidden_completed() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Incomplete A\n- [x] Completed\n- [ ] Incomplete B\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    // Hide completed
    ctx.press(KeyCode::Char('z'));

    // Now only 2 entries visible (Incomplete A and B)
    ctx.press(KeyCode::Char('g')); // Go to first (Incomplete A)
    ctx.press(KeyCode::Char('v')); // Enter selection

    ctx.press(KeyCode::Char('j')); // Move to Incomplete B (skipping hidden)
    ctx.press(KeyCode::Char('v')); // Toggle B into selection

    // Delete selected
    ctx.press(KeyCode::Char('d'));

    let journal = ctx.read_journal();
    assert!(
        !journal.contains("Incomplete A"),
        "Incomplete A should be deleted"
    );
    assert!(journal.contains("Completed"), "Completed should remain");
    assert!(
        !journal.contains("Incomplete B"),
        "Incomplete B should be deleted"
    );
}

/// SM-8: Remove last trailing tag from selected
#[test]
fn test_batch_remove_last_tag() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Entry A #tag1 #tag2\n- [ ] Entry B #tag3\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('v')); // Select A

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('v')); // Select B

    ctx.press(KeyCode::Char('x')); // Remove last tag

    let journal = ctx.read_journal();
    assert!(
        journal.contains("Entry A #tag1") && !journal.contains("#tag2"),
        "Only last tag should be removed from A"
    );
    assert!(!journal.contains("#tag3"), "Tag should be removed from B");
}

/// SM-9: Remove all trailing tags from selected
#[test]
fn test_batch_remove_all_tags() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Entry A #tag1 #tag2\n- [ ] Entry B #tag3\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('X')); // Remove all tags

    let journal = ctx.read_journal();
    assert!(
        !journal.contains("#tag1") && !journal.contains("#tag2"),
        "All tags should be removed from A"
    );
    assert!(
        !journal.contains("#tag3"),
        "All tags should be removed from B"
    );
}

/// SM-10: Selection mode in filter view
#[test]
fn test_selection_in_filter_view() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Task A\n- [ ] Task B\n- [ ] Task C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    // Enter filter mode
    ctx.press(KeyCode::Char('/'));
    ctx.type_str("!tasks");
    ctx.press(KeyCode::Enter);

    // Now in filter view
    ctx.press(KeyCode::Char('g')); // Go to first
    ctx.press(KeyCode::Char('v')); // Enter selection

    assert!(
        matches!(ctx.app.input_mode, InputMode::Selection(_)),
        "Should enter selection mode in filter view"
    );

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('v')); // Select second

    // Toggle complete
    ctx.press(KeyCode::Char('c'));

    let journal = ctx.read_journal();
    assert!(journal.contains("[x] Task A"), "Task A should be toggled");
    assert!(journal.contains("[x] Task B"), "Task B should be toggled");
    assert!(
        journal.contains("[ ] Task C"),
        "Task C should remain incomplete"
    );
}

/// SM-11: Navigation in selection mode
#[test]
fn test_selection_navigation() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A\n- [ ] B\n- [ ] C\n- [ ] D\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g')); // First
    ctx.press(KeyCode::Char('v')); // Enter selection

    // j/k navigation
    ctx.press(KeyCode::Char('j'));
    assert_eq!(ctx.selected_index(), 1, "Should move down with j");

    ctx.press(KeyCode::Char('k'));
    assert_eq!(ctx.selected_index(), 0, "Should move up with k");

    // g/G navigation
    ctx.press(KeyCode::Char('G'));
    assert_eq!(ctx.selected_index(), 3, "Should jump to last with G");

    ctx.press(KeyCode::Char('g'));
    assert_eq!(ctx.selected_index(), 0, "Should jump to first with g");

    // Still in selection mode
    assert!(matches!(ctx.app.input_mode, InputMode::Selection(_)));
}
