use std::io;

use chrono::{Days, Local, NaiveDate};

use crate::storage::{self, Entry, EntryType, LaterEntry, Line};

use super::{App, DailyState, InputMode, SelectedItem, ViewMode};

impl App {
    /// Helper to check if an entry should be shown given hide_completed setting.
    #[must_use]
    pub fn should_show_entry(&self, entry: &Entry) -> bool {
        !self.hide_completed || !matches!(entry.entry_type, EntryType::Task { completed: true })
    }

    /// Helper to check if a later entry should be shown.
    #[must_use]
    pub fn should_show_later(&self, entry: &LaterEntry) -> bool {
        !self.hide_completed || !entry.completed
    }

    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        match &self.view {
            ViewMode::Filter(state) => state.scroll_offset,
            ViewMode::Daily(state) => state.scroll_offset,
        }
    }

    pub fn scroll_offset_mut(&mut self) -> &mut usize {
        match &mut self.view {
            ViewMode::Filter(state) => &mut state.scroll_offset,
            ViewMode::Daily(state) => &mut state.scroll_offset,
        }
    }

    /// Count visible later entries (accounting for hide_completed).
    #[must_use]
    pub fn visible_later_count(&self) -> usize {
        let ViewMode::Daily(state) = &self.view else {
            return 0;
        };
        if self.hide_completed {
            state.later_entries.iter().filter(|e| !e.completed).count()
        } else {
            state.later_entries.len()
        }
    }

    /// Count visible entries before the given index (accounting for hide_completed).
    #[must_use]
    pub fn visible_entries_before(&self, entry_index: usize) -> usize {
        if !self.hide_completed {
            return entry_index;
        }
        self.entry_indices[..entry_index]
            .iter()
            .filter(|&&i| {
                if let Line::Entry(entry) = &self.lines[i] {
                    self.should_show_entry(entry)
                } else {
                    true
                }
            })
            .count()
    }

    /// Count visible later entries before the given index (accounting for hide_completed).
    #[must_use]
    pub fn visible_later_before(&self, later_index: usize) -> usize {
        let ViewMode::Daily(state) = &self.view else {
            return 0;
        };
        if self.hide_completed {
            state.later_entries[..later_index]
                .iter()
                .filter(|e| !e.completed)
                .count()
        } else {
            later_index
        }
    }

    pub fn move_up(&mut self) {
        match &mut self.view {
            ViewMode::Daily(state) => {
                state.selected = state.selected.saturating_sub(1);
            }
            ViewMode::Filter(state) => {
                state.selected = state.selected.saturating_sub(1);
            }
        }
    }

    pub fn move_down(&mut self) {
        let total = self.visible_entry_count();
        if let ViewMode::Daily(state) = &mut self.view
            && total > 0
            && state.selected < total - 1
        {
            state.selected += 1;
        } else if let ViewMode::Filter(state) = &mut self.view
            && !state.entries.is_empty()
            && state.selected < state.entries.len() - 1
        {
            state.selected += 1;
        }
    }

    pub fn jump_to_first(&mut self) {
        match &mut self.view {
            ViewMode::Daily(state) => state.selected = 0,
            ViewMode::Filter(state) => state.selected = 0,
        }
    }

    pub fn jump_to_last(&mut self) {
        let total = self.visible_entry_count();
        if let ViewMode::Daily(state) = &mut self.view
            && total > 0
        {
            state.selected = total - 1;
        } else if let ViewMode::Filter(state) = &mut self.view
            && !state.entries.is_empty()
        {
            state.selected = state.entries.len() - 1;
        }
    }

    pub fn toggle_hide_completed(&mut self) {
        self.hide_completed = !self.hide_completed;

        if self.hide_completed {
            self.clamp_selection_to_visible();
        }
    }

    pub(super) fn clamp_selection_to_visible(&mut self) {
        let visible_count = self.visible_entry_count();

        let ViewMode::Daily(state) = &mut self.view else {
            return;
        };

        if visible_count == 0 {
            state.selected = 0;
            state.scroll_offset = 0;
        } else if state.selected >= visible_count {
            state.selected = visible_count.saturating_sub(1);
        }
        state.scroll_offset = 0;
    }

    /// Returns the currently selected item, accounting for hidden completed entries.
    #[must_use]
    pub fn get_selected_item(&self) -> SelectedItem<'_> {
        match &self.view {
            ViewMode::Daily(state) => {
                let mut visible_idx = 0;

                for (actual_idx, later_entry) in state.later_entries.iter().enumerate() {
                    if !self.should_show_later(later_entry) {
                        continue;
                    }
                    if visible_idx == state.selected {
                        return SelectedItem::Later {
                            index: actual_idx,
                            entry: later_entry,
                        };
                    }
                    visible_idx += 1;
                }

                for (actual_idx, &line_idx) in self.entry_indices.iter().enumerate() {
                    if let Line::Entry(entry) = &self.lines[line_idx] {
                        if !self.should_show_entry(entry) {
                            continue;
                        }
                        if visible_idx == state.selected {
                            return SelectedItem::Daily {
                                index: actual_idx,
                                line_idx,
                                entry,
                            };
                        }
                        visible_idx += 1;
                    }
                }

                SelectedItem::None
            }
            ViewMode::Filter(state) => match state.entries.get(state.selected) {
                Some(entry) => SelectedItem::Filter {
                    index: state.selected,
                    entry,
                },
                None => SelectedItem::None,
            },
        }
    }

    #[must_use]
    pub fn visible_entry_count(&self) -> usize {
        match &self.view {
            ViewMode::Filter(state) => state.entries.len(),
            ViewMode::Daily(state) => {
                if !self.hide_completed {
                    return state.later_entries.len() + self.entry_indices.len();
                }

                let visible_later = state
                    .later_entries
                    .iter()
                    .filter(|e| self.should_show_later(e))
                    .count();
                let visible_regular = self
                    .entry_indices
                    .iter()
                    .filter(|&&i| {
                        if let Line::Entry(entry) = &self.lines[i] {
                            self.should_show_entry(entry)
                        } else {
                            true
                        }
                    })
                    .count();
                visible_later + visible_regular
            }
        }
    }

    #[must_use]
    pub fn hidden_completed_count(&self) -> usize {
        let ViewMode::Daily(state) = &self.view else {
            return 0;
        };

        let hidden_later = state.later_entries.iter().filter(|e| e.completed).count();
        let hidden_regular = self
            .entry_indices
            .iter()
            .filter(|&&i| {
                if let Line::Entry(entry) = &self.lines[i] {
                    matches!(entry.entry_type, EntryType::Task { completed: true })
                } else {
                    false
                }
            })
            .count();
        hidden_later + hidden_regular
    }

    /// Converts an actual entry index to a visible index (accounting for later entries and hidden completed)
    #[must_use]
    pub(super) fn actual_to_visible_index(&self, actual_entry_idx: usize) -> usize {
        let ViewMode::Daily(state) = &self.view else {
            return 0;
        };

        let visible_later = if self.hide_completed {
            state
                .later_entries
                .iter()
                .filter(|e| self.should_show_later(e))
                .count()
        } else {
            state.later_entries.len()
        };

        let visible_before = self.entry_indices[..actual_entry_idx]
            .iter()
            .filter(|&&i| {
                if self.hide_completed {
                    if let Line::Entry(entry) = &self.lines[i] {
                        self.should_show_entry(entry)
                    } else {
                        true
                    }
                } else {
                    true
                }
            })
            .count();

        visible_later + visible_before
    }

    /// Load a day's data into self, returning later entries for view construction.
    pub(super) fn load_day(&mut self, date: NaiveDate) -> io::Result<Vec<LaterEntry>> {
        self.current_date = date;
        let path = self.active_path().to_path_buf();
        self.lines = storage::load_day_lines(date, &path)?;
        self.entry_indices = Self::compute_entry_indices(&self.lines);
        storage::collect_later_entries_for_date(date, &path)
    }

    /// Load a day and reset to daily view with proper selection clamping
    pub(super) fn reset_daily_view(&mut self, date: NaiveDate) -> io::Result<()> {
        let later_entries = self.load_day(date)?;
        self.view = ViewMode::Daily(DailyState::new(self.entry_indices.len(), later_entries));
        if self.hide_completed {
            self.clamp_selection_to_visible();
        }
        Ok(())
    }

    pub fn goto_day(&mut self, date: NaiveDate) -> io::Result<()> {
        if date == self.current_date {
            return Ok(());
        }

        self.save();
        self.reset_daily_view(date)?;
        self.edit_buffer = None;
        self.input_mode = InputMode::Normal;

        Ok(())
    }

    pub fn prev_day(&mut self) -> io::Result<()> {
        if let Some(prev) = self.current_date.checked_sub_days(Days::new(1)) {
            self.goto_day(prev)?;
        }
        Ok(())
    }

    pub fn next_day(&mut self) -> io::Result<()> {
        if let Some(next) = self.current_date.checked_add_days(Days::new(1)) {
            self.goto_day(next)?;
        }
        Ok(())
    }

    pub fn goto_today(&mut self) -> io::Result<()> {
        self.goto_day(Local::now().date_naive())
    }

    /// Parses a date string for the :goto command.
    /// Supports natural language (tomorrow, yesterday, next-mon, 3d, -3d) and
    /// standard formats (YYYY/MM/DD, MM/DD/YYYY, MM/DD/YY, MM/DD).
    #[must_use]
    pub fn parse_goto_date(input: &str) -> Option<NaiveDate> {
        storage::parse_natural_date(input, Local::now().date_naive())
    }
}
