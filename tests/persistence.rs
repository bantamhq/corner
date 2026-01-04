mod helpers;

use chrono::NaiveDate;
use crossterm::event::KeyCode;
use helpers::TestContext;

use caliber::app::App;
use caliber::config::Config;
use caliber::storage::{JournalContext, JournalSlot, Line};

/// PS-1: Edit persistence round-trip
#[test]
fn test_edit_persists_after_reload() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let temp_dir = tempfile::TempDir::new().unwrap();
    let journal_path = temp_dir.path().join("test_journal.md");

    // Write content directly to journal file
    let content = format!("# {}\n- [ ] Persistent entry\n", date.format("%Y/%m/%d"));
    std::fs::write(&journal_path, &content).unwrap();

    // Load via App with explicit context
    let context = JournalContext::new(journal_path, None, JournalSlot::Global);
    let config = Config::default();
    let app = App::new_with_context(config, date, context).unwrap();

    // Entry should be loaded
    let has_entry = app.entry_indices.iter().any(|&i| {
        if let Line::Entry(e) = &app.lines[i] {
            e.content.contains("Persistent entry")
        } else {
            false
        }
    });
    assert!(has_entry, "Entry should be loaded from journal file");
}

/// PS-3: Entry type preservation
#[test]
fn test_entry_type_preserved() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let mut ctx = TestContext::with_date(date);

    // Create task
    ctx.press(KeyCode::Enter);
    ctx.type_str("A task");
    ctx.press(KeyCode::Enter);

    // Create note
    ctx.press(KeyCode::Enter);
    ctx.type_str("A note");
    ctx.press(KeyCode::BackTab); // Switch to note
    ctx.press(KeyCode::Enter);

    // Toggle task complete
    ctx.press(KeyCode::Char('g')); // Go to first
    ctx.press(KeyCode::Char('c'));

    // Check journal
    let journal = ctx.read_journal();
    assert!(
        journal.contains("- [x] A task"),
        "Task should be marked complete"
    );
    assert!(journal.contains("- A note"), "Note should be preserved");
}

/// PS-2: Multi-day preservation
#[test]
fn test_multi_day_entries_preserved() {
    let date_a = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    // date_b is in the content but we navigate to it via 'l' key
    let _date_b = NaiveDate::from_ymd_opt(2026, 1, 16).unwrap();
    let content = "# 2026/01/15\n- [ ] Day A entry\n# 2026/01/16\n- [ ] Day B entry\n";
    let mut ctx = TestContext::with_journal_content(date_a, content);

    // Verify day A
    assert!(
        ctx.screen_contains("Day A entry"),
        "Day A entry should be visible"
    );

    // Edit day A
    ctx.press(KeyCode::Char('i'));
    ctx.press(KeyCode::End);
    ctx.type_str(" modified");
    ctx.press(KeyCode::Enter);

    // Navigate to day B
    ctx.press(KeyCode::Char('l'));

    // Day B should be unchanged
    assert!(
        ctx.screen_contains("Day B entry"),
        "Day B entry should be visible"
    );
    assert!(
        !ctx.screen_contains("Day B entry modified"),
        "Day B entry should be unchanged"
    );

    // Verify both are persisted correctly
    let journal = ctx.read_journal();
    assert!(
        journal.contains("Day A entry modified"),
        "Modified day A should persist"
    );
    assert!(journal.contains("Day B entry"), "Day B should be preserved");
}

/// PS-4: Filter edit persistence
#[test]
fn test_filter_edit_persists() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let content = "# 2026/01/15\n- [ ] Filterable entry\n";
    let mut ctx = TestContext::with_journal_content(date, content);

    // Filter and edit
    ctx.press(KeyCode::Char('/'));
    ctx.type_str("Filterable");
    ctx.press(KeyCode::Enter);

    ctx.press(KeyCode::Char('i'));
    ctx.press(KeyCode::End);
    ctx.type_str(" edited");
    ctx.press(KeyCode::Enter);

    // Exit filter
    ctx.press(KeyCode::Tab);

    // Verify in daily view
    assert!(
        ctx.screen_contains("Filterable entry edited"),
        "Edit should appear in daily view"
    );

    // Verify persistence
    let journal = ctx.read_journal();
    assert!(
        journal.contains("Filterable entry edited"),
        "Edit should be persisted"
    );
}
