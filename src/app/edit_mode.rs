use crate::cursor::CursorBuffer;
use crate::storage::{self, Entry, EntryType, Line, RawEntry, SourceType};

use super::actions::{CreateEntry, CreateTarget, EditEntry, EditTarget};
use super::{App, EditContext, EntryLocation, InputMode, InsertPosition, ViewMode};

impl App {
    pub fn cycle_edit_entry_type(&mut self) {
        match &mut self.input_mode {
            InputMode::Edit(EditContext::Daily { entry_index }) => {
                let line_idx = match self.entry_indices.get(*entry_index) {
                    Some(&idx) => idx,
                    None => return,
                };
                if let Line::Entry(entry) = &mut self.lines[line_idx] {
                    entry.entry_type = entry.entry_type.cycle();
                }
            }
            InputMode::Edit(EditContext::FilterEdit {
                date,
                line_index,
                filter_index,
            }) => {
                let date = *date;
                let line_index = *line_index;
                let filter_index = *filter_index;
                let path = self.active_path();

                if let Ok(Some(new_type)) = storage::cycle_entry_type(date, path, line_index)
                    && let ViewMode::Filter(state) = &mut self.view
                    && let Some(filter_entry) = state.entries.get_mut(filter_index)
                {
                    filter_entry.entry_type = new_type;
                    if date == self.current_date {
                        let _ = self.reload_current_day();
                    }
                }
            }
            InputMode::Edit(EditContext::FilterQuickAdd { entry_type, .. }) => {
                *entry_type = entry_type.cycle();
            }
            InputMode::Edit(EditContext::LaterEdit {
                source_date,
                line_index,
            }) => {
                let source_date = *source_date;
                let line_index = *line_index;
                let path = self.active_path().to_path_buf();

                if let Ok(Some(_new_type)) =
                    storage::cycle_entry_type(source_date, &path, line_index)
                {
                    self.refresh_projected_entries();
                }
            }
            _ => {}
        }
    }

    fn save_current_edit(&mut self) -> Option<(EditContext, bool)> {
        let buffer = self.edit_buffer.take()?;
        let (new_content, warning) = self.normalize_content(&buffer.into_content());
        let had_content = !new_content.trim().is_empty();

        if let Some(warning) = warning {
            self.set_status(warning);
        }
        let original_content = self.original_edit_content.take().unwrap_or_default();
        let is_new_entry = original_content.is_empty();

        let context = match std::mem::replace(&mut self.input_mode, InputMode::Normal) {
            InputMode::Edit(ctx) => ctx,
            _ => return None,
        };

        match &context {
            EditContext::Daily { entry_index } => {
                self.save_daily_edit_with_action(
                    *entry_index,
                    new_content,
                    original_content,
                    is_new_entry,
                );
            }
            EditContext::FilterEdit {
                date,
                line_index,
                filter_index,
            } => {
                self.save_filter_edit_with_action(
                    *date,
                    *line_index,
                    *filter_index,
                    new_content,
                    original_content,
                );
            }
            EditContext::FilterQuickAdd { date, entry_type } => {
                self.save_filter_quick_add_with_action(*date, entry_type.clone(), new_content);
            }
            EditContext::LaterEdit {
                source_date,
                line_index,
            } => {
                self.save_later_edit_with_action(
                    *source_date,
                    *line_index,
                    new_content,
                    original_content,
                );
            }
        }

        Some((context, had_content))
    }

    pub fn exit_edit(&mut self) {
        if self.save_current_edit().is_none() {
            self.input_mode = InputMode::Normal;
        }
    }

