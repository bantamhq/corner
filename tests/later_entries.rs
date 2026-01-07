mod helpers;

use chrono::NaiveDate;
use crossterm::event::KeyCode;
use helpers::TestContext;

#[test]
fn later_entry_appears_on_target_date() {
    let source_date = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
    let content = "# 2026/01/10\n- [ ] Review doc @01/15\n";
    let mut ctx = TestContext::with_journal_content(source_date, content);

    for _ in 0..5 {
        ctx.press(KeyCode::Char('l'));
    }

    assert!(ctx.screen_contains("Review doc @01/15"));
    assert!(ctx.screen_contains("(01/10)"));
}

#[test]
fn edit_blocked_on_later_entry_with_hint() {
    let view_date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/10\n- [ ] Original @01/15\n";
    let mut ctx = TestContext::with_journal_content(view_date, content);

    assert!(ctx.screen_contains("Original @01/15"));

    ctx.press(KeyCode::Char('i'));

    assert!(ctx.status_contains("Press o to go to source"));

    let journal = ctx.read_journal();
    assert!(journal.contains("Original @01/15"));
    assert!(!journal.contains("modified"));
}

#[test]
fn toggle_completes_later_entry_in_source() {
    let view_date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/10\n- [ ] Later task @01/15\n";
    let mut ctx = TestContext::with_journal_content(view_date, content);

    ctx.press(KeyCode::Char('c'));

    let journal = ctx.read_journal();
    assert!(journal.contains("- [x] Later task @01/15"));
}

#[test]
fn delete_blocked_on_later_entry_with_hint() {
    let view_date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/10\n- [ ] Delete me @01/15\n- [ ] Keep me\n";
    let mut ctx = TestContext::with_journal_content(view_date, content);

    ctx.press(KeyCode::Char('d'));

    assert!(ctx.status_contains("Press o to go to source"));

    let journal = ctx.read_journal();
    assert!(journal.contains("Delete me"));
    assert!(journal.contains("Keep me"));
}

#[test]
fn natural_date_converts_on_save() {
    let today = chrono::Local::now().date_naive();
    let tomorrow = today + chrono::Days::new(1);
    let expected_date = tomorrow.format("@%m/%d").to_string();

    let mut ctx = TestContext::with_date(today);

    ctx.press(KeyCode::Enter);
    ctx.type_str("Call Bob @tomorrow");
    ctx.press(KeyCode::Enter);

    let journal = ctx.read_journal();
    assert!(
        journal.contains(&expected_date),
        "Natural date @tomorrow should convert to {}",
        expected_date
    );
}

#[test]
fn overdue_filter_shows_past_due_entries() {
    let today = chrono::Local::now().date_naive();
    let yesterday = today - chrono::Days::new(1);
    let past_date = yesterday.format("@%m/%d").to_string();
    let future_date = (today + chrono::Days::new(5))
        .format("@%m/%d/%y")
        .to_string();

    let content = format!(
        "# {}\n- [ ] Past due task {}\n- [ ] Future task {}\n- [ ] No date task\n",
        today.format("%Y/%m/%d"),
        past_date,
        future_date
    );
    let mut ctx = TestContext::with_journal_content(today, &content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("@overdue");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Past due task"));

    assert!(!ctx.screen_contains("Future task"));
    assert!(!ctx.screen_contains("No date task"));
}

#[test]
fn later_filter_shows_scheduled_entries() {
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

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("@later");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Scheduled task"));

    assert!(!ctx.screen_contains("Regular task"));
    assert!(!ctx.screen_contains("Recurring task"));
}
