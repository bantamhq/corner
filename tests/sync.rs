mod helpers;

use chrono::NaiveDate;
use crossterm::event::KeyCode;
use helpers::TestContext;

#[test]
fn delete_last_entry_adjusts_selection() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A\n- [ ] B\n- [ ] C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    // Go to last entry
    ctx.press(KeyCode::Char('G'));
    assert_eq!(ctx.selected_index(), 2);

    // Delete it
    ctx.press(KeyCode::Char('d'));

    // Selection should now be on new last entry
    assert_eq!(ctx.selected_index(), 1);
    assert_eq!(ctx.entry_count(), 2);

    // The selected entry should be B
    let lines = ctx.render_current();
    assert!(lines.iter().any(|l| l.starts_with("→") && l.contains(" B")));

    ctx.verify_invariants();
}

#[test]
fn delete_only_entry_leaves_valid_state() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Only entry\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    assert_eq!(ctx.entry_count(), 1);

    // Delete the only entry
    ctx.press(KeyCode::Char('d'));

    // Should be empty but valid
    assert_eq!(ctx.entry_count(), 0);
    assert_eq!(ctx.selected_index(), 0);

    // Navigation should not crash
    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('k'));
    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('G'));

    ctx.verify_invariants();
}

#[test]
fn hide_completed_skips_hidden_in_navigation() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A\n- [x] B\n- [ ] C\n- [x] D\n- [ ] E\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    // Hide completed
    ctx.press(KeyCode::Char('z'));

    // Navigate down from A
    ctx.press(KeyCode::Char('g'));
    let lines = ctx.render_current();
    assert!(lines.iter().any(|l| l.starts_with("→") && l.contains(" A")));

    ctx.press(KeyCode::Char('j'));
    let lines = ctx.render_current();
    // Should skip B and land on C
    assert!(lines.iter().any(|l| l.starts_with("→") && l.contains(" C")));

    ctx.press(KeyCode::Char('j'));
    let lines = ctx.render_current();
    // Should skip D and land on E
    assert!(lines.iter().any(|l| l.starts_with("→") && l.contains(" E")));

    ctx.verify_invariants();
}

#[test]
fn scroll_keeps_selection_visible() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut content = "# 2026/01/15\n".to_string();
    for i in 1..=50 {
        content.push_str(&format!("- [ ] Entry {}\n", i));
    }
    let mut ctx = TestContext::with_journal_content(date, &content);

    // Jump to bottom
    ctx.press(KeyCode::Char('G'));
    assert_eq!(ctx.selected_index(), 49);

    // Entry 50 should be visible
    assert!(ctx.screen_contains("Entry 50"));

    // Jump to top
    ctx.press(KeyCode::Char('g'));
    assert_eq!(ctx.selected_index(), 0);

    // Entry 1 should be visible
    assert!(ctx.screen_contains("Entry 1"));

    ctx.verify_invariants();
}

#[test]
fn later_entry_edit_syncs_to_source() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 20).unwrap();
    let content = "# 2026/01/15\n- [ ] Later task @01/20\n# 2026/01/20\n- [ ] Local task\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    // The later entry should appear on 01/20
    assert!(ctx.screen_contains("Later task"));

    // Toggle the later entry (simpler than editing)
    ctx.press(KeyCode::Char('g')); // Go to first (later entry)
    ctx.press(KeyCode::Char(' ')); // Toggle complete

    // Verify the source day (01/15) has the toggle
    let journal = ctx.read_journal();
    assert!(journal.contains("[x] Later task"));

    // The change should be in the 01/15 section
    let idx_15 = journal.find("# 2026/01/15").unwrap();
    let idx_20 = journal.find("# 2026/01/20").unwrap();
    let idx_toggled = journal.find("[x] Later task").unwrap();
    assert!(idx_toggled > idx_15 && idx_toggled < idx_20);

    ctx.verify_invariants();
}