    fn save_daily_edit_with_action(
        &mut self,
        entry_index: usize,
        new_content: String,
        original_content: String,
        is_new_entry: bool,
    ) {
        let line_idx = match self.entry_indices.get(entry_index) {
            Some(&idx) => idx,
            None => return,
        };

        if new_content.trim().is_empty() {
            self.delete_at_index_daily(entry_index);
            if let ViewMode::Daily(state) = &mut self.view {
                state.scroll_offset = 0;
            }
            return;
        }

        let entry_type = if let Line::Entry(entry) = &self.lines[line_idx] {
            entry.entry_type.clone()
        } else {
            return;
        };

        if let Some(entry) = self.get_daily_entry_mut(entry_index) {
            entry.content = new_content.clone();
            self.save();
        }

        if is_new_entry {
            if let Line::Entry(raw_entry) = &self.lines[line_idx] {
                let entry =
                    Entry::from_raw(raw_entry, self.current_date, line_idx, SourceType::Local);
                let target = CreateTarget {
                    date: self.current_date,
                    line_index: line_idx,
                    entry,
                    is_filter_quick_add: false,
                };
                let action = CreateEntry::new(target);
                let _ = self.execute_action(Box::new(action));
            }
        } else if original_content != new_content {
            let target = EditTarget {
                location: EntryLocation::Daily { line_idx },
                original_content,
                new_content,
                entry_type,
            };
            let action = EditEntry::new(target);
            let _ = self.execute_action(Box::new(action));
        }
    }

    fn save_filter_edit_with_action(
        &mut self,
        date: chrono::NaiveDate,
        line_index: usize,
        filter_index: usize,
        new_content: String,
        original_content: String,
    ) {
        let path = self.active_path().to_path_buf();

        if new_content.trim().is_empty() {
            let _ = storage::delete_entry(date, &path, line_index);
        } else if let Some((entry_type, new_content)) =
            self.update_remote_entry(date, line_index, new_content, &original_content)
        {
            let entry = Entry {
                entry_type: entry_type.clone(),
                content: original_content.clone(),
                source_date: date,
                line_index,
                source_type: SourceType::Local,
            };
            let target = EditTarget {
                location: EntryLocation::Filter {
                    index: filter_index,
                    entry,
                },
                original_content,
                new_content,
                entry_type,
            };
            let action = EditEntry::new(target);
            let _ = self.execute_action(Box::new(action));
        }

        if date == self.current_date {
            let _ = self.reload_current_day();
        }
        let _ = self.refresh_filter();
    }

    fn save_filter_quick_add_with_action(
        &mut self,
        date: chrono::NaiveDate,
        entry_type: EntryType,
        content: String,
    ) {
        let path = self.active_path().to_path_buf();

        if !content.trim().is_empty()
            && let Ok(mut lines) = storage::load_day_lines(date, &path)
        {
            let line_index = lines.len();
            let raw_entry = RawEntry {
                entry_type: entry_type.clone(),
                content: content.clone(),
            };
            lines.push(Line::Entry(raw_entry));
            let _ = storage::save_day_lines(date, &path, &lines);

            let entry = Entry {
                entry_type,
                content,
                source_date: date,
                line_index,
                source_type: SourceType::Local,
            };
            let target = CreateTarget {
                date,
                line_index,
                entry,
                is_filter_quick_add: true,
            };
            let action = CreateEntry::new(target);
            let _ = self.execute_action(Box::new(action));

            if date == self.current_date {
                let _ = self.reload_current_day();
            }
        }

        let _ = self.refresh_filter();
        if let ViewMode::Filter(state) = &mut self.view {
            state.selected = state.entries.len().saturating_sub(1);
        }
    }

    fn save_later_edit_with_action(
        &mut self,
        source_date: chrono::NaiveDate,
        line_index: usize,
        new_content: String,
        original_content: String,
    ) {
        let path = self.active_path().to_path_buf();

        if new_content.trim().is_empty() {
            let _ = storage::delete_entry(source_date, &path, line_index);
        } else if let Some((entry_type, new_content)) =
            self.update_remote_entry(source_date, line_index, new_content, &original_content)
        {
            let entry = Entry {
                entry_type: entry_type.clone(),
                content: original_content.clone(),
                source_date,
                line_index,
                source_type: SourceType::Later,
            };
            let target = EditTarget {
                location: EntryLocation::Projected(entry),
                original_content,
                new_content,
                entry_type,
            };
            let action = EditEntry::new(target);
            let _ = self.execute_action(Box::new(action));
        }

        self.refresh_projected_entries();
    }

