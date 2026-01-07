mod helpers;

use chrono::NaiveDate;
use crossterm::event::KeyCode;
use helpers::TestContext;

use caliber::app::ViewMode;

#[test]
fn tag_filter_shows_matching_entries() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content =
        "# 2026/01/15\n- [ ] Task with #work\n- [ ] Task with #personal\n- Note with #work\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("#work");
    ctx.press(KeyCode::Enter);

    assert!(matches!(ctx.app.view, ViewMode::Filter(_)));
    assert!(ctx.screen_contains("Task with #work"));
    assert!(ctx.screen_contains("Note with #work"));
    assert!(!ctx.screen_contains("#personal"));
}

#[test]
fn tasks_filter_shows_only_tasks() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A task\n- A note\n* An event\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("!tasks");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("A task"));
    assert!(!ctx.screen_contains("A note"));
    assert!(!ctx.screen_contains("An event"));
}

#[test]
fn notes_filter_shows_only_notes() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A task\n- A note\n* An event\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("!notes");
    ctx.press(KeyCode::Enter);

    assert!(!ctx.screen_contains("A task"));
    assert!(ctx.screen_contains("A note"));
    assert!(!ctx.screen_contains("An event"));
}

#[test]
fn completed_filter_shows_done_tasks() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Incomplete task\n- [x] Completed task\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("!tasks");
    ctx.press(KeyCode::Enter);
    assert!(ctx.screen_contains("Incomplete task"));
    assert!(!ctx.screen_contains("Completed task"));

    ctx.press(KeyCode::Tab);
    ctx.press(KeyCode::Char('/'));
    ctx.type_str("!completed");
    ctx.press(KeyCode::Enter);
    assert!(!ctx.screen_contains("Incomplete task"));
    assert!(ctx.screen_contains("Completed task"));
}

#[test]
fn filters_combine_with_and_logic() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content =
        "# 2026/01/15\n- [ ] Work task #work\n- [ ] Personal task #personal\n- Work note #work\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("!tasks #work");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Work task #work"));
    assert!(!ctx.screen_contains("Personal task"));
    assert!(!ctx.screen_contains("Work note"));
}

#[test]
fn edit_in_filter_persists_to_journal() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Original task\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("task");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Char('i'));
    ctx.press(KeyCode::End);
    ctx.type_str(" modified");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Tab);
    assert!(ctx.screen_contains("Original task modified"));

    let journal = ctx.read_journal();
    assert!(journal.contains("Original task modified"));
}

#[test]
fn toggle_in_filter_updates_journal() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] My task\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("!tasks");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Char('c'));

    let journal = ctx.read_journal();
    assert!(journal.contains("- [x] My task"));
}

#[test]
fn enter_in_filter_creates_new_entry() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_date(date);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("!tasks");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Enter);
    ctx.type_str("New from filter");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("New from filter"));

    let journal = ctx.read_journal();
    assert!(journal.contains("New from filter"));
}

#[test]
fn tab_toggles_between_daily_and_filter() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Task #work\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("#work");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Tab);
    assert!(matches!(ctx.app.view, ViewMode::Daily(_)));

    ctx.press(KeyCode::Tab);
    assert!(matches!(ctx.app.view, ViewMode::Filter(_)));
    assert!(ctx.screen_contains("#work"));
}

#[test]
fn delete_in_filter_removes_from_journal() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Delete me\n- [ ] Keep me\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("Delete");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Char('d'));

    ctx.press(KeyCode::Tab);
    assert!(!ctx.screen_contains("Delete me"));
    assert!(ctx.screen_contains("Keep me"));

    let journal = ctx.read_journal();
    assert!(!journal.contains("Delete me"));
}

#[test]
fn not_prefix_excludes_matching_entries() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content =
        "# 2026/01/15\n- [ ] Work task #work\n- [ ] Personal task #personal\n- [ ] Untagged task\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("not:#work");
    ctx.press(KeyCode::Enter);

    assert!(!ctx.screen_contains("Work task"));
    assert!(ctx.screen_contains("Personal task"));
    assert!(ctx.screen_contains("Untagged"));
}

