use std::io;

use chrono::{Days, NaiveDate};

use crate::cursor::CursorBuffer;
use crate::storage::{
    self, Entry, EntryType, RawEntry, SourceType, add_done_date, is_done_on_date,
    parse_to_raw_entry, remove_done_date, strip_done_meta,
};

use super::{App, EditContext, InputMode, Line, SelectedItem, ViewMode};

/// Target for delete operations - Entry carries all location info
pub enum DeleteTarget {
    /// Projected entry (Recurring) - Entry has source_date, line_index, source_type
    Projected(Entry),
    /// Local entry in daily view - line_idx is index in self.lines
    Daily { line_idx: usize, entry: Entry },
    /// Entry in filter view - index is position in filter results
    Filter { index: usize, entry: Entry },
}

/// Location of an entry for operations that just need to find it (toggle, tag removal).
/// Shared between ToggleTarget and TagRemovalTarget since they have identical structure.
#[derive(Clone)]
pub enum EntryLocation {
    /// Projected entry (Recurring) - Entry has source_date, line_index, source_type
    Projected(Entry),
    /// Local entry in daily view - line_idx is index in self.lines
    Daily { line_idx: usize },
    /// Entry in filter view - index is position in filter results
    Filter { index: usize, entry: Entry },
}

/// Target for toggle operations (type alias for semantic clarity)
pub type ToggleTarget = EntryLocation;

/// Target for tag removal operations (type alias for semantic clarity)
pub type TagRemovalTarget = EntryLocation;

/// Target for yank operations - just needs content from any source
pub struct YankTarget {
    pub content: String,
}

impl App {
    fn clamp_daily_selection(&mut self) {
        let visible = self.visible_entry_count();
        if let ViewMode::Daily(state) = &mut self.view
            && visible > 0
            && state.selected >= visible
        {
            state.selected = visible - 1;
        }
    }

    pub(super) fn add_entries_to_date(
        &mut self,
        entries: Vec<RawEntry>,
        target_date: NaiveDate,
    ) -> io::Result<()> {
        let path = self.active_path().to_path_buf();
        let mut target_lines = storage::load_day_lines(target_date, &path)?;
        for entry in entries {
            target_lines.push(Line::Entry(entry));
        }
        storage::save_day_lines(target_date, &path, &target_lines)?;
        self.refresh_affected_views(target_date)
    }

    fn extract_location_from_current(&self) -> Option<EntryLocation> {
        match self.get_selected_item() {
            SelectedItem::Projected { entry, .. } => Some(EntryLocation::Projected(entry.clone())),
            SelectedItem::Daily { line_idx, .. } => Some(EntryLocation::Daily { line_idx }),
            SelectedItem::Filter { index, entry } => Some(EntryLocation::Filter {
                index,
                entry: entry.clone(),
            }),
            SelectedItem::None => None,
        }
    }

    /// Extract delete target from current selection
    pub fn extract_delete_target_from_current(&self) -> Option<DeleteTarget> {
        match self.get_selected_item() {
            SelectedItem::Projected { entry, .. } => Some(DeleteTarget::Projected(entry.clone())),
            SelectedItem::Daily {
                line_idx,
                entry: raw_entry,
                ..
            } => Some(DeleteTarget::Daily {
                line_idx,
                entry: Entry::from_raw(raw_entry, self.current_date, line_idx, SourceType::Local),
            }),
            SelectedItem::Filter { index, entry } => Some(DeleteTarget::Filter {
                index,
                entry: entry.clone(),
            }),
            SelectedItem::None => None,
        }
    }

    pub fn delete_current_entry(&mut self) -> io::Result<()> {
        if let SelectedItem::Projected { .. } = self.get_selected_item() {
            self.set_status("Press o to go to source");
            return Ok(());
        }

        // Yank before deleting (like Vim)
        if let Some(yank_target) = self.extract_yank_target_from_current() {
            let _ = Self::copy_to_clipboard(Self::yank_target_content(&yank_target));
        }

        let Some(target) = self.extract_delete_target_from_current() else {
            return Ok(());
        };
        let action = super::actions::DeleteEntries::single(target);
        self.execute_action(Box::new(action))
    }

