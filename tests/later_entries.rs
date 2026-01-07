mod helpers;

use chrono::NaiveDate;
use crossterm::event::KeyCode;
use helpers::TestContext;

/// LE-1: Entry with @date appears in target day's Later section
#[test]
fn test_later_entry_appears_on_target_date() {
    let source_date = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
    let content = "# 2026/01/10\n- [ ] Review doc @01/15\n";
    let mut ctx = TestContext::with_journal_content(source_date, content);

    // Navigate to 1/15 (5 days forward)
    for _ in 0..5 {
        ctx.press(KeyCode::Char('l'));
    }

    // Entry should appear as later entry
    assert!(
        ctx.screen_contains("Review doc @01/15"),
        "Later entry should appear on target date"
    );
    // Should show source date indicator
    assert!(
        ctx.screen_contains("(01/10)"),
        "Source date indicator should appear"
    );
}

/// LE-3: Edit is blocked on projected (later) entries, shows status message
#[test]
fn test_edit_later_entry() {
    // Start viewing 1/15, with entry created on 1/10 targeting 1/15
    let view_date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/10\n- [ ] Original @01/15\n";
    let mut ctx = TestContext::with_journal_content(view_date, content);

    // We're on 1/15, should see later entry from 1/10
    assert!(
        ctx.screen_contains("Original @01/15"),
        "Later entry should be visible"
    );

    // Try to edit the later entry - should be blocked
    ctx.press(KeyCode::Char('i'));

    // Should show status message indicating how to go to source
    assert!(
        ctx.status_contains("Press o to go to source"),
        "Edit should be blocked with go-to-source hint"
    );

    // Journal should be unchanged (edit was blocked)
    let journal = ctx.read_journal();
    assert!(
        journal.contains("Original @01/15"),
        "Original entry should be unchanged"
    );
    assert!(
        !journal.contains("modified"),
        "Entry should not have been modified"
    );
}

/// LE-4: Toggle later entry completion
#[test]
fn test_toggle_later_entry() {
    let view_date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/10\n- [ ] Later task @01/15\n";
    let mut ctx = TestContext::with_journal_content(view_date, content);

    ctx.press(KeyCode::Char('c')); // Toggle completion

    let journal = ctx.read_journal();
    assert!(
        journal.contains("- [x] Later task @01/15"),
        "Later entry should be marked complete"
    );
}

/// LE-5: Delete is blocked on projected (later) entries, shows status message
#[test]
fn test_delete_later_entry() {
    let view_date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/10\n- [ ] Delete me @01/15\n- [ ] Keep me\n";
    let mut ctx = TestContext::with_journal_content(view_date, content);

    // Try to delete the later entry - should be blocked
    ctx.press(KeyCode::Char('d'));

    // Should show status message indicating how to go to source
    assert!(
        ctx.status_contains("Press o to go to source"),
        "Delete should be blocked with go-to-source hint"
    );

    // Journal should be unchanged (delete was blocked)
    let journal = ctx.read_journal();
    assert!(
        journal.contains("Delete me"),
        "Entry should not have been deleted"
    );
    assert!(journal.contains("Keep me"), "Other entry should remain");
}

/// LE-2: Natural date conversion (@tomorrow -> @MM/DD)
#[test]
fn test_natural_date_conversion() {
    // Use the actual current date for this test since natural date conversion
    // uses Local::now(), not the app's current_date
    let today = chrono::Local::now().date_naive();
    let tomorrow = today + chrono::Days::new(1);
    let expected_date = tomorrow.format("@%m/%d").to_string();

    let mut ctx = TestContext::with_date(today);

    // Create entry with natural date
    ctx.press(KeyCode::Enter);
    ctx.type_str("Call Bob @tomorrow");
    ctx.press(KeyCode::Enter);

    // Check journal - should have converted @tomorrow to tomorrow's actual date
    let journal = ctx.read_journal();
    assert!(
        journal.contains(&expected_date),
        "Natural date @tomorrow should convert to {}",
        expected_date
    );
}

/// LE-7: Overdue filter shows entries with past @dates
#[test]
fn test_overdue_filter() {
    // Create entries with past dates using actual today for proper filtering
    let today = chrono::Local::now().date_naive();
    let yesterday = today - chrono::Days::new(1);
    // Use MM/DD format for past date (will prefer past interpretation)
    let past_date = yesterday.format("@%m/%d").to_string();
    // Use explicit year for future date to avoid being interpreted as last year
    let future_date = (today + chrono::Days::new(5))
        .format("@%m/%d/%y")
        .to_string();

    // Create journal with entries that have past and future @dates
    let content = format!(
        "# {}\n- [ ] Past due task {}\n- [ ] Future task {}\n- [ ] No date task\n",
        today.format("%Y/%m/%d"),
        past_date,
        future_date
    );
    let mut ctx = TestContext::with_journal_content(today, &content);

    // Filter for @overdue
    ctx.press(KeyCode::Char('/'));
    ctx.type_str("@overdue");
    ctx.press(KeyCode::Enter);

    // Past due entry should appear
    assert!(
        ctx.screen_contains("Past due task"),
        "Overdue entry should appear in @overdue filter"
    );

    // Future and undated entries should not appear
    assert!(
        !ctx.screen_contains("Future task"),
        "Future entry should not appear in @overdue filter"
    );
    assert!(
        !ctx.screen_contains("No date task"),
        "Undated entry should not appear in @overdue filter"
    );
}

/// LE-6: @later filter shows entries with @date patterns
#[test]
fn test_later_filter() {
    let today = chrono::Local::now().date_naive();
    let future_date = (today + chrono::Days::new(5))
        .format("@%m/%d/%y")
        .to_string();

    let content = format!(
        "# {}\n- [ ] Scheduled task {}\n- [ ] Regular task\n- [ ] Recurring task @every-day\n",
        today.format("%Y/%m/%d"),
        future_date
    );
    let mut ctx = TestContext::with_journal_content(today, &content);

    // Filter for @later
    ctx.press(KeyCode::Char('/'));
    ctx.type_str("@later");
    ctx.press(KeyCode::Enter);

    // Later entry should appear
    assert!(
        ctx.screen_contains("Scheduled task"),
        "Later entry should appear in @later filter"
    );

    // Regular and recurring entries should not appear
    assert!(
        !ctx.screen_contains("Regular task"),
        "Regular entry should not appear in @later filter"
    );
    assert!(
        !ctx.screen_contains("Recurring task"),
        "Recurring entry should not appear in @later filter"
    );
}
