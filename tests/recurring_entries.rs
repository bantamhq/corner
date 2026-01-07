mod helpers;

use chrono::NaiveDate;
use crossterm::event::KeyCode;
use helpers::TestContext;

#[test]
fn recurring_daily_appears_every_day() {
    let source_date = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
    let content = "# 2026/01/10\n- [ ] Stand-up meeting @every-day\n";
    let mut ctx = TestContext::with_journal_content(source_date, content);

    for _ in 0..3 {
        ctx.press(KeyCode::Char('l'));
    }

    assert!(ctx.screen_contains("Stand-up meeting"));
    assert!(ctx.screen_contains("(01/10)"));
}

#[test]
fn recurring_weekday_appears_only_on_weekdays() {
    let monday = NaiveDate::from_ymd_opt(2026, 1, 12).unwrap();
    let content = "# 2026/01/01\n- [ ] Weekday task @every-weekday\n";
    let mut ctx = TestContext::with_journal_content(monday, content);

    assert!(ctx.screen_contains("Weekday task"));

    for _ in 0..5 {
        ctx.press(KeyCode::Char('l'));
    }

    assert!(!ctx.screen_contains("Weekday task"));
}

#[test]
fn recurring_weekly_appears_on_matching_day() {
    let monday = NaiveDate::from_ymd_opt(2026, 1, 12).unwrap();
    let content = "# 2026/01/01\n- [ ] Weekly review @every-mon\n";
    let mut ctx = TestContext::with_journal_content(monday, content);

    assert!(ctx.screen_contains("Weekly review"));

    ctx.press(KeyCode::Char('l'));

    assert!(!ctx.screen_contains("Weekly review"));
}

#[test]
fn toggle_materializes_completed_recurring_entry() {
    let today = chrono::Local::now().date_naive();
    let past = today - chrono::Days::new(7);
    let content = format!(
        "# {}\n- [ ] Daily task @every-day\n",
        past.format("%Y/%m/%d")
    );
    let mut ctx = TestContext::with_journal_content(today, &content);

    ctx.press(KeyCode::Char('c'));

    let journal = ctx.read_journal();
    let today_section = today.format("# %Y/%m/%d").to_string();

    assert!(journal.contains("- [ ] Daily task @every-day"));

    assert!(journal.contains(&today_section));
    assert!(journal.contains("- [x] ↺ Daily task"));

    assert!(!ctx.screen_contains("↺ Daily task @every-day"));
}

#[test]
fn recurring_hidden_when_completed_today() {
    let today = chrono::Local::now().date_naive();
    let past = today - chrono::Days::new(7);

    let content = format!(
        "# {past}\n- [ ] Daily task @every-day\n\n# {today}\n- [x] ↺ Daily task\n",
        past = past.format("%Y/%m/%d"),
        today = today.format("%Y/%m/%d")
    );
    let ctx = TestContext::with_journal_content(today, &content);

    assert!(!ctx.screen_contains("Daily task @every-day"));

    assert!(ctx.screen_contains("↺ Daily task"));
}

#[test]
fn recurring_reappears_after_deleting_completion() {
    let today = chrono::Local::now().date_naive();
    let past = today - chrono::Days::new(7);

    let content = format!(
        "# {past}\n- [ ] Daily task @every-day\n\n# {today}\n- [x] ↺ Daily task\n",
        past = past.format("%Y/%m/%d"),
        today = today.format("%Y/%m/%d")
    );
    let mut ctx = TestContext::with_journal_content(today, &content);

    assert!(!ctx.screen_contains("Daily task @every-day"));

    ctx.press(KeyCode::Char('d'));

    assert!(ctx.screen_contains("Daily task @every-day"));
}

#[test]
fn edit_blocked_on_recurring_with_hint() {
    let today = chrono::Local::now().date_naive();
    let past = today - chrono::Days::new(1);
    let content = format!(
        "# {}\n- [ ] Daily task @every-day\n",
        past.format("%Y/%m/%d")
    );
    let mut ctx = TestContext::with_journal_content(today, &content);

    ctx.press(KeyCode::Char('i'));

    assert!(ctx.status_contains("Press o to go to source"));
}

#[test]
fn delete_blocked_on_recurring_with_hint() {
    let today = chrono::Local::now().date_naive();
    let past = today - chrono::Days::new(1);
    let content = format!(
        "# {}\n- [ ] Daily task @every-day\n",
        past.format("%Y/%m/%d")
    );
    let mut ctx = TestContext::with_journal_content(today, &content);

    ctx.press(KeyCode::Char('d'));

    assert!(ctx.status_contains("Press o to go to source"));

    let journal = ctx.read_journal();
    assert!(journal.contains("Daily task @every-day"));
}

#[test]
fn recurring_filter_shows_only_recurring_entries() {
    let today = chrono::Local::now().date_naive();
    let content = format!(
        "# {}\n- [ ] Daily task @every-day\n- [ ] One-time task @01/15\n- [ ] Regular task\n",
        today.format("%Y/%m/%d")
    );
    let mut ctx = TestContext::with_journal_content(today, &content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("@recurring");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Daily task @every-day"));

    assert!(!ctx.screen_contains("One-time task"));
    assert!(!ctx.screen_contains("Regular task"));
}

#[test]
fn recurring_monthly_appears_on_matching_day() {
    let view_date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/01\n- [ ] Monthly review @every-15\n";
    let mut ctx = TestContext::with_journal_content(view_date, content);

    assert!(ctx.screen_contains("Monthly review"));
    assert!(ctx.screen_contains("(01/01)"));

    ctx.press(KeyCode::Char('l'));

    assert!(!ctx.screen_contains("Monthly review"));

    ctx.press(KeyCode::Char('h'));
    for _ in 0..31 {
        ctx.press(KeyCode::Char('l'));
    }

    assert!(ctx.screen_contains("Monthly review"));
}

#[test]
fn recurring_monthly_falls_back_to_last_day() {
    let feb_28 = NaiveDate::from_ymd_opt(2026, 2, 28).unwrap();
    let content = "# 2026/01/01\n- [ ] End of month task @every-31\n";
    let mut ctx = TestContext::with_journal_content(feb_28, content);

    assert!(ctx.screen_contains("End of month task"));

    ctx.press(KeyCode::Char('h'));

    assert!(!ctx.screen_contains("End of month task"));
}

#[test]
fn o_key_navigates_to_recurring_source() {
    let source_date = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
    let view_date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/10\n- [ ] Daily task @every-day\n";
    let mut ctx = TestContext::with_journal_content(view_date, content);

    assert!(ctx.screen_contains("Daily task"));
    assert!(ctx.screen_contains("(01/10)"));

    ctx.press(KeyCode::Char('o'));

    assert_eq!(ctx.app.current_date, source_date);

    assert!(ctx.screen_contains("Daily task @every-day"));
    assert!(!ctx.screen_contains("(01/10)"));
}

#[test]
fn recurring_on_source_date_behaves_as_regular() {
    let source_date = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
    let content = "# 2026/01/10\n- [ ] Daily task @every-day\n";
    let mut ctx = TestContext::with_journal_content(source_date, content);

    assert!(ctx.screen_contains("Daily task @every-day"));

    assert!(!ctx.screen_contains("(01/10)"));

    ctx.press(KeyCode::Char('i'));

    assert!(!ctx.status_contains("Press o to go to source"));
}
