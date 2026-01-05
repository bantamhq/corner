use std::io;

use crate::cursor::CursorBuffer;
use crate::storage::{self, Entry, EntryType};

use super::{App, EditContext, InputMode, Line, SelectedItem, ViewMode};

/// Target for delete operations (owns data extracted from SelectedItem)
pub enum DeleteTarget {
    Later {
        source_date: chrono::NaiveDate,
        line_index: usize,
        entry_type: EntryType,
        content: String,
    },
    Daily {
        line_idx: usize,
        entry: Entry,
    },
    Filter {
        index: usize,
        source_date: chrono::NaiveDate,
        line_index: usize,
        entry_type: EntryType,
        content: String,
    },
}

/// Location of an entry for operations that just need to find it (toggle, tag removal).
/// Shared between ToggleTarget and TagRemovalTarget since they have identical structure.
#[derive(Clone)]
pub enum EntryLocation {
    Later {
        source_date: chrono::NaiveDate,
        line_index: usize,
    },
    Daily {
        line_idx: usize,
    },
    Filter {
        index: usize,
        source_date: chrono::NaiveDate,
        line_index: usize,
    },
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
            SelectedItem::Later { entry, .. } => Some(DeleteTarget::Later {
                source_date: entry.source_date,
                line_index: entry.line_index,
                entry_type: entry.entry_type.clone(),
                content: entry.content.clone(),
            }),
            SelectedItem::Daily {
                line_idx, entry, ..
            } => Some(DeleteTarget::Daily {
                line_idx,
                entry: entry.clone(),
            }),
            SelectedItem::Filter { index, entry } => Some(DeleteTarget::Filter {
                index,
                source_date: entry.source_date,
                line_index: entry.line_index,
                entry_type: entry.entry_type.clone(),
                content: entry.content.clone(),
            }),
            SelectedItem::None => None,
        }
    }

    /// Delete the currently selected entry (view-aware)
    pub fn delete_current_entry(&mut self) -> io::Result<()> {
        let Some(target) = self.extract_delete_target_from_current() else {
            return Ok(());
        };
        let action = super::actions::DeleteEntries::single(target);
        self.execute_action(Box::new(action))
    }

    /// Extract toggle target from current selection (only for tasks)
    pub fn extract_toggle_target_from_current(&self) -> Option<ToggleTarget> {
        match self.get_selected_item() {
            SelectedItem::Later { entry, .. } => {
                if matches!(entry.entry_type, EntryType::Task { .. }) {
                    Some(ToggleTarget::Later {
                        source_date: entry.source_date,
                        line_index: entry.line_index,
                    })
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
                        source_date: entry.source_date,
                        line_index: entry.line_index,
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
            ToggleTarget::Later {
                source_date,
                line_index,
            } => {
                storage::toggle_entry_complete(source_date, &path, line_index)?;
                if let ViewMode::Daily(state) = &mut self.view {
                    state.later_entries =
                        storage::collect_later_entries_for_date(self.current_date, &path)?;
                }
            }
            ToggleTarget::Daily { line_idx } => {
                if let Line::Entry(entry) = &mut self.lines[line_idx] {
                    entry.toggle_complete();
                    self.save();
                }
            }
            ToggleTarget::Filter {
                index,
                source_date,
                line_index,
            } => {
                storage::toggle_entry_complete(source_date, &path, line_index)?;

                if let ViewMode::Filter(state) = &mut self.view {
                    let filter_entry = &mut state.entries[index];
                    filter_entry.completed = !filter_entry.completed;
                    if let EntryType::Task { completed } = &mut filter_entry.entry_type {
                        *completed = filter_entry.completed;
                    }
                }

                if source_date == self.current_date {
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
    pub fn edit_current_entry(&mut self) {
        let (ctx, content) = match self.get_selected_item() {
            SelectedItem::Later { index, entry } => (
                EditContext::LaterEdit {
                    source_date: entry.source_date,
                    line_index: entry.line_index,
                    later_index: index,
                },
                entry.content.clone(),
            ),
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
    }

    /// Extract yank target from current selection
    pub fn extract_yank_target_from_current(&self) -> Option<YankTarget> {
        let content = match self.get_selected_item() {
            SelectedItem::Later { entry, .. } => entry.content.clone(),
            SelectedItem::Daily { entry, .. } => entry.content.clone(),
            SelectedItem::Filter { entry, .. } => entry.content.clone(),
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
            SelectedItem::Later { entry, .. } => Some(TagRemovalTarget::Later {
                source_date: entry.source_date,
                line_index: entry.line_index,
            }),
            SelectedItem::Daily { line_idx, .. } => Some(TagRemovalTarget::Daily { line_idx }),
            SelectedItem::Filter { index, entry } => Some(TagRemovalTarget::Filter {
                index,
                source_date: entry.source_date,
                line_index: entry.line_index,
            }),
            SelectedItem::None => None,
        }
    }

    /// Extract cycle target (location + entry type) from current selection
    pub fn extract_cycle_target_from_current(&self) -> Option<super::actions::CycleTarget> {
        match self.get_selected_item() {
            SelectedItem::Later { entry, .. } => Some(super::actions::CycleTarget {
                location: EntryLocation::Later {
                    source_date: entry.source_date,
                    line_index: entry.line_index,
                },
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
                    source_date: entry.source_date,
                    line_index: entry.line_index,
                },
                original_type: entry.entry_type.clone(),
            }),
            SelectedItem::None => None,
        }
    }

    /// Extract tag target (location + content) from current selection
    pub fn extract_tag_target_from_current(&self) -> Option<super::actions::TagTarget> {
        match self.get_selected_item() {
            SelectedItem::Later { entry, .. } => Some(super::actions::TagTarget {
                location: EntryLocation::Later {
                    source_date: entry.source_date,
                    line_index: entry.line_index,
                },
                original_content: entry.content.clone(),
            }),
            SelectedItem::Daily {
                line_idx, entry, ..
            } => Some(super::actions::TagTarget {
                location: EntryLocation::Daily { line_idx },
                original_content: entry.content.clone(),
            }),
            SelectedItem::Filter { index, entry } => Some(super::actions::TagTarget {
                location: EntryLocation::Filter {
                    index,
                    source_date: entry.source_date,
                    line_index: entry.line_index,
                },
                original_content: entry.content.clone(),
            }),
            SelectedItem::None => None,
        }
    }

    /// Remove the last trailing tag from the selected entry
    pub fn remove_last_tag_from_current_entry(&mut self) -> io::Result<()> {
        let Some(target) = self.extract_tag_target_from_current() else {
            return Ok(());
        };
        let action =
            super::actions::RemoveLastTag::single(target.location, target.original_content);
        self.execute_action(Box::new(action))
    }

    /// Remove all trailing tags from the selected entry
    pub fn remove_all_tags_from_current_entry(&mut self) -> io::Result<()> {
        let Some(target) = self.extract_tag_target_from_current() else {
            return Ok(());
        };
        let action =
            super::actions::RemoveAllTags::single(target.location, target.original_content);
        self.execute_action(Box::new(action))
    }

    /// Append a favorite tag to the current entry
    pub fn append_tag_to_current_entry(&mut self, tag: &str) -> io::Result<()> {
        let Some(target) = self.extract_tag_target_from_current() else {
            return Ok(());
        };
        let action = super::actions::AppendTag::single(
            target.location,
            target.original_content,
            tag.to_string(),
        );
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

    fn copy_to_clipboard(text: &str) -> Result<(), arboard::Error> {
        let mut clipboard = arboard::Clipboard::new()?;
        clipboard.set_text(text)?;
        Ok(())
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
