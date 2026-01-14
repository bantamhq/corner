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
use caliber::ui::surface::Surface;

pub struct TestContext {
    pub app: App,
    pub temp_dir: TempDir,
}

impl TestContext {
    pub fn new() -> Self {
        // SAFETY: Tests run single-threaded per test file, env var is set before any other work
        unsafe {
            std::env::set_var("CALIBER_SKIP_CLIPBOARD", "1");
            std::env::set_var("CALIBER_SKIP_REGISTRY", "1");
        }
        Self::with_date(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap())
    }

    pub fn with_date(date: NaiveDate) -> Self {
        // SAFETY: Tests run single-threaded per test file, env var is set before any other work
        unsafe {
            std::env::set_var("CALIBER_SKIP_CLIPBOARD", "1");
            std::env::set_var("CALIBER_SKIP_REGISTRY", "1");
        }
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let journal_path = temp_dir.path().join("test_journal.md");
        std::fs::write(&journal_path, "").expect("Failed to create journal");

        let context = JournalContext::new(journal_path, None, JournalSlot::Hub);

        let config = Config::default();
        let app = App::new_with_context(config, date, context, None, Surface::default())
            .expect("Failed to create app");

        Self { app, temp_dir }
    }

    pub fn with_journal_content(date: NaiveDate, content: &str) -> Self {
        Self::with_config_and_content(date, content, Config::default())
    }

    pub fn with_config_and_content(date: NaiveDate, content: &str, config: Config) -> Self {
        // SAFETY: Tests run single-threaded per test file, env var is set before any other work
        unsafe {
            std::env::set_var("CALIBER_SKIP_CLIPBOARD", "1");
            std::env::set_var("CALIBER_SKIP_REGISTRY", "1");
        }
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let journal_path = temp_dir.path().join("test_journal.md");
        std::fs::write(&journal_path, content).expect("Failed to write journal");

        let context = JournalContext::new(journal_path, None, JournalSlot::Hub);
        let app = App::new_with_context(config, date, context, None, Surface::default())
            .expect("Failed to create app");

        Self { app, temp_dir }
    }

    pub fn press(&mut self, key: KeyCode) {
        let event = KeyEvent::new(key, KeyModifiers::NONE);
        self.handle_key_event(event);
    }

    pub fn press_with_modifiers(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        let event = KeyEvent::new(key, modifiers);
        self.handle_key_event(event);
    }

    pub fn type_str(&mut self, s: &str) {
        for c in s.chars() {
            self.press(KeyCode::Char(c));
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match &self.app.input_mode {
            InputMode::Normal => {
                let _ = handlers::handle_normal_key(&mut self.app, key);
            }
            InputMode::Edit(_) => {
                handlers::handle_edit_key(&mut self.app, key);
            }
            InputMode::Reorder => {
                handlers::handle_reorder_key(&mut self.app, key);
            }
            InputMode::Confirm(_) => {
                let _ = handlers::handle_confirm_key(&mut self.app, key.code);
            }
            InputMode::Selection(_) => {
                let _ = handlers::handle_selection_key(&mut self.app, key);
            }
            InputMode::CommandPalette(_) => {
                let _ = handlers::handle_command_palette_key(&mut self.app, key);
            }
            InputMode::FilterPrompt => {
                let _ = handlers::handle_filter_prompt_key(&mut self.app, key);
            }
        }
    }

    pub fn render_daily(&mut self) -> Vec<String> {
        let context = ui::RenderContext::for_test(80, 24);
        let _ = ui::prepare_render(&mut self.app, &context);
        ui::build_daily_list(&self.app, context.main_area.width as usize)
            .into_lines()
            .iter()
            .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    pub fn render_filter(&mut self) -> Vec<String> {
        let context = ui::RenderContext::for_test(80, 24);
        let _ = ui::prepare_render(&mut self.app, &context);
        ui::build_filter_list(&self.app, context.main_area.width as usize)
            .into_lines()
            .iter()
            .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    pub fn render_current(&mut self) -> Vec<String> {
        match &self.app.view {
            ViewMode::Daily(_) => self.render_daily(),
            ViewMode::Filter(_) => self.render_filter(),
        }
    }

    pub fn screen_contains(&mut self, text: &str) -> bool {
        self.render_current().iter().any(|line| line.contains(text))
    }

    pub fn find_line(&mut self, text: &str) -> Option<String> {
        self.render_current()
            .into_iter()
            .find(|line| line.contains(text))
    }

    pub fn status_contains(&self, text: &str) -> bool {
        self.app
            .status_message
            .as_ref()
            .is_some_and(|s| s.contains(text))
    }

    pub fn journal_path(&self) -> PathBuf {
        self.temp_dir.path().join("test_journal.md")
    }

    pub fn read_journal(&self) -> String {
        std::fs::read_to_string(self.journal_path()).unwrap_or_default()
    }

    pub fn cursor_position(&self) -> Option<usize> {
        self.app.edit_buffer.as_ref().map(|b| b.cursor_char_pos())
    }

    pub fn selected_index(&self) -> usize {
        match &self.app.view {
            ViewMode::Daily(state) => state.selected,
            ViewMode::Filter(state) => state.selected,
        }
    }

    pub fn entry_count(&self) -> usize {
        self.app.entry_indices.len()
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {}
}