    fn current_is_task(&self) -> bool {
        let entry_type = match self.get_selected_item() {
            SelectedItem::Projected { entry, .. } => Some(&entry.entry_type),
            SelectedItem::Daily { entry, .. } => Some(&entry.entry_type),
            SelectedItem::Filter { entry, .. } => Some(&entry.entry_type),
            SelectedItem::None => None,
        };
        entry_type.is_some_and(|t| matches!(t, EntryType::Task { .. }))
    }

    pub fn extract_toggle_target_from_current(&self) -> Option<ToggleTarget> {
        if self.current_is_task() {
            self.extract_location_from_current()
        } else {
            None
        }
    }

    pub fn execute_toggle(&mut self, target: ToggleTarget) -> io::Result<()> {
        let path = self.active_path().to_path_buf();
        match target {
            ToggleTarget::Projected(entry) => {
                let Some(content) =
                    storage::get_entry_content(entry.source_date, &path, entry.line_index)
                else {
                    return Ok(());
                };

                let new_content = if is_done_on_date(&content, self.current_date) {
                    remove_done_date(&content, self.current_date)
                } else {
                    add_done_date(&content, self.current_date)
                };

                storage::update_entry_content(
                    entry.source_date,
                    &path,
                    entry.line_index,
                    new_content,
                )?;
                self.refresh_projected_entries();
            }
            ToggleTarget::Daily { line_idx } => {
                if let Line::Entry(raw_entry) = &mut self.lines[line_idx] {
                    raw_entry.toggle_complete();
                    self.save();
                }
            }
            ToggleTarget::Filter { index, entry } => {
                storage::toggle_entry_complete(entry.source_date, &path, entry.line_index)?;

                if let ViewMode::Filter(state) = &mut self.view {
                    let filter_entry = &mut state.entries[index];
                    if let EntryType::Task { completed } = &mut filter_entry.entry_type {
                        *completed = !*completed;
                    }
                }

                if entry.source_date == self.current_date {
                    self.reload_current_day()?;
                }
            }
        }
        Ok(())
    }

    pub fn toggle_current_entry(&mut self) -> io::Result<()> {
        let Some(target) = self.extract_toggle_target_from_current() else {
            return Ok(());
        };
        self.execute_toggle(target)
    }

    pub fn edit_current_entry(&mut self) {
        let (ctx, content) = match self.get_selected_item() {
            SelectedItem::Projected { .. } => {
                self.set_status("Press o to go to source");
                return;
            }
            SelectedItem::Daily { index, entry, .. } => (
                EditContext::Daily { entry_index: index },
                entry.content.clone(),
            ),
            SelectedItem::Filter { index, entry } => (
                EditContext::FilterEdit {
                    date: entry.source_date,
                    line_index: entry.line_index,
                    filter_index: index,
                },
                entry.content.clone(),
            ),
            SelectedItem::None => return,
        };

        // Keep original with metadata for restoration on save
        self.original_edit_content = Some(content.clone());
        // Strip metadata for display in edit buffer
        let display_content = strip_done_meta(&content);
        self.edit_buffer = Some(CursorBuffer::new(display_content));
        self.input_mode = InputMode::Edit(ctx);
        self.update_hints();
    }

    pub fn extract_yank_target_from_current(&self) -> Option<YankTarget> {
        let content = match self.get_selected_item() {
            SelectedItem::Projected { entry, .. } => {
                format!("{}{}", entry.entry_type.prefix(), entry.content)
            }
            SelectedItem::Daily { entry, .. } => {
                format!("{}{}", entry.prefix(), entry.content)
            }
            SelectedItem::Filter { entry, .. } => {
                format!("{}{}", entry.entry_type.prefix(), entry.content)
            }
            SelectedItem::None => return None,
        };
        Some(YankTarget { content })
    }

    pub fn yank_target_content(target: &YankTarget) -> &str {
        &target.content
    }

    pub fn execute_yank(&mut self, contents: &[&str]) {
        if contents.is_empty() {
            return;
        }
        let combined = contents.join("\n");
        match Self::copy_to_clipboard(&combined) {
            Ok(()) => {
                if contents.len() == 1 {
                    self.set_status("Yanked");
                } else {
                    self.set_status(format!("Yanked {} entries", contents.len()));
                }
            }
            Err(e) => self.set_status(format!("Failed to yank: {e}")),
        }
    }