#[test]
fn after_filter_includes_entries_on_or_after_date() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Today entry\n# 2026/01/14\n- [ ] Yesterday entry\n# 2026/01/10\n- [ ] Old entry\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("@after:1/14/26");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Today entry"));
    assert!(ctx.screen_contains("Yesterday entry"));
    assert!(!ctx.screen_contains("Old entry"));
}

#[test]
fn before_filter_includes_entries_on_or_before_date() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Today entry\n# 2026/01/14\n- [ ] Yesterday entry\n# 2026/01/10\n- [ ] Old entry\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("@before:1/14/26");
    ctx.press(KeyCode::Enter);

    assert!(!ctx.screen_contains("Today entry"));
    assert!(ctx.screen_contains("Yesterday entry"));
    assert!(ctx.screen_contains("Old entry"));
}

#[test]
fn not_tasks_excludes_task_entries() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A task\n- A note\n* An event\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("not:!tasks");
    ctx.press(KeyCode::Enter);

    assert!(!ctx.screen_contains("A task"));
    assert!(ctx.screen_contains("A note"));
    assert!(ctx.screen_contains("An event"));
}

#[test]
fn r_key_refreshes_filter_results() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Task without tag\n- [ ] Task with #work\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("#work");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Task with #work"));
    assert!(!ctx.screen_contains("Task without tag"));

    ctx.press(KeyCode::Tab);
    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('i'));
    ctx.press(KeyCode::End);
    ctx.type_str(" #work");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Tab);
    ctx.press(KeyCode::Char('r'));

    assert!(ctx.screen_contains("Task without tag"));
    assert!(ctx.screen_contains("Task with #work"));
}

#[test]
fn number_key_filters_by_favorite_tag() {
    use std::collections::HashMap;

    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content =
        "# 2026/01/15\n- [ ] Task with #work\n- [ ] Task with #personal\n- [ ] Task without tags\n";

    let mut config = caliber::config::Config::default();
    let mut tags = HashMap::new();
    tags.insert("1".to_string(), "work".to_string());
    config.favorite_tags = tags;

    let mut ctx = TestContext::with_config_and_content(date, content, config);

    ctx.press(KeyCode::Char('1'));

    assert!(matches!(ctx.app.view, ViewMode::Filter(_)));
    assert!(ctx.screen_contains("Task with #work"));
    assert!(!ctx.screen_contains("Task with #personal"));
    assert!(!ctx.screen_contains("Task without tags"));
}

#[test]
fn undo_redo_works_in_filter_view() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Entry A\n- [ ] Entry B\n- [ ] Entry C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("Entry");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Entry A"));
    assert!(ctx.screen_contains("Entry B"));
    assert!(ctx.screen_contains("Entry C"));

    ctx.press(KeyCode::Char('k'));
    ctx.press(KeyCode::Char('d'));

    assert!(ctx.screen_contains("Entry A"));
    assert!(!ctx.screen_contains("Entry B"));
    assert!(ctx.screen_contains("Entry C"));

    ctx.press(KeyCode::Char('u'));

    assert!(ctx.screen_contains("Entry B"));

    ctx.press(KeyCode::Char('U'));

    assert!(!ctx.screen_contains("Entry B"));

    ctx.press(KeyCode::Char('u'));
    let journal = ctx.read_journal();
    assert!(journal.contains("Entry B"));
}

#[test]
fn dollar_prefix_expands_saved_filter() {
    use std::collections::HashMap;

    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content =
        "# 2026/01/15\n- [ ] Urgent task #urgent\n- [ ] Normal task #work\n- A note #urgent\n";

    let mut config = caliber::config::Config::default();
    let mut filters = HashMap::new();
    filters.insert("t".to_string(), "!tasks".to_string());
    config.filters = filters;

    let mut ctx = TestContext::with_config_and_content(date, content, config);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("$t #urgent");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Urgent task #urgent"));
    assert!(!ctx.screen_contains("Normal task"));
    assert!(!ctx.screen_contains("A note"));
}
