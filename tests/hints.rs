mod helpers;

use std::collections::HashMap;

use crossterm::event::KeyCode;
use helpers::TestContext;

use caliber::config::Config;

#[test]
fn command_hints_show_but_do_not_autocomplete() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char(':'));
    ctx.type_str("qu");
    ctx.press(KeyCode::Right);

    // Commands show hints but right arrow doesn't autocomplete
    assert_eq!(ctx.app.prompt_content(), Some("qu"));
}

#[test]
fn tag_hint_completes_with_right_arrow() {
    let content = "# 2026/01/15\n- [ ] Task with #feature tag\n";
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Enter);
    ctx.type_str("New task #fe");
    ctx.press(KeyCode::Right);
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("New task #feature"));
    assert!(ctx.read_journal().contains("New task #feature"));
}

#[test]
fn filter_type_hint_completes_with_right_arrow() {
    let content = "# 2026/01/15\n- [ ] Incomplete task\n- [x] Completed task\n- A note\n";
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("!ta");
    ctx.press(KeyCode::Right);
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Incomplete task"));
    assert!(!ctx.screen_contains("A note"));
}

#[test]
fn date_op_hint_completes_with_right_arrow() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("@be");
    ctx.press(KeyCode::Right);

    assert_eq!(ctx.app.prompt_content(), Some("@before:"));
}

#[test]
fn date_op_hint_completes_with_enter_but_does_not_submit() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("@be");
    ctx.press(KeyCode::Enter);

    assert_eq!(ctx.app.prompt_content(), Some("@before:"));
    assert!(matches!(
        ctx.app.input_mode,
        caliber::app::InputMode::Prompt(_)
    ));
}

#[test]
fn negation_hint_completes_with_right_arrow() {
    let content = "# 2026/01/15\n- [ ] Task with #feature tag\n";
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("not:#fe");
    ctx.press(KeyCode::Right);

    let query = ctx.app.prompt_content();
    assert_eq!(query, Some("not:#feature "));
}

#[test]
fn tag_hints_complete_in_multiword_context() {
    let content = "# 2026/01/15\n- [ ] Task with #work tag\n";
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Enter);
    ctx.type_str("Meeting notes #wo");
    ctx.press(KeyCode::Right);
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Meeting notes #work"));
}

#[test]
fn exact_match_skips_completion() {
    let content = "# 2026/01/15\n- [ ] Task with #bug tag\n";
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Enter);
    ctx.type_str("#bug");
    ctx.press(KeyCode::Right);

    let buffer = ctx
        .app
        .edit_buffer
        .as_ref()
        .map(|b| b.content().to_string());
    assert_eq!(
        buffer,
        Some("#bug".to_string()),
        "Should not add anything on exact match"
    );
}

#[test]
fn escape_clears_command_buffer_and_exits() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char(':'));
    ctx.type_str("da");
    ctx.press(KeyCode::Esc);

    assert!(ctx.app.prompt_is_empty());
    assert!(matches!(
        ctx.app.input_mode,
        caliber::app::InputMode::Normal
    ));
}

#[test]
fn escape_exits_query_input_mode() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("!tasks");
    ctx.press(KeyCode::Esc);

    assert!(matches!(
        ctx.app.input_mode,
        caliber::app::InputMode::Normal
    ));
}

#[test]
fn tags_collect_from_journal_for_hints() {
    let content = "# 2026/01/15\n- [ ] #alpha task\n- [ ] #beta task\n- [ ] #alpha again\n";
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Enter);
    ctx.type_str("#a");
    ctx.press(KeyCode::Right);
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("#alpha"));
}

#[test]
fn saved_filter_hint_completes_with_right_arrow() {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut config = Config::default();
    config.filters = HashMap::from([
        ("work".to_string(), "#work !tasks".to_string()),
        ("weekly".to_string(), "@after:d7".to_string()),
    ]);

    let mut ctx = TestContext::with_config_and_content(date, "", config);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("$wo");
    ctx.press(KeyCode::Right);

    assert_eq!(ctx.app.prompt_content(), Some("$work "));
}

#[test]
fn date_value_hints_show_after_colon() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("@before:d");
    ctx.press(KeyCode::Right);

    assert!(ctx.app.prompt_content().is_some_and(|s| s.starts_with("@before:d")));
}

#[test]
fn empty_filter_shows_guidance_message() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char('/'));

    assert!(matches!(
        ctx.app.hint_state,
        caliber::app::HintContext::GuidanceMessage { .. }
    ));

    ctx.press(KeyCode::Right);
    assert!(ctx.app.prompt_is_empty());
}

#[test]
fn command_without_colon_does_not_need_continuation() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char(':'));
    ctx.type_str("config");

    assert!(!ctx.app.input_needs_continuation());
}

#[test]
fn date_value_hints_recognize_future_suffix() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("@before:d7+");

    assert!(matches!(
        ctx.app.hint_state,
        caliber::app::HintContext::DateValues { .. }
    ));
}

#[test]
fn relative_days_limit_to_three_digits() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("@before:d999");

    assert!(matches!(
        ctx.app.hint_state,
        caliber::app::HintContext::DateValues { .. }
    ));

    ctx.type_str("9");

    assert!(matches!(
        ctx.app.hint_state,
        caliber::app::HintContext::Inactive
    ));
}
