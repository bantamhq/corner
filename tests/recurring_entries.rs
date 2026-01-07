mod helpers;

use chrono::NaiveDate;
use crossterm::event::KeyCode;
use helpers::TestContext;

/// RE-1: Entry with @every-day appears on all days
#[test]
fn test_recurring_daily_appears_every_day() {
    let source_date = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
    let content = "# 2026/01/10\n- [ ] Stand-up meeting @every-day\n";
    let mut ctx = TestContext::with_journal_content(source_date, content);

    // Navigate forward 3 days
    for _ in 0..3 {
        ctx.press(KeyCode::Char('l'));
    }

    // Entry should appear as recurring entry
    assert!(
        ctx.screen_contains("Stand-up meeting"),
        "Recurring entry should appear on target date"
    );
    // Should show source date indicator
    assert!(
        ctx.screen_contains("(01/10)"),
        "Source date indicator should appear"
    );
}

/// RE-2: Entry with @every-weekday only appears on weekdays
#[test]
fn test_recurring_weekday_appears_on_weekdays() {
    // Start on Monday 2026-01-12
    let monday = NaiveDate::from_ymd_opt(2026, 1, 12).unwrap();
    let content = "# 2026/01/01\n- [ ] Weekday task @every-weekday\n";
    let mut ctx = TestContext::with_journal_content(monday, content);

    // Should appear on Monday
    assert!(
        ctx.screen_contains("Weekday task"),
        "Recurring entry should appear on Monday"
    );

    // Navigate to Saturday (5 days forward)
    for _ in 0..5 {
        ctx.press(KeyCode::Char('l'));
    }

    // Should NOT appear on Saturday
    assert!(
        !ctx.screen_contains("Weekday task"),
        "Recurring entry should not appear on Saturday"
    );
}

/// RE-3: Entry with @every-monday only appears on Mondays
#[test]
fn test_recurring_weekly_appears_on_day() {
    // Start on Monday 2026-01-12
    let monday = NaiveDate::from_ymd_opt(2026, 1, 12).unwrap();
    let content = "# 2026/01/01\n- [ ] Weekly review @every-mon\n";
    let mut ctx = TestContext::with_journal_content(monday, content);

    // Should appear on Monday
    assert!(
        ctx.screen_contains("Weekly review"),
        "Recurring entry should appear on Monday"
    );

    // Navigate to Tuesday (1 day forward)
    ctx.press(KeyCode::Char('l'));

    // Should NOT appear on Tuesday
    assert!(
        !ctx.screen_contains("Weekly review"),
        "Recurring entry should not appear on Tuesday"
    );
}

/// RE-4: Toggle on recurring entry materializes completed copy
#[test]
fn test_toggle_recurring_materializes_completed() {
    // We're viewing today with a recurring entry from the past
    let today = chrono::Local::now().date_naive();
    let past = today - chrono::Days::new(7);
    let content = format!(
        "# {}\n- [ ] Daily task @every-day\n",
        past.format("%Y/%m/%d")
    );
    let mut ctx = TestContext::with_journal_content(today, &content);

    // Toggle the recurring entry
    ctx.press(KeyCode::Char('c'));

    // Check journal - should have materialized completed entry on today
    let journal = ctx.read_journal();
    let today_section = today.format("# %Y/%m/%d").to_string();

    // Original recurring entry should still exist
    assert!(
        journal.contains("- [ ] Daily task @every-day"),
        "Original recurring entry should remain"
    );

    // A completed copy should be added to today with ↺ prefix
    assert!(
        journal.contains(&today_section),
        "Today's section should exist"
    );
    assert!(
        journal.contains("- [x] ↺ Daily task"),
        "Completed copy should be added to today with ↺ prefix"
    );

    // Recurring entry should be hidden from view (not shown in projected entries)
    assert!(
        !ctx.screen_contains("↺ Daily task @every-day"),
        "Recurring entry should be hidden when done-today copy exists"
    );
}

