mod helpers;

use chrono::NaiveDate;
use crossterm::event::KeyCode;
use helpers::TestContext;

#[test]
fn h_l_keys_navigate_between_days() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Today entry\n# 2026/01/14\n- [ ] Yesterday entry\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    assert_eq!(ctx.app.current_date, date);
    assert!(ctx.screen_contains("Today entry"));

    ctx.press(KeyCode::Char('h'));
    assert_eq!(
        ctx.app.current_date,
        NaiveDate::from_ymd_opt(2026, 1, 14).unwrap()
    );
    assert!(ctx.screen_contains("Yesterday entry"));

    ctx.press(KeyCode::Char('l'));
    assert_eq!(ctx.app.current_date, date);
    assert!(ctx.screen_contains("Today entry"));
}

#[test]
fn t_key_jumps_to_today() {
    let actual_today = chrono::Local::now().date_naive();
    let past_date = actual_today - chrono::Days::new(5);
    let mut ctx = TestContext::with_date(past_date);

    ctx.press(KeyCode::Char('h'));
    ctx.press(KeyCode::Char('h'));
    assert_eq!(ctx.app.current_date, past_date - chrono::Days::new(2));

    ctx.press(KeyCode::Char('t'));
    assert_eq!(ctx.app.current_date, actual_today);
}

#[test]
fn g_shift_g_j_k_navigate_between_entries() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content =
        "# 2026/01/15\n- [ ] Entry 1\n- [ ] Entry 2\n- [ ] Entry 3\n- [ ] Entry 4\n- [ ] Entry 5\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    let lines = ctx.render_daily();
    assert!(lines
        .iter()
        .any(|l| l.starts_with("→") && l.contains("Entry 1")));

    ctx.press(KeyCode::Char('G'));
    let lines = ctx.render_daily();
    assert!(lines
        .iter()
        .any(|l| l.starts_with("→") && l.contains("Entry 5")));

    ctx.press(KeyCode::Char('k'));
    let lines = ctx.render_daily();
    assert!(lines
        .iter()
        .any(|l| l.starts_with("→") && l.contains("Entry 4")));

    ctx.press(KeyCode::Char('j'));
    let lines = ctx.render_daily();
    assert!(lines
        .iter()
        .any(|l| l.starts_with("→") && l.contains("Entry 5")));
}

#[test]
fn navigation_skips_hidden_completed_entries() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A\n- [x] B\n- [ ] C\n- [x] D\n- [ ] E\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('z'));

    ctx.press(KeyCode::Char('g'));
    let lines = ctx.render_daily();
    assert!(lines.iter().any(|l| l.starts_with("→") && l.contains(" A")));

    ctx.press(KeyCode::Char('j'));
    let lines = ctx.render_daily();
    assert!(lines.iter().any(|l| l.starts_with("→") && l.contains(" C")));

    ctx.press(KeyCode::Char('j'));
    let lines = ctx.render_daily();
    assert!(lines.iter().any(|l| l.starts_with("→") && l.contains(" E")));
}

#[test]
fn l_key_navigates_to_future_dates() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_date(date);

    ctx.press(KeyCode::Char('l'));
    assert_eq!(
        ctx.app.current_date,
        NaiveDate::from_ymd_opt(2026, 1, 16).unwrap()
    );
}

#[test]
fn bracket_keys_navigate_between_days() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_date(date);

    ctx.press(KeyCode::Char('['));
    assert_eq!(
        ctx.app.current_date,
        NaiveDate::from_ymd_opt(2026, 1, 14).unwrap()
    );

    ctx.press(KeyCode::Char(']'));
    assert_eq!(
        ctx.app.current_date,
        NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()
    );
}
