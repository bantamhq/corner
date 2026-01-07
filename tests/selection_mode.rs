mod helpers;

use chrono::NaiveDate;
use crossterm::event::{KeyCode, KeyModifiers};
use helpers::TestContext;

use caliber::app::InputMode;

#[test]
fn v_key_enters_and_escape_exits_selection() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Entry A\n- [ ] Entry B\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('v'));
    assert!(matches!(ctx.app.input_mode, InputMode::Selection(_)));

    ctx.press(KeyCode::Esc);
    assert!(matches!(ctx.app.input_mode, InputMode::Normal));
}

#[test]
fn v_key_toggles_entry_selection() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Entry A\n- [ ] Entry B\n- [ ] Entry C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('v'));

    let state = ctx.app.get_selection_state().unwrap();
    assert!(state.is_selected(0));
    assert_eq!(state.count(), 1);

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('v'));

    let state = ctx.app.get_selection_state().unwrap();
    assert!(state.is_selected(0));
    assert!(state.is_selected(1));
    assert_eq!(state.count(), 2);
}

#[test]
fn shift_v_selects_range_of_entries() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Entry A\n- [ ] Entry B\n- [ ] Entry C\n- [ ] Entry D\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('G'));

    ctx.press_with_modifiers(KeyCode::Char('V'), KeyModifiers::SHIFT);

    let state = ctx.app.get_selection_state().unwrap();
    assert_eq!(state.count(), 4);
    assert!(state.is_selected(0));
    assert!(state.is_selected(1));
    assert!(state.is_selected(2));
    assert!(state.is_selected(3));
}

#[test]
fn d_key_batch_deletes_selected_entries() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Keep\n- [ ] Delete A\n- [ ] Delete B\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('j'));

    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('d'));

    assert!(matches!(ctx.app.input_mode, InputMode::Normal));

    let journal = ctx.read_journal();
    assert!(journal.contains("Keep"));
    assert!(!journal.contains("Delete A"));
    assert!(!journal.contains("Delete B"));
}

#[test]
fn c_key_batch_toggles_selected_tasks() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Task A\n- [ ] Task B\n- Note\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('c'));

    let journal = ctx.read_journal();
    assert!(journal.contains("[x] Task A"));
    assert!(journal.contains("[x] Task B"));
}

#[test]
fn y_key_batch_yanks_selected_entries() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Entry A\n- [ ] Entry B\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('y'));

    assert!(matches!(ctx.app.input_mode, InputMode::Selection(_)));

    assert!(ctx.screen_contains("Entry A"));
    assert!(ctx.screen_contains("Entry B"));
}

#[test]
fn selection_respects_hidden_completed_entries() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Incomplete A\n- [x] Completed\n- [ ] Incomplete B\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('z'));

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('d'));

    let journal = ctx.read_journal();
    assert!(!journal.contains("Incomplete A"));
    assert!(journal.contains("Completed"));
    assert!(!journal.contains("Incomplete B"));
}

#[test]
fn x_key_removes_last_tag_from_selected() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Entry A #tag1 #tag2\n- [ ] Entry B #tag3\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('x'));

    let journal = ctx.read_journal();
    assert!(journal.contains("Entry A #tag1") && !journal.contains("#tag2"));
    assert!(!journal.contains("#tag3"));
}

#[test]
fn shift_x_removes_all_tags_from_selected() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Entry A #tag1 #tag2\n- [ ] Entry B #tag3\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('X'));

    let journal = ctx.read_journal();
    assert!(!journal.contains("#tag1") && !journal.contains("#tag2"));
    assert!(!journal.contains("#tag3"));
}

#[test]
fn selection_works_in_filter_view() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Task A\n- [ ] Task B\n- [ ] Task C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('/'));
    ctx.type_str("!tasks");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('v'));

    assert!(matches!(ctx.app.input_mode, InputMode::Selection(_)));

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('c'));

    let journal = ctx.read_journal();
    assert!(journal.contains("[x] Task A"));
    assert!(journal.contains("[x] Task B"));
    assert!(journal.contains("[ ] Task C"));
}

#[test]
fn batch_delete_undos_as_single_operation() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Keep\n- [ ] Delete A\n- [ ] Delete B\n- [ ] Delete C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('j'));

    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('G'));
    ctx.press_with_modifiers(KeyCode::Char('V'), KeyModifiers::SHIFT);

    let state = ctx.app.get_selection_state().unwrap();
    assert_eq!(state.count(), 3);

    ctx.press(KeyCode::Char('d'));

    let journal = ctx.read_journal();
    assert!(journal.contains("Keep"));
    assert!(!journal.contains("Delete A"));
    assert!(!journal.contains("Delete B"));
    assert!(!journal.contains("Delete C"));

    ctx.press(KeyCode::Char('u'));

    let journal = ctx.read_journal();
    assert!(journal.contains("Keep"));
    assert!(journal.contains("Delete A"));
    assert!(journal.contains("Delete B"));
    assert!(journal.contains("Delete C"));
}

#[test]
fn batch_delete_supports_undo_redo() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Keep\n- [ ] Delete Me\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('G'));
    ctx.press(KeyCode::Char('d'));

    let journal = ctx.read_journal();
    assert!(!journal.contains("Delete Me"));

    ctx.press(KeyCode::Char('u'));
    let journal = ctx.read_journal();
    assert!(journal.contains("Delete Me"));

    ctx.press(KeyCode::Char('U'));
    let journal = ctx.read_journal();
    assert!(!journal.contains("Delete Me"));

    ctx.press(KeyCode::Char('u'));
    let journal = ctx.read_journal();
    assert!(journal.contains("Delete Me"));
}

#[test]
fn selection_mode_allows_navigation_keys() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A\n- [ ] B\n- [ ] C\n- [ ] D\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('v'));

    ctx.press(KeyCode::Char('j'));
    assert_eq!(ctx.selected_index(), 1);

    ctx.press(KeyCode::Char('k'));
    assert_eq!(ctx.selected_index(), 0);

    ctx.press(KeyCode::Char('G'));
    assert_eq!(ctx.selected_index(), 3);

    ctx.press(KeyCode::Char('g'));
    assert_eq!(ctx.selected_index(), 0);

    assert!(matches!(ctx.app.input_mode, InputMode::Selection(_)));
}