    fn update_remote_entry(
        &mut self,
        date: chrono::NaiveDate,
        line_index: usize,
        new_content: String,
        original_content: &str,
    ) -> Option<(EntryType, String)> {
        let path = self.active_path().to_path_buf();
        let entry_type = storage::get_entry_type(date, &path, line_index);

        match storage::update_entry_content(date, &path, line_index, new_content.clone()) {
            Ok(false) => {
                self.set_error(format!(
                    "Failed to update: no entry at index {line_index} for {date}"
                ));
                None
            }
            Err(e) => {
                self.set_error(format!("Failed to save: {e}"));
                None
            }
            Ok(true) if original_content != new_content => Some((entry_type, new_content)),
            Ok(true) => None,
        }
    }

    pub fn cancel_edit_mode(&mut self) {
        self.edit_buffer = None;
        self.original_edit_content = None;

        if let InputMode::Edit(EditContext::Daily { entry_index }) =
            std::mem::replace(&mut self.input_mode, InputMode::Normal)
            && let Some(entry) = self.get_daily_entry(entry_index)
            && entry.content.is_empty()
        {
            self.delete_at_index_daily(entry_index);
            if let ViewMode::Daily(state) = &mut self.view {
                state.scroll_offset = 0;
            }
        }
    }

    pub fn commit_and_add_new(&mut self) {
        let Some((context, had_content)) = self.save_current_edit() else {
            return;
        };

        match context {
            EditContext::Daily { entry_index } if had_content => {
                let entry_type = self
                    .get_daily_entry(entry_index)
                    .map(|e| e.entry_type.clone())
                    .unwrap_or(EntryType::Task { completed: false });

                let new_raw_entry = RawEntry {
                    entry_type: match entry_type {
                        EntryType::Task { .. } => EntryType::Task { completed: false },
                        other => other,
                    },
                    content: String::new(),
                };
                self.add_entry_internal(new_raw_entry, InsertPosition::Below);
            }
            EditContext::FilterQuickAdd { date, entry_type } => {
                self.original_edit_content = Some(String::new());
                self.edit_buffer = Some(CursorBuffer::empty());
                self.input_mode = InputMode::Edit(EditContext::FilterQuickAdd {
                    date,
                    entry_type: match entry_type {
                        EntryType::Task { .. } => EntryType::Task { completed: false },
                        other => other,
                    },
                });
            }
            _ => {}
        }
    }

    pub(super) fn add_entry_internal(&mut self, entry: RawEntry, position: InsertPosition) {
        use super::SelectedItem;

        let insert_pos =
            if matches!(position, InsertPosition::Bottom) || self.entry_indices.is_empty() {
                self.lines.len()
            } else {
                match self.get_selected_item() {
                    SelectedItem::Daily { index, .. } => match position {
                        InsertPosition::Below => self.entry_indices[index] + 1,
                        InsertPosition::Above => self.entry_indices[index],
                        InsertPosition::Bottom => unreachable!(),
                    },
                    _ => self.lines.len(),
                }
            };

        self.lines.insert(insert_pos, Line::Entry(entry));
        self.entry_indices = Self::compute_entry_indices(&self.lines);

        let entry_index = self
            .entry_indices
            .iter()
            .position(|&idx| idx == insert_pos)
            .unwrap_or(self.entry_indices.len().saturating_sub(1));

        let visible_index = self.actual_to_visible_index(entry_index);
        if let ViewMode::Daily(state) = &mut self.view {
            state.selected = visible_index;
        }

        self.original_edit_content = Some(String::new());
        self.edit_buffer = Some(CursorBuffer::empty());
        self.input_mode = InputMode::Edit(EditContext::Daily { entry_index });
    }

    pub fn new_task(&mut self, position: InsertPosition) {
        self.add_entry_internal(RawEntry::new_task(""), position);
    }
}