    pub fn yank_current_entry(&mut self) {
        let Some(target) = self.extract_yank_target_from_current() else {
            return;
        };
        let content = Self::yank_target_content(&target);
        self.execute_yank(&[content]);
    }

    pub fn extract_cycle_target_from_current(&self) -> Option<super::actions::CycleTarget> {
        let location = self.extract_location_from_current()?;
        let original_type = match self.get_selected_item() {
            SelectedItem::Projected { entry, .. } => entry.entry_type.clone(),
            SelectedItem::Daily { entry, .. } => entry.entry_type.clone(),
            SelectedItem::Filter { entry, .. } => entry.entry_type.clone(),
            SelectedItem::None => return None,
        };
        Some(super::actions::CycleTarget {
            location,
            original_type,
        })
    }

    pub fn extract_content_target_from_current(&self) -> Option<super::actions::ContentTarget> {
        let location = self.extract_location_from_current()?;
        let content = match self.get_selected_item() {
            SelectedItem::Projected { entry, .. } => entry.content.clone(),
            SelectedItem::Daily { entry, .. } => entry.content.clone(),
            SelectedItem::Filter { entry, .. } => entry.content.clone(),
            SelectedItem::None => return None,
        };
        Some(super::actions::ContentTarget::new(location, content))
    }

    pub fn remove_last_tag_from_current_entry(&mut self) -> io::Result<()> {
        let Some(target) = self.extract_content_target_from_current() else {
            return Ok(());
        };
        let action =
            super::actions::RemoveLastTag::single(target.location, target.original_content);
        self.execute_action(Box::new(action))
    }

    pub fn remove_all_tags_from_current_entry(&mut self) -> io::Result<()> {
        let Some(target) = self.extract_content_target_from_current() else {
            return Ok(());
        };
        let action =
            super::actions::RemoveAllTags::single(target.location, target.original_content);
        self.execute_action(Box::new(action))
    }

    pub fn append_tag_to_current_entry(&mut self, tag: &str) -> io::Result<()> {
        let Some(target) = self.extract_content_target_from_current() else {
            return Ok(());
        };
        let action = super::actions::AppendTag::single(
            target.location,
            target.original_content,
            tag.to_string(),
        );
        self.execute_action(Box::new(action))
    }

    pub fn cycle_current_entry_type(&mut self) -> io::Result<()> {
        let Some(target) = self.extract_cycle_target_from_current() else {
            return Ok(());
        };
        let action = super::actions::CycleEntryType::single(target.location, target.original_type);
        self.execute_action(Box::new(action))
    }

    fn is_test_environment() -> bool {
        // cfg(test) only works for unit tests in this crate.
        // For integration tests, we check CALIBER_SKIP_CLIPBOARD which TestContext sets.
        cfg!(test) || std::env::var("CALIBER_SKIP_CLIPBOARD").is_ok()
    }

    pub(super) fn copy_to_clipboard(text: &str) -> Result<(), arboard::Error> {
        if Self::is_test_environment() {
            return Ok(());
        }
        let mut clipboard = arboard::Clipboard::new()?;
        clipboard.set_text(text)?;
        Ok(())
    }

    fn read_from_clipboard() -> Result<String, arboard::Error> {
        if Self::is_test_environment() {
            return Ok(String::new());
        }
        let mut clipboard = arboard::Clipboard::new()?;
        clipboard.get_text()
    }

    pub fn paste_from_clipboard(&mut self) -> io::Result<()> {
        let text = match Self::read_from_clipboard() {
            Ok(t) => t,
            Err(e) => {
                self.set_error(format!("Failed to read clipboard: {e}"));
                return Ok(());
            }
        };
        self.paste_entries_from_text(&text)
    }

