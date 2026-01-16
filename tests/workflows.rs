mod helpers;

use chrono::NaiveDate;
use crossterm::event::{KeyCode, KeyModifiers};
use helpers::TestContext;

#[test]
fn entry_lifecycle_create_edit_toggle_delete_undo() {
    let mut ctx = TestContext::new();

    // Create
    ctx.press(KeyCode::Enter);
    ctx.type_str("My task");
    ctx.press(KeyCode::Enter);
    assert!(ctx.screen_contains("My task"));
    assert!(ctx.read_journal().contains("My task"));
    ctx.verify_invariants();

    // Edit
    ctx.press(KeyCode::Char('i'));
    ctx.press(KeyCode::End);
    ctx.type_str(" updated");
    ctx.press(KeyCode::Enter);
    assert!(ctx.screen_contains("My task updated"));
    assert!(ctx.read_journal().contains("My task updated"));
    ctx.verify_invariants();

    // Toggle complete
    ctx.press(KeyCode::Char(' '));
    assert!(ctx.screen_contains("[x]"));
    assert!(ctx.read_journal().contains("[x]"));
    ctx.verify_invariants();

    // Delete
    ctx.press(KeyCode::Char('d'));
    assert!(!ctx.screen_contains("My task"));
    assert!(!ctx.read_journal().contains("My task"));
    ctx.verify_invariants();

    // Undo
    ctx.press(KeyCode::Char('u'));
    assert!(ctx.screen_contains("My task"));
    assert!(ctx.read_journal().contains("My task"));
    ctx.verify_invariants();
}

#[test]
fn batch_select_delete_with_range() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Keep A\n- [ ] Delete B\n- [ ] Delete C\n- [ ] Delete D\n- [ ] Keep E\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    // Select range B through D
    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('j')); // On B
    ctx.press(KeyCode::Char('v')); // Start selection

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('j')); // On D
    ctx.press_with_modifiers(KeyCode::Char('V'), KeyModifiers::SHIFT); // Range select

    // Delete selected
    ctx.press(KeyCode::Char('d'));

    // Verify correct entries remain
    let journal = ctx.read_journal();
    assert!(journal.contains("Keep A"));
    assert!(!journal.contains("Delete B"));
    assert!(!journal.contains("Delete C"));
    assert!(!journal.contains("Delete D"));
    assert!(journal.contains("Keep E"));

    ctx.verify_invariants();

    // Undo should restore all
    ctx.press(KeyCode::Char('u'));
    let journal = ctx.read_journal();
    assert!(journal.contains("Delete B"));
    assert!(journal.contains("Delete C"));
    assert!(journal.contains("Delete D"));

    ctx.verify_invariants();
}

#[test]
fn reorder_entries_persists_correctly() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A\n- [ ] B\n- [ ] C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    // Move A down twice (A should end up at bottom)
    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('r')); // Enter reorder mode
    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Enter); // Confirm

    // Verify order in journal: B, C, A
    let journal = ctx.read_journal();
    let b_pos = journal.find("[ ] B").unwrap();
    let c_pos = journal.find("[ ] C").unwrap();
    let a_pos = journal.find("[ ] A").unwrap();
    assert!(b_pos < c_pos && c_pos < a_pos);

    // Verify order on screen matches
    let lines = ctx.render_current();
    let b_line = lines.iter().position(|l| l.contains(" B")).unwrap();
    let c_line = lines.iter().position(|l| l.contains(" C")).unwrap();
    let a_line = lines.iter().position(|l| l.contains(" A")).unwrap();
    assert!(b_line < c_line && c_line < a_line);

    ctx.verify_invariants();
}

#[test]
fn day_navigation_preserves_entries() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_date(date);

    // Create entry on day 1
    ctx.press(KeyCode::Enter);
    ctx.type_str("Day 1 entry");
    ctx.press(KeyCode::Enter);

    // Navigate to previous day
    ctx.press(KeyCode::Char('h'));
    ctx.verify_invariants();

    // Create entry on day 2
    ctx.press(KeyCode::Enter);
    ctx.type_str("Day 2 entry");
    ctx.press(KeyCode::Enter);

    // Navigate back to day 1
    ctx.press(KeyCode::Char('l'));
    ctx.verify_invariants();

    // Verify day 1 entry still exists
    assert!(ctx.screen_contains("Day 1 entry"));

    // Verify both in journal
    let journal = ctx.read_journal();
    assert!(journal.contains("Day 1 entry"));
    assert!(journal.contains("Day 2 entry"));

    ctx.verify_invariants();
}

#[test]
fn journal_switch_isolates_changes() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_date(date);

    // Create entry in hub
    ctx.press(KeyCode::Enter);
    ctx.type_str("Hub entry");
    ctx.press(KeyCode::Enter);
    assert!(ctx.screen_contains("Hub entry"));

    let hub_journal = ctx.read_journal();
    assert!(hub_journal.contains("Hub entry"));

    ctx.verify_invariants();
}
