use std::io;

use chrono::Local;

use crate::cursor::CursorBuffer;
use crate::storage::{self, EntryType};

use super::{App, EditContext, FilterState, InputMode, ViewMode};

impl App {
    /// Switch to filter view with the given query.
    fn reset_filter_view(&mut self, query: String) -> io::Result<()> {
        let (query, unknown_filters) = storage::expand_saved_filters(&query, &self.config.filters);
        let mut filter = storage::parse_filter_query(&query);
        filter.invalid_tokens.extend(unknown_filters);

        if !filter.invalid_tokens.is_empty() {
            self.set_status(format!(
                "Unknown filter: {}",
                filter.invalid_tokens.join(", ")
            ));
        }

        let entries = storage::collect_filtered_entries(&filter, self.active_path())?;
        let selected = entries.len().saturating_sub(1);

        self.view = ViewMode::Filter(FilterState {
            query_buffer: CursorBuffer::new(query.clone()),
            query,
            entries,
            selected,
            scroll_offset: 0,
        });
        self.finalize_view_switch();

        if self.combined_view {
            self.load_combined_filter()?;
        }
        Ok(())
    }

    pub fn execute_filter(&mut self) -> io::Result<()> {
        self.save();
        let query = self
            .last_filter_query
            .clone()
            .unwrap_or_else(|| self.config.default_filter.clone());
        self.input_mode = InputMode::Normal;
        self.reset_filter_view(query)
    }

    pub fn quick_filter(&mut self, query: &str) -> io::Result<()> {
        self.save();
        self.reset_filter_view(query.to_string())
    }

    pub fn cancel_filter(&mut self) {
        if let ViewMode::Filter(state) = &self.view {
            self.last_filter_query = Some(state.query.clone());
        }
        self.restore_daily_view();
        if self.combined_view {
            let _ = self.load_combined_daily();
        }
    }

    pub fn return_to_filter(&mut self) -> io::Result<()> {
        let query = self
            .last_filter_query
            .clone()
            .unwrap_or_else(|| self.config.default_filter.clone());
        self.quick_filter(&query)
    }

    pub fn refresh_filter(&mut self) -> io::Result<()> {
        if self.combined_view {
            self.load_combined_filter()?;
            return Ok(());
        }

        let path = self.active_path().to_path_buf();
        let ViewMode::Filter(state) = &mut self.view else {
            return Ok(());
        };

        let filter = storage::parse_filter_query(&state.query);
        let error_msg = if filter.invalid_tokens.is_empty() {
            None
        } else {
            Some(format!(
                "Unknown filter: {}",
                filter.invalid_tokens.join(", ")
            ))
        };

        state.entries = storage::collect_filtered_entries(&filter, &path)?;
        state.selected = state.selected.min(state.entries.len().saturating_sub(1));
        state.scroll_offset = 0;

        if let Some(msg) = error_msg {
            self.set_error(msg);
        }
        Ok(())
    }

    pub fn filter_quick_add(&mut self) {
        if self.combined_view {
            self.set_error("Switch to a journal to add entries");
            return;
        }
        let today = Local::now().date_naive();
        self.original_edit_content = Some(String::new());
        self.edit_buffer = Some(CursorBuffer::empty());
        self.input_mode = InputMode::Edit(EditContext::FilterQuickAdd {
            date: today,
            entry_type: EntryType::Task { completed: false },
        });
    }

    #[must_use]
    pub fn filter_visual_line(&self) -> usize {
        let ViewMode::Filter(state) = &self.view else {
            return 0;
        };
        state.selected
    }

    #[must_use]
    pub fn filter_total_lines(&self) -> usize {
        let ViewMode::Filter(state) = &self.view else {
            return 1;
        };
        state.entries.len()
    }

    pub fn cycle_view(&mut self) -> io::Result<()> {
        match &self.view {
            ViewMode::Daily(_) => self.execute_filter()?,
            ViewMode::Filter(_) => self.cancel_filter(),
        }
        Ok(())
    }

    pub fn enter_filter_prompt(&mut self) -> io::Result<()> {
        if matches!(self.view, ViewMode::Daily(_)) {
            self.execute_filter()?;
        }

        if let ViewMode::Filter(state) = &mut self.view {
            let mut query = state.query.clone();
            if !query.is_empty() {
                query.push(' ');
            }
            state.query_buffer = CursorBuffer::new(query);
        }

        self.input_mode = InputMode::FilterPrompt;
        self.refresh_tag_cache();
        self.update_hints();
        Ok(())
    }

    pub fn submit_filter_prompt(&mut self) -> io::Result<()> {
        let ViewMode::Filter(state) = &mut self.view else {
            self.input_mode = InputMode::Normal;
            return Ok(());
        };

        let new_query = state.query_buffer.content().trim().to_string();

        let (expanded, unknown_filters) =
            storage::expand_saved_filters(&new_query, &self.config.filters);
        let mut filter = storage::parse_filter_query(&expanded);
        filter.invalid_tokens.extend(unknown_filters);

        if !filter.invalid_tokens.is_empty() {
            self.set_error(format!(
                "Unknown filter: {}",
                filter.invalid_tokens.join(", ")
            ));
            return Ok(());
        }

        let needs_refresh = new_query != state.query;
        state.query = new_query;

        if needs_refresh {
            self.refresh_filter()?;
        }

        if let ViewMode::Filter(state) = &self.view {
            self.last_filter_query = Some(state.query.clone());
        }

        self.clear_hints();
        self.input_mode = InputMode::Normal;
        Ok(())
    }

    pub fn cancel_filter_prompt(&mut self) {
        if let ViewMode::Filter(state) = &mut self.view {
            state.query_buffer = CursorBuffer::new(state.query.clone());
        }
        self.clear_hints();
        self.input_mode = InputMode::Normal;
    }
}
