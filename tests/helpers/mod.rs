#![allow(dead_code)]

use std::path::PathBuf;

use chrono::NaiveDate;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tempfile::TempDir;

use caliber::app::{App, InputMode, ViewMode};
use caliber::config::Config;
use caliber::handlers;
use caliber::storage::{JournalContext, JournalSlot};
use caliber::ui;

/// Test context holding temporary directory and app state
pub struct TestContext {
    pub app: App,
    pub temp_dir: TempDir,
}

impl TestContext {
    /// Create a test context with a fresh temporary journal on a default date
    pub fn new() -> Self {
        Self::with_date(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap())
    }

    /// Create a test context with a specific date
    pub fn with_date(date: NaiveDate) -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let journal_path = temp_dir.path().join("test_journal.md");
        std::fs::write(&journal_path, "").expect("Failed to create journal");

        let context = JournalContext::new(journal_path, None, JournalSlot::Global);

        let config = Config::default();
        let app = App::new_with_context(config, date, context).expect("Failed to create app");

        Self { app, temp_dir }
    }

    /// Create context with pre-existing journal content
    pub fn with_journal_content(date: NaiveDate, content: &str) -> Self {
        Self::with_config_and_content(date, content, Config::default())
    }

    /// Create context with custom config and pre-existing journal content
    pub fn with_config_and_content(date: NaiveDate, content: &str, config: Config) -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let journal_path = temp_dir.path().join("test_journal.md");
        std::fs::write(&journal_path, content).expect("Failed to write journal");

        let context = JournalContext::new(journal_path, None, JournalSlot::Global);
        let app = App::new_with_context(config, date, context).expect("Failed to create app");

        Self { app, temp_dir }
    }

    /// Simulate a key press
    pub fn press(&mut self, key: KeyCode) {
        let event = KeyEvent::new(key, KeyModifiers::NONE);
        self.handle_key_event(event);
    }

    /// Simulate a key with modifiers
    pub fn press_with_modifiers(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        let event = KeyEvent::new(key, modifiers);
        self.handle_key_event(event);
    }

    /// Type a string character by character
    pub fn type_str(&mut self, s: &str) {
        for c in s.chars() {
            self.press(KeyCode::Char(c));
        }
    }

    /// Handle key event through appropriate handler
    fn handle_key_event(&mut self, key: KeyEvent) {
        if self.app.show_help {
            handlers::handle_help_key(&mut self.app, key.code);
        } else {
            match &self.app.input_mode {
                InputMode::Command => {
                    let _ = handlers::handle_command_key(&mut self.app, key);
                }
                InputMode::Normal => {
                    let _ = handlers::handle_normal_key(&mut self.app, key.code);
                }
                InputMode::Edit(_) => {
                    handlers::handle_edit_key(&mut self.app, key);
                }
                InputMode::QueryInput => {
                    let _ = handlers::handle_query_input_key(&mut self.app, key);
                }
                InputMode::Reorder => {
                    handlers::handle_reorder_key(&mut self.app, key.code);
                }
                InputMode::Confirm(_) => {
                    let _ = handlers::handle_confirm_key(&mut self.app, key.code);
                }
                InputMode::Selection(_) => {
                    let _ = handlers::handle_selection_key(&mut self.app, key);
                }
            }
        }
    }

    /// Get rendered daily view content as strings
    pub fn render_daily(&self) -> Vec<String> {
        let lines = ui::render_daily_view(&self.app, 76);
        lines
            .iter()
            .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    /// Get rendered filter view content as strings
    pub fn render_filter(&self) -> Vec<String> {
        let lines = ui::render_filter_view(&self.app, 76);
        lines
            .iter()
            .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    /// Get current view's rendered content
    pub fn render_current(&self) -> Vec<String> {
        match &self.app.view {
            ViewMode::Daily(_) => self.render_daily(),
            ViewMode::Filter(_) => self.render_filter(),
        }
    }

    /// Check if any rendered line contains the given text
    pub fn screen_contains(&self, text: &str) -> bool {
        self.render_current().iter().any(|line| line.contains(text))
    }

    /// Find the line containing text and return it
    pub fn find_line(&self, text: &str) -> Option<String> {
        self.render_current()
            .into_iter()
            .find(|line| line.contains(text))
    }

    /// Get journal file path
    pub fn journal_path(&self) -> PathBuf {
        self.temp_dir.path().join("test_journal.md")
    }

    /// Read raw journal content
    pub fn read_journal(&self) -> String {
        std::fs::read_to_string(self.journal_path()).unwrap_or_default()
    }

    /// Get cursor position in edit buffer (only valid in Edit mode)
    pub fn cursor_position(&self) -> Option<usize> {
        self.app.edit_buffer.as_ref().map(|b| b.cursor_char_pos())
    }

    /// Get selected index in current view
    pub fn selected_index(&self) -> usize {
        match &self.app.view {
            ViewMode::Daily(state) => state.selected,
            ViewMode::Filter(state) => state.selected,
        }
    }

    /// Get number of entries in current day
    pub fn entry_count(&self) -> usize {
        self.app.entry_indices.len()
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // No global state to reset - context is owned by App
    }
}