    pub fn paste_entries_from_text(&mut self, text: &str) -> io::Result<()> {
        let mut raw_entries = Self::parse_paste_raw(text);
        if raw_entries.is_empty() {
            self.set_error("Nothing to paste");
            return Ok(());
        }

        for entry in &mut raw_entries {
            let (normalized, warning) = self.normalize_content(&entry.content);
            entry.content = normalized;
            if let Some(warn) = warning {
                self.set_status(warn);
            }
        }

        let (date, insert_after) = match self.get_selected_item() {
            SelectedItem::Daily { line_idx, .. } => (self.current_date, line_idx),
            SelectedItem::Projected { entry, .. } => (entry.source_date, entry.line_index),
            SelectedItem::Filter { entry, .. } => (entry.source_date, entry.line_index),
            SelectedItem::None => (self.current_date, 0),
        };

        let path = self.active_path().to_path_buf();
        let mut lines = storage::load_day_lines(date, &path)?;
        let insert_pos = if lines.is_empty() {
            0
        } else {
            insert_after + 1
        };

        // Insert raw entries into lines and build full entries for action
        let entries: Vec<Entry> = raw_entries
            .iter()
            .enumerate()
            .map(|(i, raw)| {
                let line_idx = insert_pos + i;
                lines.insert(line_idx, Line::Entry(raw.clone()));
                Entry::from_raw(raw, date, line_idx, SourceType::Local)
            })
            .collect();

        storage::save_day_lines(date, &path, &lines)?;
        let pasted_count = entries.len();
        self.refresh_affected_views(date)?;

        // Move cursor to the last pasted entry
        if date == self.current_date {
            let last_pasted_line = insert_pos + pasted_count - 1;
            if let Some(actual_idx) = self
                .entry_indices
                .iter()
                .position(|&idx| idx == last_pasted_line)
            {
                let visible_idx = self.actual_to_visible_index(actual_idx);
                if let ViewMode::Daily(state) = &mut self.view {
                    state.selected = visible_idx;
                }
            }
        }

        let target = super::actions::PasteTarget {
            date,
            start_line_index: insert_pos,
            entries,
        };
        let action = super::actions::PasteEntries::new(target);
        self.execute_action(Box::new(action))
    }

    fn parse_paste_raw(text: &str) -> Vec<RawEntry> {
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .map(parse_to_raw_entry)
            .collect()
    }

    pub(super) fn delete_at_index_daily(&mut self, entry_index: usize) {
        if entry_index >= self.entry_indices.len() {
            return;
        }
        let line_idx = self.entry_indices[entry_index];
        self.lines.remove(line_idx);
        self.entry_indices = Self::compute_entry_indices(&self.lines);
        self.clamp_daily_selection();
    }

    fn move_current_entry_to_date(&mut self, target_date: NaiveDate) -> io::Result<()> {
        if let SelectedItem::Projected { .. } = self.get_selected_item() {
            self.set_status("Press o to go to source");
            return Ok(());
        }

        let (source_date, raw_entry) = match self.get_selected_item() {
            SelectedItem::Daily { entry, .. } => (self.current_date, entry.clone()),
            SelectedItem::Filter { entry, .. } => {
                let content = storage::get_entry_content(
                    entry.source_date,
                    self.active_path(),
                    entry.line_index,
                )
                .unwrap_or_default();
                (
                    entry.source_date,
                    RawEntry {
                        entry_type: entry.entry_type.clone(),
                        content,
                    },
                )
            }
            SelectedItem::Projected { .. } | SelectedItem::None => return Ok(()),
        };

        if source_date == target_date {
            self.set_status("Entry already on target date");
            return Ok(());
        }

        let Some(delete_target) = self.extract_delete_target_from_current() else {
            return Ok(());
        };
        let delete_action = super::actions::DeleteEntries::single(delete_target);
        self.execute_action(Box::new(delete_action))?;

        self.add_entries_to_date(vec![raw_entry], target_date)?;
        self.set_status(format!("Moved to {}", target_date.format("%m/%d")));
        Ok(())
    }

    pub fn move_current_entry_to_today(&mut self) -> io::Result<()> {
        let today = chrono::Local::now().date_naive();
        self.move_current_entry_to_date(today)
    }

    pub fn defer_current_entry(&mut self) -> io::Result<()> {
        let tomorrow = chrono::Local::now()
            .date_naive()
            .checked_add_days(Days::new(1))
            .expect("tomorrow should be valid");
        self.move_current_entry_to_date(tomorrow)
    }
}
