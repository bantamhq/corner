use std::io;

use chrono::{Days, Local, Months, NaiveDate};

use crate::storage::{self, Entry, EntryType, Line, RawEntry, strip_recurring_tags};

use super::{App, DailyState, InputMode, SelectedItem, ViewMode};

/// Filter out recurring entries that have been "done today" (have a matching ↺ entry).
/// When a recurring entry is toggled complete, a materialized copy with ↺ prefix is created.
/// This hides the projected recurring entry if such a copy exists.
pub(super) fn filter_done_today_recurring(projected: Vec<Entry>, lines: &[Line]) -> Vec<Entry> {
    let local_contents: Vec<&str> = lines
        .iter()
        .filter_map(|line| match line {
            Line::Entry(raw) => Some(raw.content.as_str()),
            _ => None,
        })
        .collect();

    projected
        .into_iter()
        .filter(|entry| {
            let expected_content = format!("↺ {}", strip_recurring_tags(&entry.content));
            !local_contents.iter().any(|&c| c == expected_content)
        })
        .collect()
}

impl App {
    /// Helper to check if a raw entry should be shown given hide_completed setting.
    #[must_use]
    pub fn should_show_raw_entry(&self, entry: &RawEntry) -> bool {
        !self.hide_completed || !matches!(entry.entry_type, EntryType::Task { completed: true })
    }

    /// Helper to check if an entry should be shown given hide_completed setting.
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

    /// Count visible projected entries (accounting for hide_completed).
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

    /// Count visible entries before the given index (accounting for hide_completed).
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

    /// Count visible projected entries before the given index (accounting for hide_completed).
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
        hidden_projected + hidden_regular
    }

    /// Converts an actual entry index to a visible index (accounting for projected entries and hidden completed)
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
        let projected = filter_done_today_recurring(projected, &self.lines);
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
        let projected_entries = filter_done_today_recurring(projected_entries, &self.lines);
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
        let projected_entries = filter_done_today_recurring(projected_entries, &self.lines);
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

        // Find and select the entry at the given line index
        if let Some(actual_idx) = self.entry_indices.iter().position(|&idx| idx == line_index) {
            let visible_idx = self.actual_to_visible_index(actual_idx);
            if let ViewMode::Daily(state) = &mut self.view {
                state.selected = visible_idx;
            }
        }

        Ok(())
    }
}
