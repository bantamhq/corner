use std::io;

use chrono::{Days, Local, Months, NaiveDate};

use crate::storage::{self, Entry, EntryType, Line, RawEntry};

use super::{App, DailyState, InputMode, SelectedItem, ViewMode};

impl App {
    #[must_use]
    pub fn should_show_raw_entry(&self, entry: &RawEntry) -> bool {
        !self.hide_completed || !matches!(entry.entry_type, EntryType::Task { completed: true })
    }

    #[must_use]
    pub fn should_show_entry(&self, entry: &Entry) -> bool {
        !self.hide_completed || !matches!(entry.entry_type, EntryType::Task { completed: true })
    }

    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        self.view.scroll_offset()
    }

    pub fn scroll_offset_mut(&mut self) -> &mut usize {
        self.view.scroll_offset_mut()
    }

    #[must_use]
    pub fn visible_projected_count(&self) -> usize {
        let ViewMode::Daily(state) = &self.view else {
            return 0;
        };
        if self.hide_completed {
            state
                .projected_entries
                .iter()
                .filter(|e| self.should_show_entry(e))
                .count()
        } else {
            state.projected_entries.len()
        }
    }

    #[must_use]
    pub fn visible_entries_before(&self, entry_index: usize) -> usize {
        if !self.hide_completed {
            return entry_index;
        }
        self.entry_indices[..entry_index]
            .iter()
            .filter(|&&i| {
                if let Line::Entry(raw_entry) = &self.lines[i] {
                    self.should_show_raw_entry(raw_entry)
                } else {
                    true
                }
            })
            .count()
    }

    #[must_use]
    pub fn visible_projected_before(&self, projected_index: usize) -> usize {
        let ViewMode::Daily(state) = &self.view else {
            return 0;
        };
        if self.hide_completed {
            state.projected_entries[..projected_index]
                .iter()
                .filter(|e| self.should_show_entry(e))
                .count()
        } else {
            projected_index
        }
    }

    pub fn move_up(&mut self) {
        self.view.move_up();
    }

    pub fn move_down(&mut self) {
        let total = self.visible_entry_count();
        self.view.move_down(total);
    }

    pub fn jump_to_first(&mut self) {
        self.view.jump_to_first();
    }

    pub fn jump_to_last(&mut self) {
        let total = self.visible_entry_count();
        self.view.jump_to_last(total);
    }

    pub fn toggle_hide_completed(&mut self) {
        let ViewMode::Daily(state) = &mut self.view else {
            self.hide_completed = !self.hide_completed;
            return;
        };

        let old_selected = state.selected;

        if self.hide_completed {
            // Turning OFF hide - find where current selection maps to
            let new_selected = self.find_selection_after_unhide(old_selected);
            self.hide_completed = false;
            if let ViewMode::Daily(state) = &mut self.view {
                state.selected = new_selected;
                state.scroll_offset = 0;
            }
        } else {
            // Turning ON hide - find where current selection maps to
            self.hide_completed = true;
            let new_selected = self.find_selection_after_hide(old_selected);
            if let ViewMode::Daily(state) = &mut self.view {
                state.selected = new_selected;
                state.scroll_offset = 0;
            }
        }
    }

    fn find_selection_after_unhide(&self, old_visible_idx: usize) -> usize {
        let ViewMode::Daily(state) = &self.view else {
            return 0;
        };

        // When hide was ON, only non-completed entries were visible
        // Find which actual entry was at old_visible_idx, return its actual index
        let mut visible_idx = 0;
        let mut actual_idx = 0;

        // Check projected entries
        for entry in &state.projected_entries {
            let is_completed = matches!(entry.entry_type, EntryType::Task { completed: true });
            if !is_completed {
                if visible_idx == old_visible_idx {
                    return actual_idx;
                }
                visible_idx += 1;
            }
            actual_idx += 1;
        }

        // Check regular entries
        for &line_idx in &self.entry_indices {
            if let Line::Entry(raw_entry) = &self.lines[line_idx] {
                let is_completed =
                    matches!(raw_entry.entry_type, EntryType::Task { completed: true });
                if !is_completed {
                    if visible_idx == old_visible_idx {
                        return actual_idx;
                    }
                    visible_idx += 1;
                }
                actual_idx += 1;
            }
        }

        // Fallback to same index
        old_visible_idx
    }

    fn find_selection_after_hide(&self, old_visible_idx: usize) -> usize {
        let ViewMode::Daily(state) = &self.view else {
            return 0;
        };

        // When hide was OFF, all entries were visible, so old_visible_idx = actual position
        // Now hide is ON, find where that entry maps to (or first visible after it)
        let mut actual_idx = 0;
        let mut new_visible_idx = 0;
        let mut found_target = false;
        let mut first_visible_at_or_after = None;

        // Check projected entries
        for entry in &state.projected_entries {
            let is_visible_now = self.should_show_entry(entry);

            if actual_idx == old_visible_idx {
                found_target = true;
                if is_visible_now {
                    return new_visible_idx;
                }
            }

            if is_visible_now {
                if found_target && first_visible_at_or_after.is_none() {
                    first_visible_at_or_after = Some(new_visible_idx);
                }
                new_visible_idx += 1;
            }
            actual_idx += 1;
        }

        // Check regular entries
        for &line_idx in &self.entry_indices {
            if let Line::Entry(raw_entry) = &self.lines[line_idx] {
                let is_visible_now = self.should_show_raw_entry(raw_entry);

                if actual_idx == old_visible_idx {
                    found_target = true;
                    if is_visible_now {
                        return new_visible_idx;
                    }
                }

                if is_visible_now {
                    if found_target && first_visible_at_or_after.is_none() {
                        first_visible_at_or_after = Some(new_visible_idx);
                    }
                    new_visible_idx += 1;
                }
                actual_idx += 1;
            }
        }

        // Return first visible entry at or after the old selection, or 0
        first_visible_at_or_after.unwrap_or(0)
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

    #[must_use]
    pub fn get_selected_item(&self) -> SelectedItem<'_> {
        match &self.view {
            ViewMode::Daily(state) => {
                let mut visible_idx = 0;

                for (actual_idx, projected_entry) in state.projected_entries.iter().enumerate() {
                    if !self.should_show_entry(projected_entry) {
                        continue;
                    }
                    if visible_idx == state.selected {
                        return SelectedItem::Projected {
                            index: actual_idx,
                            entry: projected_entry,
                        };
                    }
                    visible_idx += 1;
                }

                for (actual_idx, &line_idx) in self.entry_indices.iter().enumerate() {
                    if let Line::Entry(raw_entry) = &self.lines[line_idx] {
                        if !self.should_show_raw_entry(raw_entry) {
                            continue;
                        }
                        if visible_idx == state.selected {
                            return SelectedItem::Daily {
                                index: actual_idx,
                                line_idx,
                                entry: raw_entry,
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
                    return state.projected_entries.len() + self.entry_indices.len();
                }

                let visible_projected = state
                    .projected_entries
                    .iter()
                    .filter(|e| self.should_show_entry(e))
                    .count();
                let visible_regular = self
                    .entry_indices
                    .iter()
                    .filter(|&&i| {
                        if let Line::Entry(raw_entry) = &self.lines[i] {
                            self.should_show_raw_entry(raw_entry)
                        } else {
                            true
                        }
                    })
                    .count();
                visible_projected + visible_regular
            }
        }
    }

    #[must_use]
    pub fn hidden_completed_count(&self) -> usize {
        let ViewMode::Daily(state) = &self.view else {
            return 0;
        };

        let hidden_calendar = self
            .calendar_store
            .events_for_date(self.current_date)
            .iter()
            .filter(|e| e.is_past())
            .count();
        let hidden_projected = state
            .projected_entries
            .iter()
            .filter(|e| matches!(e.entry_type, EntryType::Task { completed: true }))
            .count();
        let hidden_regular = self
            .entry_indices
            .iter()
            .filter(|&&i| {
                if let Line::Entry(raw_entry) = &self.lines[i] {
                    matches!(raw_entry.entry_type, EntryType::Task { completed: true })
                } else {
                    false
                }
            })
            .count();
        hidden_calendar + hidden_projected + hidden_regular
    }

    #[must_use]
    pub(super) fn actual_to_visible_index(&self, actual_entry_idx: usize) -> usize {
        let ViewMode::Daily(state) = &self.view else {
            return 0;
        };

        let visible_projected = if self.hide_completed {
            state
                .projected_entries
                .iter()
                .filter(|e| self.should_show_entry(e))
                .count()
        } else {
            state.projected_entries.len()
        };

        let visible_before = self.entry_indices[..actual_entry_idx]
            .iter()
            .filter(|&&i| {
                if self.hide_completed {
                    if let Line::Entry(raw_entry) = &self.lines[i] {
                        self.should_show_raw_entry(raw_entry)
                    } else {
                        true
                    }
                } else {
                    true
                }
            })
            .count();

        visible_projected + visible_before
    }

    pub(super) fn load_day(&mut self, date: NaiveDate) -> io::Result<Vec<Entry>> {
        self.current_date = date;
        let path = self.active_path().to_path_buf();
        self.lines = storage::load_day_lines(date, &path)?;
        self.entry_indices = Self::compute_entry_indices(&self.lines);
        storage::collect_projected_entries_for_date(date, &path)
    }

    pub fn refresh_projected_entries(&mut self) {
        let projected =
            storage::collect_projected_entries_for_date(self.current_date, self.active_path())
                .unwrap_or_default();
        if let ViewMode::Daily(state) = &mut self.view {
            state.projected_entries = projected;
        }
        self.clamp_selection_to_visible();
    }

    pub(super) fn finalize_view_switch(&mut self) {
        self.input_mode = InputMode::Normal;
        self.executor.clear();
    }

    pub(super) fn reset_daily_view(&mut self, date: NaiveDate) -> io::Result<()> {
        let projected_entries = self.load_day(date)?;
        self.view = ViewMode::Daily(DailyState::new(self.entry_indices.len(), projected_entries));
        if self.hide_completed {
            self.clamp_selection_to_visible();
        }
        self.finalize_view_switch();
        Ok(())
    }

    pub(super) fn restore_daily_view(&mut self) {
        let projected_entries =
            storage::collect_projected_entries_for_date(self.current_date, self.active_path())
                .unwrap_or_default();
        self.view = ViewMode::Daily(DailyState::new(self.entry_indices.len(), projected_entries));
        if self.hide_completed {
            self.clamp_selection_to_visible();
        }
        self.finalize_view_switch();
    }

    pub fn goto_day(&mut self, date: NaiveDate) -> io::Result<()> {
        let in_filter = matches!(self.view, ViewMode::Filter(_));
        if date == self.current_date && !in_filter {
            return Ok(());
        }

        self.save();
        self.reset_daily_view(date)?;
        self.edit_buffer = None;
        self.last_daily_date = date;
        self.sync_calendar_state(date);

        Ok(())
    }

    fn navigate_by<F>(&mut self, calc: F) -> io::Result<()>
    where
        F: FnOnce(NaiveDate) -> Option<NaiveDate>,
    {
        if let Some(new_date) = calc(self.current_date) {
            self.goto_day(new_date)?;
        }
        Ok(())
    }

    pub fn prev_day(&mut self) -> io::Result<()> {
        self.navigate_by(|d| d.checked_sub_days(Days::new(1)))
    }

    pub fn next_day(&mut self) -> io::Result<()> {
        self.navigate_by(|d| d.checked_add_days(Days::new(1)))
    }

    pub fn goto_today(&mut self) -> io::Result<()> {
        self.goto_day(Local::now().date_naive())
    }

    pub fn prev_week(&mut self) -> io::Result<()> {
        self.navigate_by(|d| d.checked_sub_days(Days::new(7)))
    }

    pub fn next_week(&mut self) -> io::Result<()> {
        self.navigate_by(|d| d.checked_add_days(Days::new(7)))
    }

    pub fn prev_month(&mut self) -> io::Result<()> {
        self.navigate_by(|d| {
            d.checked_sub_months(Months::new(1))
                .map(|m| super::calendar::clamp_day_to_month(d, m))
        })
    }

    pub fn next_month(&mut self) -> io::Result<()> {
        self.navigate_by(|d| {
            d.checked_add_months(Months::new(1))
                .map(|m| super::calendar::clamp_day_to_month(d, m))
        })
    }

    pub fn prev_year(&mut self) -> io::Result<()> {
        self.navigate_by(|d| {
            d.checked_sub_months(Months::new(12))
                .map(|m| super::calendar::clamp_day_to_month(d, m))
        })
    }

    pub fn next_year(&mut self) -> io::Result<()> {
        self.navigate_by(|d| {
            d.checked_add_months(Months::new(12))
                .map(|m| super::calendar::clamp_day_to_month(d, m))
        })
    }

    /// Navigate to the source date and select the entry at the given line index.
    /// Used for projected entries to jump to their source.
    pub fn go_to_source(&mut self, source_date: NaiveDate, line_index: usize) -> io::Result<()> {
        self.goto_day(source_date)?;

        if let Some(actual_idx) = self.entry_indices.iter().position(|&idx| idx == line_index) {
            let visible_idx = self.actual_to_visible_index(actual_idx);
            if let ViewMode::Daily(state) = &mut self.view {
                state.selected = visible_idx;
            }
        }

        Ok(())
    }
}
