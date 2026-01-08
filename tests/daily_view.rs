mod helpers;

use caliber::config::Config;
use chrono::NaiveDate;
use crossterm::event::KeyCode;
use helpers::TestContext;

#[test]
fn o_key_creates_entry_below_current() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] First\n- [ ] Second\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('o'));
    ctx.type_str("Below first");
    ctx.press(KeyCode::Enter);

    let journal = ctx.read_journal();
    let first_pos = journal.find("First").unwrap();
    let below_pos = journal.find("Below first").unwrap();
    let second_pos = journal.find("Second").unwrap();

    assert!(first_pos < below_pos && below_pos < second_pos);
}

#[test]
fn shift_o_creates_entry_above_current() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] First\n- [ ] Second\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('G'));
    ctx.press(KeyCode::Char('O'));
    ctx.type_str("Above second");
    ctx.press(KeyCode::Enter);

    let journal = ctx.read_journal();
    let first_pos = journal.find("First").unwrap();
    let above_pos = journal.find("Above second").unwrap();
    let second_pos = journal.find("Second").unwrap();

    assert!(first_pos < above_pos && above_pos < second_pos);
}

#[test]
fn delete_removes_entry_and_undo_restores() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Entry A\n- [ ] Entry B\n- [ ] Entry C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    assert_eq!(ctx.selected_index(), 0);
    ctx.press(KeyCode::Char('j'));
    assert_eq!(ctx.selected_index(), 1);

    ctx.press(KeyCode::Char('d'));
    assert!(!ctx.screen_contains("Entry B"));
    assert!(ctx.screen_contains("Entry A"));
    assert!(ctx.screen_contains("Entry C"));

    assert!(ctx.selected_index() < ctx.entry_count());
    assert_eq!(ctx.selected_index(), 1);

    ctx.press(KeyCode::Char('u'));
    assert!(ctx.screen_contains("Entry B"));
}

#[test]
fn reorder_mode_moves_entry_with_j_k() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A\n- [ ] B\n- [ ] C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('r'));
    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Enter);

    let journal = ctx.read_journal();
    let b_pos = journal.find(" B").unwrap();
    let a_pos = journal.find(" A").unwrap();
    let c_pos = journal.find(" C").unwrap();
    assert!(b_pos < a_pos && a_pos < c_pos);
}

#[test]
fn escape_cancels_reorder_without_saving() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A\n- [ ] B\n- [ ] C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('r'));
    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Esc);

    let journal = ctx.read_journal();
    let a_pos = journal.find(" A").unwrap();
    let b_pos = journal.find(" B").unwrap();
    let c_pos = journal.find(" C").unwrap();
    assert!(a_pos < b_pos && b_pos < c_pos);
}

#[test]
fn z_key_toggles_completed_visibility() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Incomplete\n- [x] Completed\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    assert!(ctx.screen_contains("Incomplete"));
    assert!(ctx.screen_contains("Completed"));

    ctx.press(KeyCode::Char('z'));
    assert!(ctx.screen_contains("Incomplete"));
    assert!(!ctx.screen_contains("Completed"));

    ctx.press(KeyCode::Char('z'));
    assert!(ctx.screen_contains("Completed"));
}

#[test]
fn shift_t_key_tidies_entries_by_type() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Incomplete task\n- A note\n- [x] Completed task\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('T'));

    let journal = ctx.read_journal();
    let completed_pos = journal.find("Completed task").unwrap();
    let note_pos = journal.find("A note").unwrap();
    let incomplete_pos = journal.find("Incomplete task").unwrap();
    assert!(completed_pos < note_pos && note_pos < incomplete_pos);
}

#[test]
fn c_key_toggles_task_completion() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] My task\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('c'));
    assert!(ctx.screen_contains("[x]"));

    ctx.press(KeyCode::Char('c'));
    assert!(ctx.screen_contains("[ ]"));

    let journal = ctx.read_journal();
    assert!(journal.contains("- [ ] My task"));
}

#[test]
fn delete_last_entry_selects_previous() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A\n- [ ] B\n- [ ] C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('G'));
    assert_eq!(ctx.selected_index(), 2);

    ctx.press(KeyCode::Char('d'));
    assert_eq!(ctx.selected_index(), 1);
    assert!(ctx.screen_contains("B"));
    assert!(!ctx.screen_contains(" C"));
}