/// RE-4b: Recurring entry hidden when done-today copy exists on initial load
#[test]
fn test_recurring_hidden_when_done_today_exists() {
    let today = chrono::Local::now().date_naive();
    let past = today - chrono::Days::new(7);

    // Journal has both: recurring entry from past, and completed ↺ entry from today
    let content = format!(
        "# {past}\n- [ ] Daily task @every-day\n\n# {today}\n- [x] ↺ Daily task\n",
        past = past.format("%Y/%m/%d"),
        today = today.format("%Y/%m/%d")
    );
    let ctx = TestContext::with_journal_content(today, &content);

    // The recurring entry should NOT appear (hidden due to done-today match)
    assert!(
        !ctx.screen_contains("Daily task @every-day"),
        "Recurring entry should be hidden when done-today copy exists"
    );

    // The completed ↺ entry SHOULD appear
    assert!(
        ctx.screen_contains("↺ Daily task"),
        "Completed ↺ entry should be visible"
    );
}

/// RE-4c: Deleting done-today entry causes recurring to reappear
#[test]
fn test_recurring_reappears_after_deleting_done_today() {
    let today = chrono::Local::now().date_naive();
    let past = today - chrono::Days::new(7);

    // Journal has both: recurring entry from past, and completed ↺ entry from today
    let content = format!(
        "# {past}\n- [ ] Daily task @every-day\n\n# {today}\n- [x] ↺ Daily task\n",
        past = past.format("%Y/%m/%d"),
        today = today.format("%Y/%m/%d")
    );
    let mut ctx = TestContext::with_journal_content(today, &content);

    // Initially recurring is hidden
    assert!(
        !ctx.screen_contains("Daily task @every-day"),
        "Recurring should be hidden initially"
    );

    // Delete the ↺ entry (it should be selected since it's the only local entry)
    ctx.press(KeyCode::Char('d'));

    // Now recurring should reappear
    assert!(
        ctx.screen_contains("Daily task @every-day"),
        "Recurring entry should reappear after deleting done-today copy"
    );
}

/// RE-5: Edit is blocked on recurring entries
#[test]
fn test_edit_blocked_on_recurring() {
    let today = chrono::Local::now().date_naive();
    let past = today - chrono::Days::new(1);
    let content = format!(
        "# {}\n- [ ] Daily task @every-day\n",
        past.format("%Y/%m/%d")
    );
    let mut ctx = TestContext::with_journal_content(today, &content);

    // Try to edit the recurring entry
    ctx.press(KeyCode::Char('i'));

    // Should show status message
    assert!(
        ctx.status_contains("Press o to go to source"),
        "Edit should be blocked with go-to-source hint"
    );
}

/// RE-6: Delete is blocked on recurring entries
#[test]
fn test_delete_blocked_on_recurring() {
    let today = chrono::Local::now().date_naive();
    let past = today - chrono::Days::new(1);
    let content = format!(
        "# {}\n- [ ] Daily task @every-day\n",
        past.format("%Y/%m/%d")
    );
    let mut ctx = TestContext::with_journal_content(today, &content);

    // Try to delete the recurring entry
    ctx.press(KeyCode::Char('d'));

    // Should show status message
    assert!(
        ctx.status_contains("Press o to go to source"),
        "Delete should be blocked with go-to-source hint"
    );

    // Journal should be unchanged
    let journal = ctx.read_journal();
    assert!(
        journal.contains("Daily task @every-day"),
        "Entry should not have been deleted"
    );
}

/// RE-7: @recurring filter shows recurring entries
#[test]
fn test_recurring_filter() {
    let today = chrono::Local::now().date_naive();
    let content = format!(
        "# {}\n- [ ] Daily task @every-day\n- [ ] One-time task @01/15\n- [ ] Regular task\n",
        today.format("%Y/%m/%d")
    );
    let mut ctx = TestContext::with_journal_content(today, &content);

    // Filter for @recurring
    ctx.press(KeyCode::Char('/'));
    ctx.type_str("@recurring");
    ctx.press(KeyCode::Enter);

    // Recurring entry should appear
    assert!(
        ctx.screen_contains("Daily task @every-day"),
        "Recurring entry should appear in @recurring filter"
    );

    // One-time and regular entries should not appear
    assert!(
        !ctx.screen_contains("One-time task"),
        "Later entry should not appear in @recurring filter"
    );
    assert!(
        !ctx.screen_contains("Regular task"),
        "Regular entry should not appear in @recurring filter"
    );
}

