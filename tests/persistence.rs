mod helpers;

use chrono::NaiveDate;
use crossterm::event::KeyCode;
use helpers::TestContext;

use caliber::app::App;
use caliber::config::Config;
use caliber::storage::{JournalContext, JournalSlot, Line};
use caliber::ui::surface::Surface;

#[test]
fn edits_persist_after_app_reload() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let temp_dir = tempfile::TempDir::new().unwrap();
    let journal_path = temp_dir.path().join("test_journal.md");

    let content = format!("# {}\n- [ ] Persistent entry\n", date.format("%Y/%m/%d"));
    std::fs::write(&journal_path, &content).unwrap();

    let context = JournalContext::new(journal_path, None, JournalSlot::Hub);
    let config = Config::default();
    let app = App::new_with_context(config, date, context, None, Surface::default()).unwrap();

    let has_entry = app.entry_indices.iter().any(|&i| {
        if let Line::Entry(e) = &app.lines[i] {
            e.content.contains("Persistent entry")
        } else {
            false
        }
    });
    assert!(has_entry);
}

#[test]
fn entry_type_preserved_on_save() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_date(date);

    ctx.press(KeyCode::Enter);
    ctx.type_str("A task");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Enter);
    ctx.type_str("A note");
    ctx.press(KeyCode::BackTab);
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Char('g'));
    ctx.press(KeyCode::Char('c'));

    let journal = ctx.read_journal();
    assert!(journal.contains("- [x] A task"));
    assert!(journal.contains("- A note"));
}

#[test]
fn multi_day_entries_preserved_on_edit() {
    let date_a = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let _date_b = NaiveDate::from_ymd_opt(2026, 1, 16).unwrap();
    let content = "# 2026/01/15\n- [ ] Day A entry\n# 2026/01/16\n- [ ] Day B entry\n";
    let mut ctx = TestContext::with_journal_content(date_a, content);

    assert!(ctx.screen_contains("Day A entry"));

    ctx.press(KeyCode::Char('i'));
    ctx.press(KeyCode::End);
    ctx.type_str(" modified");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Char('l'));

    assert!(ctx.screen_contains("Day B entry"));
    assert!(!ctx.screen_contains("Day B entry modified"));

    let journal = ctx.read_journal();
    assert!(journal.contains("Day A entry modified"));
    assert!(journal.contains("Day B entry"));
}