#[test]
fn delete_middle_entry_keeps_selection_index() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A\n- [ ] B\n- [ ] C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('j'));
    assert_eq!(ctx.selected_index(), 1);

    ctx.press(KeyCode::Char('d'));
    assert!(ctx.selected_index() < ctx.entry_count());
    assert_eq!(ctx.selected_index(), 1);
}

#[test]
fn navigation_scrolls_to_keep_selection_visible() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut content = "# 2026/01/15\n".to_string();
    for i in 1..=30 {
        content.push_str(&format!("- [ ] Entry {}\n", i));
    }
    let mut ctx = TestContext::with_journal_content(date, &content);

    ctx.press(KeyCode::Char('G'));
    assert_eq!(ctx.selected_index(), 29);
    assert!(ctx.screen_contains("Entry 30"));

    ctx.press(KeyCode::Char('k'));
    assert_eq!(ctx.selected_index(), 28);

    ctx.press(KeyCode::Char('g'));
    assert_eq!(ctx.selected_index(), 0);
    assert!(ctx.screen_contains("Entry 1"));
}

#[test]
fn y_key_copies_entry_content() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Yank this content\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('y'));

    assert!(ctx.screen_contains("Yank this content"));
}

#[test]
fn reorder_skips_hidden_completed_entries() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] A\n- [x] B\n- [ ] C\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    ctx.press(KeyCode::Char('z'));

    assert!(ctx.screen_contains(" A"));
    assert!(!ctx.screen_contains("[x] B"));
    assert!(ctx.screen_contains(" C"));

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('r'));
    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Enter);

    let journal = ctx.read_journal();
    let _b_pos = journal.find("[x] B").unwrap();
    let c_pos = journal.find("[ ] C").unwrap();
    let a_pos = journal.find("[ ] A").unwrap();

    assert!(c_pos < a_pos);
}

#[test]
fn config_header_date_format_customizes_display() {
    let mut config = Config::default();
    config.header_date_format = "%A, %B %d".to_string();
    let date = NaiveDate::from_ymd_opt(2026, 1, 4).unwrap();
    let ctx = TestContext::with_config_and_content(date, "", config);

    assert!(ctx.screen_contains("Sunday, January 04"));
}

#[test]
fn config_hide_completed_hides_on_startup() {
    let mut config = Config::default();
    config.hide_completed = true;
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Incomplete\n- [x] Complete\n";
    let ctx = TestContext::with_config_and_content(date, content, config);

    assert!(ctx.screen_contains("Incomplete"));
    assert!(!ctx.screen_contains("Complete"));
    assert!(ctx.screen_contains("Hiding 1 completed"));
}

#[test]
fn backslash_opens_and_closes_date_interface() {
    use caliber::app::InputMode;

    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_date(date);

    // Opens date interface
    ctx.press(KeyCode::Char('\\'));
    assert!(matches!(ctx.app.input_mode, InputMode::Interface(_)));

    // Toggle closes it
    ctx.press(KeyCode::Char('\\'));
    assert!(matches!(ctx.app.input_mode, InputMode::Normal));
}

#[test]
fn date_interface_navigation_changes_selected_date() {
    use caliber::app::{InputMode, InterfaceContext};

    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_date(date);

    ctx.press(KeyCode::Char('\\'));

    // Navigate right one day
    ctx.press(KeyCode::Char('l'));

    if let InputMode::Interface(InterfaceContext::Date(ref state)) = ctx.app.input_mode {
        assert_eq!(state.selected, NaiveDate::from_ymd_opt(2026, 1, 16).unwrap());
    } else {
        panic!("Expected date interface mode");
    }
}

#[test]
fn date_interface_enter_navigates_to_selected_date() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_date(date);

    ctx.press(KeyCode::Char('\\'));
    // Navigate forward 5 days
    for _ in 0..5 {
        ctx.press(KeyCode::Char('l'));
    }
    ctx.press(KeyCode::Enter);

    assert_eq!(ctx.app.current_date, NaiveDate::from_ymd_opt(2026, 1, 20).unwrap());
}