/// RE-8: Entry with @every-15 appears on 15th of each month
#[test]
fn test_recurring_monthly_appears_on_day() {
    // Source date is Jan 1, we'll view Jan 15
    let view_date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/01\n- [ ] Monthly review @every-15\n";
    let mut ctx = TestContext::with_journal_content(view_date, content);

    // Should appear on the 15th
    assert!(
        ctx.screen_contains("Monthly review"),
        "Monthly recurring entry should appear on the 15th"
    );
    assert!(
        ctx.screen_contains("(01/01)"),
        "Source date indicator should appear"
    );

    // Navigate to the 16th
    ctx.press(KeyCode::Char('l'));

    // Should NOT appear on the 16th
    assert!(
        !ctx.screen_contains("Monthly review"),
        "Monthly recurring entry should not appear on the 16th"
    );

    // Navigate back to 15th and forward to Feb 15 (30 days total)
    ctx.press(KeyCode::Char('h')); // back to 15th
    for _ in 0..31 {
        ctx.press(KeyCode::Char('l'));
    }

    // Should appear on Feb 15
    assert!(
        ctx.screen_contains("Monthly review"),
        "Monthly recurring entry should appear on Feb 15th"
    );
}

/// RE-9: @every-31 falls back to last day of month in short months
#[test]
fn test_recurring_monthly_fallback_to_last_day() {
    // Feb 2026 has 28 days (not a leap year)
    let feb_28 = NaiveDate::from_ymd_opt(2026, 2, 28).unwrap();
    let content = "# 2026/01/01\n- [ ] End of month task @every-31\n";
    let mut ctx = TestContext::with_journal_content(feb_28, content);

    // Should appear on Feb 28 (last day of Feb)
    assert!(
        ctx.screen_contains("End of month task"),
        "Monthly @every-31 should fall back to Feb 28"
    );

    // Navigate to Feb 27
    ctx.press(KeyCode::Char('h'));

    // Should NOT appear on Feb 27
    assert!(
        !ctx.screen_contains("End of month task"),
        "Monthly @every-31 should not appear on Feb 27"
    );
}

/// RE-10: Go-to-source (o key) navigates from recurring entry to source
#[test]
fn test_go_to_source_on_recurring() {
    // Use fixed dates for reliable testing
    let source_date = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
    let view_date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/10\n- [ ] Daily task @every-day\n";
    let mut ctx = TestContext::with_journal_content(view_date, content);

    // Verify we're viewing 1/15 with the projected entry
    assert!(
        ctx.screen_contains("Daily task"),
        "Recurring entry should be visible"
    );
    // Should show source date indicator since it's a projected entry
    assert!(
        ctx.screen_contains("(01/10)"),
        "Should show source date indicator on projected view"
    );

    // Press 'o' to go to source
    ctx.press(KeyCode::Char('o'));

    // Verify we navigated to source date
    assert_eq!(
        ctx.app.current_date, source_date,
        "Should have navigated to source date"
    );

    // Entry should now be a regular entry (not projected)
    // Should NOT show source date indicator anymore
    assert!(
        ctx.screen_contains("Daily task @every-day"),
        "Entry should be visible on source date"
    );
    assert!(
        !ctx.screen_contains("(01/10)"),
        "Should not show source date indicator on source date"
    );
}

/// RE-11: Recurring entry on its source date appears as regular entry
#[test]
fn test_recurring_on_source_date_is_regular() {
    // View the same date where the recurring entry is defined
    let source_date = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
    let content = "# 2026/01/10\n- [ ] Daily task @every-day\n";
    let mut ctx = TestContext::with_journal_content(source_date, content);

    // Entry should appear as a regular entry (not projected)
    assert!(
        ctx.screen_contains("Daily task @every-day"),
        "Entry should be visible"
    );

    // Should NOT show source date suffix (since it's the source date)
    assert!(
        !ctx.screen_contains("(01/10)"),
        "Should not show source date indicator on source date"
    );

    // Edit should work (not blocked)
    ctx.press(KeyCode::Char('i'));

    // Should NOT show the "go to source" message
    assert!(
        !ctx.status_contains("Press o to go to source"),
        "Edit should not be blocked on source date"
    );
}
