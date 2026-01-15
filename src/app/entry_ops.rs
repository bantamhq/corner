use std::io;

use crate::cursor::CursorBuffer;
use crate::storage::{
    self, Entry, EntryType, RawEntry, SourceType, parse_to_raw_entry, strip_recurring_tags,
};

use super::{App, EditContext, InputMode, Line, SelectedItem, ViewMode};

/// Target for delete operations - Entry carries all location info
pub enum DeleteTarget {
    /// Projected entry (Later/Recurring) - Entry has source_date, line_index, source_type
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
    /// Projected entry (Later/Recurring) - Entry has source_date, line_index, source_type
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

    /// Delete the currently selected entry (view-aware, yanks to clipboard first)
    /// Later entries are deletable (deletes source), Recurring entries are read-only.
    pub fn delete_current_entry(&mut self) -> io::Result<()> {
        if let SelectedItem::Projected { entry, .. } = self.get_selected_item()
            && !matches!(entry.source_type, SourceType::Later)
        {
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

    /// Extract toggle target from current selection (only for tasks)
    pub fn extract_toggle_target_from_current(&self) -> Option<ToggleTarget> {
        match self.get_selected_item() {
            SelectedItem::Projected { entry, .. } => {
                if matches!(entry.entry_type, EntryType::Task { .. }) {
                    Some(ToggleTarget::Projected(entry.clone()))
                } else {
                    None
                }
            }
            SelectedItem::Daily {
                line_idx, entry, ..
            } => {
                if matches!(entry.entry_type, EntryType::Task { .. }) {
                    Some(ToggleTarget::Daily { line_idx })
                } else {
                    None
                }
            }
            SelectedItem::Filter { index, entry } => {
                if matches!(entry.entry_type, EntryType::Task { .. }) {
                    Some(ToggleTarget::Filter {
                        index,
                        entry: entry.clone(),
                    })
                } else {
                    None
                }
            }
            SelectedItem::None => None,
        }
    }

    /// Execute toggle on a target
    pub fn execute_toggle(&mut self, target: ToggleTarget) -> io::Result<()> {
        let path = self.active_path().to_path_buf();
        match target {
            ToggleTarget::Projected(entry) => {
                match entry.source_type {
                    SourceType::Later => {
                        storage::toggle_entry_complete(entry.source_date, &path, entry.line_index)?;
                    }
                    SourceType::Recurring => {
                        // Materialize: create completed copy on today instead of toggling source
                        // Add ↺ prefix for matching when hiding done-today recurring entries
                        let content =
                            storage::get_entry_content(entry.source_date, &path, entry.line_index)
                                .map(|c| format!("↺ {}", strip_recurring_tags(&c)))
                                .unwrap_or_default();

                        let raw_entry = RawEntry {
                            entry_type: EntryType::Task { completed: true },
                            content,
                        };
                        self.lines.push(Line::Entry(raw_entry));
                        self.entry_indices = Self::compute_entry_indices(&self.lines);
                        self.save();
                    }
                    SourceType::Local => unreachable!("projected entries are never Local"),
                    SourceType::Calendar { .. } => {
                        unreachable!("calendar entries are never toggled")
                    }
                }
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

    /// Toggle task completion (view-aware)
    pub fn toggle_current_entry(&mut self) -> io::Result<()> {
        let Some(target) = self.extract_toggle_target_from_current() else {
            return Ok(());
        };
        self.execute_toggle(target)
    }

    /// Start editing the current entry (view-aware)
    /// Later entries are editable (edits source), Recurring entries are read-only.
    pub fn edit_current_entry(&mut self) {
        let (ctx, content) = match self.get_selected_item() {
            SelectedItem::Projected { entry, .. } => {
                if !matches!(entry.source_type, SourceType::Later) {
                    self.set_status("Press o to go to source");
                    return;
                }
                (
                    EditContext::LaterEdit {
                        source_date: entry.source_date,
                        line_index: entry.line_index,
                    },
                    entry.content.clone(),
                )
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

        self.original_edit_content = Some(content.clone());
        self.edit_buffer = Some(CursorBuffer::new(content));
        self.input_mode = InputMode::Edit(ctx);
        self.update_hints();
    }

    /// Extract yank target from current selection (includes prefix for round-trip paste)
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

    /// Get content from a yank target
    pub fn yank_target_content(target: &YankTarget) -> &str {
        &target.content
    }

    /// Execute yank - copy content to clipboard
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

    /// Extract tag removal target from current selection
    pub fn extract_tag_removal_target_from_current(&self) -> Option<TagRemovalTarget> {
        match self.get_selected_item() {
            SelectedItem::Projected { entry, .. } => {
                Some(TagRemovalTarget::Projected(entry.clone()))
            }
            SelectedItem::Daily { line_idx, .. } => Some(TagRemovalTarget::Daily { line_idx }),
            SelectedItem::Filter { index, entry } => Some(TagRemovalTarget::Filter {
                index,
                entry: entry.clone(),
            }),
            SelectedItem::None => None,
        }
    }

    /// Extract cycle target (location + entry type) from current selection
    pub fn extract_cycle_target_from_current(&self) -> Option<super::actions::CycleTarget> {
        match self.get_selected_item() {
            SelectedItem::Projected { entry, .. } => Some(super::actions::CycleTarget {
                location: EntryLocation::Projected(entry.clone()),
                original_type: entry.entry_type.clone(),
            }),
            SelectedItem::Daily {
                line_idx, entry, ..
            } => Some(super::actions::CycleTarget {
                location: EntryLocation::Daily { line_idx },
                original_type: entry.entry_type.clone(),
            }),
            SelectedItem::Filter { index, entry } => Some(super::actions::CycleTarget {
                location: EntryLocation::Filter {
                    index,
                    entry: entry.clone(),
                },
                original_type: entry.entry_type.clone(),
            }),
            SelectedItem::None => None,
        }
    }

    /// Extract content target (location + content) from current selection.
    /// Used for tag operations, date operations, and other content transformations.
    pub fn extract_content_target_from_current(&self) -> Option<super::actions::ContentTarget> {
        match self.get_selected_item() {
            SelectedItem::Projected { entry, .. } => Some(super::actions::ContentTarget::new(
                EntryLocation::Projected(entry.clone()),
                entry.content.clone(),
            )),
            SelectedItem::Daily {
                line_idx, entry, ..
            } => Some(super::actions::ContentTarget::new(
                EntryLocation::Daily { line_idx },
                entry.content.clone(),
            )),
            SelectedItem::Filter { index, entry } => Some(super::actions::ContentTarget::new(
                EntryLocation::Filter {
                    index,
                    entry: entry.clone(),
                },
                entry.content.clone(),
            )),
            SelectedItem::None => None,
        }
    }

    /// Remove the last trailing tag from the selected entry
    pub fn remove_last_tag_from_current_entry(&mut self) -> io::Result<()> {
        let Some(target) = self.extract_content_target_from_current() else {
            return Ok(());
        };
        let action =
            super::actions::RemoveLastTag::single(target.location, target.original_content);
        self.execute_action(Box::new(action))
    }

    /// Remove all trailing tags from the selected entry
    pub fn remove_all_tags_from_current_entry(&mut self) -> io::Result<()> {
        let Some(target) = self.extract_content_target_from_current() else {
            return Ok(());
        };
        let action =
            super::actions::RemoveAllTags::single(target.location, target.original_content);
        self.execute_action(Box::new(action))
    }

    /// Append a favorite tag to the current entry
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

    /// Defer the @date on the current entry by 1 day
    pub fn defer_current_entry(&mut self) -> io::Result<()> {
        let Some(target) = self.extract_content_target_from_current() else {
            return Ok(());
        };

        if super::actions::is_recurring_entry(&target.location) {
            self.set_error("Cannot defer recurring entries");
            return Ok(());
        }

        let action = super::actions::DeferDate::single(target.location, target.original_content);
        self.execute_action(Box::new(action))
    }

    /// Remove the @date from the current entry
    pub fn remove_date_from_current_entry(&mut self) -> io::Result<()> {
        let Some(target) = self.extract_content_target_from_current() else {
            return Ok(());
        };
        let action = super::actions::RemoveDate::single(target.location, target.original_content);
        self.execute_action(Box::new(action))
    }

    /// Cycle entry type on the current entry
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

    /// Paste clipboard content as entries below current selection
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

    /// Paste text as entries below current selection (used by bracketed paste)
    pub fn paste_entries_from_text(&mut self, text: &str) -> io::Result<()> {
        let raw_entries = Self::parse_paste_raw(text);
        if raw_entries.is_empty() {
            self.set_error("Nothing to paste");
            return Ok(());
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

        if date == self.current_date {
            self.reload_current_day()?;
        }
        if let ViewMode::Filter(_) = &self.view {
            let _ = self.refresh_filter();
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
}
