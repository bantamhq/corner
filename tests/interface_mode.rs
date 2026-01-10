mod helpers;

use crossterm::event::KeyCode;
use helpers::TestContext;

#[test]
fn date_interface_opens_with_backslash() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('\\'));

    assert!(ctx.footer_contains(" DATE "));
}

#[test]
fn date_interface_closes_with_backslash() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('\\'));
    assert!(ctx.footer_contains(" DATE "));

    ctx.press(KeyCode::Char('\\'));
    assert!(ctx.footer_contains(" DAILY "));
}

#[test]
fn date_interface_closes_with_esc() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('\\'));
    assert!(ctx.footer_contains(" DATE "));

    ctx.press(KeyCode::Esc);
    assert!(ctx.footer_contains(" DAILY "));
}

#[test]
fn date_interface_closes_with_backspace_on_empty_input() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('\\'));
    ctx.press(KeyCode::Backspace);

    assert!(ctx.footer_contains(" DAILY "));
}

#[test]
fn date_interface_backspace_deletes_char_when_input_has_text() {
    use caliber::app::{InputMode, InterfaceContext};

    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('\\'));
    ctx.type_str("1/15");
    ctx.press(KeyCode::Backspace);

    assert!(ctx.footer_contains(" DATE "));
    if let InputMode::Interface(InterfaceContext::Date(ref state)) = ctx.app.input_mode {
        assert_eq!(state.query.content(), "1/1");
    }
}

#[test]
fn date_interface_closes_with_backslash_when_has_text() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('\\'));
    ctx.type_str("1/15");
    ctx.press(KeyCode::Char('\\'));

    assert!(ctx.footer_contains(" DAILY "));
}

#[test]
fn project_interface_opens_with_period() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('.'));

    assert!(ctx.footer_contains(" PROJECT "));
}

#[test]
fn project_interface_closes_with_period() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('.'));
    assert!(ctx.footer_contains(" PROJECT "));

    ctx.press(KeyCode::Char('.'));
    assert!(ctx.footer_contains(" DAILY "));
}

#[test]
fn project_interface_closes_with_esc() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('.'));
    assert!(ctx.footer_contains(" PROJECT "));

    ctx.press(KeyCode::Esc);
    assert!(ctx.footer_contains(" DAILY "));
}

#[test]
fn project_interface_navigation_keeps_interface_open() {
    let mut ctx = TestContext::new();
    ctx.press(KeyCode::Char('.'));
    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('k'));

    assert!(ctx.footer_contains(" PROJECT "));
}

#[test]
fn tag_interface_opens_with_comma() {
    let mut ctx = TestContext::with_journal_content(
        chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        "# 2026/01/15\n- [ ] Task #work\n",
    );
    ctx.press(KeyCode::Char(','));

    assert!(ctx.footer_contains(" TAG "));
}

#[test]
fn tag_interface_closes_with_comma() {
    let mut ctx = TestContext::with_journal_content(
        chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        "# 2026/01/15\n- [ ] Task #work\n",
    );
    ctx.press(KeyCode::Char(','));
    assert!(ctx.footer_contains(" TAG "));

    ctx.press(KeyCode::Char(','));
    assert!(ctx.footer_contains(" DAILY "));
}

#[test]
fn tag_interface_closes_with_esc() {
    let mut ctx = TestContext::with_journal_content(
        chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        "# 2026/01/15\n- [ ] Task #work\n",
    );
    ctx.press(KeyCode::Char(','));
    assert!(ctx.footer_contains(" TAG "));

    ctx.press(KeyCode::Esc);
    assert!(ctx.footer_contains(" DAILY "));
}

#[test]
fn tag_interface_navigation_keeps_interface_open() {
    let mut ctx = TestContext::with_journal_content(
        chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        "# 2026/01/15\n- [ ] Task #work #home\n",
    );
    ctx.press(KeyCode::Char(','));
    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('k'));

    assert!(ctx.footer_contains(" TAG "));
}

#[test]
fn tag_interface_select_filters_by_tag() {
    let mut ctx = TestContext::with_journal_content(
        chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        "# 2026/01/15\n- [ ] Task one #work\n- [ ] Task two #home\n",
    );
    ctx.press(KeyCode::Char(','));
    ctx.press(KeyCode::Enter);

    assert!(ctx.footer_contains(" FILTER "));
    assert!(ctx.screen_contains("Task two"));
}

#[test]
fn tag_delete_removes_all_occurrences() {
    let mut ctx = TestContext::with_journal_content(
        chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        "# 2026/01/15\n- [ ] Task one #work\n- [ ] Task two #work\n- [ ] Task three #home\n",
    );
    ctx.press(KeyCode::Char(','));
    ctx.press(KeyCode::Char('d'));
    ctx.press(KeyCode::Char('y'));

    let journal = ctx.read_journal();
    assert!(!journal.contains("#home"));
    assert!(journal.contains("#work"));
    assert!(journal.contains("Task one"));
    assert!(journal.contains("Task two"));
    assert!(journal.contains("Task three"));
}

#[test]
fn tag_rename_updates_all_occurrences() {
    let mut ctx = TestContext::with_journal_content(
        chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        "# 2026/01/15\n- [ ] Task one #work\n- [ ] Task two #work\n",
    );
    ctx.press(KeyCode::Char(','));
    ctx.press(KeyCode::Char('r'));
    ctx.type_str("job");
    ctx.press(KeyCode::Enter);

    let journal = ctx.read_journal();
    assert!(!journal.contains("#work"));
    assert!(journal.contains("#job"));
    assert_eq!(journal.matches("#job").count(), 2);
}

#[test]
fn tag_rename_validates_empty_name() {
    let mut ctx = TestContext::with_journal_content(
        chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        "# 2026/01/15\n- [ ] Task #work\n",
    );
    ctx.press(KeyCode::Char(','));
    ctx.press(KeyCode::Char('r'));
    ctx.press(KeyCode::Enter);

    assert!(ctx.status_contains("cannot be empty"));
    let journal = ctx.read_journal();
    assert!(journal.contains("#work"));
}

#[test]
fn tag_rename_ignores_invalid_starting_chars() {
    let mut ctx = TestContext::with_journal_content(
        chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        "# 2026/01/15\n- [ ] Task #apple\n",
    );
    ctx.press(KeyCode::Char(','));
    ctx.press(KeyCode::Char('r'));
    ctx.type_str("123banana");
    ctx.press(KeyCode::Enter);

    let journal = ctx.read_journal();
    assert!(!journal.contains("#apple"));
    assert!(journal.contains("#banana"));
}
