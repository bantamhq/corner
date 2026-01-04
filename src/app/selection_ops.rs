use std::io;

use crate::storage::{Entry, EntryType, FilterEntry, LaterEntry};
use crate::ui::{remove_all_trailing_tags, remove_last_trailing_tag};

use super::{
    App, DeleteTarget, InputMode, Line, SelectionState, TagRemovalTarget, ToggleTarget, ViewMode,
    YankTarget,
};

/// Represents an entry at a selected visible index, used for batch operations.
enum SelectedEntry<'a> {
    Later(&'a LaterEntry),
    Daily {
        line_idx: usize,
        entry: &'a Entry,
    },
    Filter {
        index: usize,
        entry: &'a FilterEntry,
    },
}

impl App {
    /// Enter selection mode at current cursor position
    pub fn enter_selection_mode(&mut self) {
        let current = self.current_visible_index();
        if current < self.visible_entry_count() {
            self.input_mode = InputMode::Selection(SelectionState::new(current));
        }
    }

    /// Exit selection mode, returning to Normal
    pub fn exit_selection_mode(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    /// Move cursor down in selection mode (without extending)
    pub fn selection_move_down(&mut self) {
        self.move_down();
    }

    /// Move cursor up in selection mode (without extending)
    pub fn selection_move_up(&mut self) {
        self.move_up();
    }

    /// Select range from anchor to current cursor (Shift+V)
    pub fn selection_extend_to_cursor(&mut self) {
        let current = self.current_visible_index();
        if let InputMode::Selection(ref mut state) = self.input_mode {
            state.extend_to(current);
        }
    }

    /// Toggle selection on current entry (Space)
    pub fn selection_toggle_current(&mut self) {
        let current = self.current_visible_index();
        if let InputMode::Selection(ref mut state) = self.input_mode {
            state.toggle(current);
        }
    }

    /// Get current visible index based on view mode
    fn current_visible_index(&self) -> usize {
        match &self.view {
            ViewMode::Daily(state) => state.selected,
            ViewMode::Filter(state) => state.selected,
        }
    }

    /// Get later entry at visible index
    fn get_later_at_visible_index(&self, visible_idx: usize) -> Option<&LaterEntry> {
        let ViewMode::Daily(state) = &self.view else {
            return None;
        };

        let mut current_visible = 0;
        for later in &state.later_entries {
            if !self.should_show_later(later) {
                continue;
            }
            if current_visible == visible_idx {
                return Some(later);
            }
            current_visible += 1;
        }
        None
    }

    /// Get daily entry at visible entry index (after later entries)
    fn get_daily_at_visible_entry_index(
        &self,
        visible_entry_idx: usize,
    ) -> Option<(usize, &Entry)> {
        let mut current_visible = 0;
        for &line_idx in &self.entry_indices {
            if let Line::Entry(entry) = &self.lines[line_idx] {
                if !self.should_show_entry(entry) {
                    continue;
                }
                if current_visible == visible_entry_idx {
                    return Some((line_idx, entry));
                }
                current_visible += 1;
            }
        }
        None
    }

    /// Get entry at a visible index (unified lookup for selection operations)
    fn get_entry_at_visible_index(&self, visible_idx: usize) -> Option<SelectedEntry<'_>> {
        match &self.view {
            ViewMode::Daily(_) => {
                let later_count = self.visible_later_count();
                if visible_idx < later_count {
                    self.get_later_at_visible_index(visible_idx)
                        .map(SelectedEntry::Later)
                } else {
                    let entry_idx = visible_idx - later_count;
                    self.get_daily_at_visible_entry_index(entry_idx)
                        .map(|(line_idx, entry)| SelectedEntry::Daily { line_idx, entry })
                }
            }
            ViewMode::Filter(state) => {
                state
                    .entries
                    .get(visible_idx)
                    .map(|entry| SelectedEntry::Filter {
                        index: visible_idx,
                        entry,
                    })
            }
        }
    }

    /// Collect targets from selected entries using a mapper function
    fn collect_targets_from_selected<T, F>(&self, mapper: F) -> Vec<T>
    where
        F: Fn(SelectedEntry<'_>) -> Option<T>,
    {
        let InputMode::Selection(ref state) = self.input_mode else {
            return vec![];
        };

        state
            .selected_indices
            .iter()
            .filter_map(|&idx| self.get_entry_at_visible_index(idx).and_then(&mapper))
            .collect()
    }

    /// Collect delete targets for all selected entries
    fn collect_delete_targets_from_selected(&self) -> Vec<DeleteTarget> {
        self.collect_targets_from_selected(|entry| match entry {
            SelectedEntry::Later(later) => Some(DeleteTarget::Later {
                source_date: later.source_date,
                line_index: later.line_index,
                entry_type: later.entry_type.clone(),
                content: later.content.clone(),
            }),
            SelectedEntry::Daily { line_idx, entry } => Some(DeleteTarget::Daily {
                line_idx,
                entry: entry.clone(),
            }),
            SelectedEntry::Filter { index, entry } => Some(DeleteTarget::Filter {
                index,
                source_date: entry.source_date,
                line_index: entry.line_index,
                entry_type: entry.entry_type.clone(),
                content: entry.content.clone(),
            }),
        })
    }

    /// Collect toggle targets for all selected entries (tasks only)
    fn collect_toggle_targets_from_selected(&self) -> Vec<ToggleTarget> {
        self.collect_targets_from_selected(|entry| {
            // Helper to check if entry type is a task
            fn is_task(entry_type: &EntryType) -> bool {
                matches!(entry_type, EntryType::Task { .. })
            }

            match entry {
                SelectedEntry::Later(later) if is_task(&later.entry_type) => {
                    Some(ToggleTarget::Later {
                        source_date: later.source_date,
                        line_index: later.line_index,
                    })
                }
                SelectedEntry::Daily { line_idx, entry } if is_task(&entry.entry_type) => {
                    Some(ToggleTarget::Daily { line_idx })
                }
                SelectedEntry::Filter { index, entry } if is_task(&entry.entry_type) => {
                    Some(ToggleTarget::Filter {
                        index,
                        source_date: entry.source_date,
                        line_index: entry.line_index,
                    })
                }
                _ => None,
            }
        })
    }

    /// Collect yank targets for all selected entries
    fn collect_yank_targets_from_selected(&self) -> Vec<YankTarget> {
        self.collect_targets_from_selected(|entry| {
            let content = match entry {
                SelectedEntry::Later(later) => later.content.clone(),
                SelectedEntry::Daily { entry, .. } => entry.content.clone(),
                SelectedEntry::Filter { entry, .. } => entry.content.clone(),
            };
            Some(YankTarget { content })
        })
    }

    /// Collect tag removal targets for all selected entries
    fn collect_tag_removal_targets_from_selected(&self) -> Vec<TagRemovalTarget> {
        self.collect_targets_from_selected(|entry| match entry {
            SelectedEntry::Later(later) => Some(TagRemovalTarget::Later {
                source_date: later.source_date,
                line_index: later.line_index,
            }),
            SelectedEntry::Daily { line_idx, .. } => Some(TagRemovalTarget::Daily { line_idx }),
            SelectedEntry::Filter { index, entry } => Some(TagRemovalTarget::Filter {
                index,
                source_date: entry.source_date,
                line_index: entry.line_index,
            }),
        })
    }

    /// Delete all selected entries
    pub fn delete_selected(&mut self) -> io::Result<()> {
        let mut targets = self.collect_delete_targets_from_selected();
        if targets.is_empty() {
            self.exit_selection_mode();
            return Ok(());
        }

        let count = targets.len();

        // Sort by line index descending to maintain valid indices during deletion
        targets.sort_by(|a, b| {
            let idx_a = match a {
                DeleteTarget::Daily { line_idx, .. } => *line_idx,
                DeleteTarget::Later { line_index, .. } => *line_index,
                DeleteTarget::Filter { line_index, .. } => *line_index,
            };
            let idx_b = match b {
                DeleteTarget::Daily { line_idx, .. } => *line_idx,
                DeleteTarget::Later { line_index, .. } => *line_index,
                DeleteTarget::Filter { line_index, .. } => *line_index,
            };
            idx_b.cmp(&idx_a)
        });

        for target in targets {
            self.execute_delete(target)?;
        }

        self.set_status(format!("Deleted {} entries", count));
        self.exit_selection_mode();
        Ok(())
    }

    /// Toggle all selected entries (tasks only)
    pub fn toggle_selected(&mut self) -> io::Result<()> {
        let targets = self.collect_toggle_targets_from_selected();
        if targets.is_empty() {
            return Ok(());
        }

        let count = targets.len();

        for target in targets {
            self.execute_toggle(target)?;
        }

        self.set_status(format!("Toggled {} entries", count));
        Ok(())
    }

    /// Yank all selected entries to clipboard
    pub fn yank_selected(&mut self) {
        let targets = self.collect_yank_targets_from_selected();
        if targets.is_empty() {
            return;
        }

        let contents: Vec<&str> = targets.iter().map(Self::yank_target_content).collect();
        self.execute_yank(&contents);
    }

    /// Remove last trailing tag from all selected entries
    pub fn remove_last_tag_from_selected(&mut self) -> io::Result<()> {
        let targets = self.collect_tag_removal_targets_from_selected();
        if targets.is_empty() {
            return Ok(());
        }

        let count = targets.len();

        for target in targets {
            self.execute_tag_removal(target, remove_last_trailing_tag)?;
        }

        self.set_status(format!("Removed tags from {} entries", count));
        Ok(())
    }

    /// Remove all trailing tags from all selected entries
    pub fn remove_all_tags_from_selected(&mut self) -> io::Result<()> {
        let targets = self.collect_tag_removal_targets_from_selected();
        if targets.is_empty() {
            return Ok(());
        }

        let count = targets.len();

        for target in targets {
            self.execute_tag_removal(target, remove_all_trailing_tags)?;
        }

        self.set_status(format!("Removed all tags from {} entries", count));
        Ok(())
    }

    /// Append a tag to all selected entries
    pub fn append_tag_to_selected(&mut self, tag: &str) -> io::Result<()> {
        let targets = self.collect_tag_removal_targets_from_selected();
        if targets.is_empty() {
            return Ok(());
        }

        let count = targets.len();

        for target in targets {
            self.execute_append_tag(target, tag)?;
        }

        self.set_status(format!("Added #{tag} to {} entries", count));
        Ok(())
    }

    /// Cycle entry type on all selected entries
    pub fn cycle_selected_entry_types(&mut self) -> io::Result<()> {
        let targets = self.collect_tag_removal_targets_from_selected();
        if targets.is_empty() {
            return Ok(());
        }

        let count = targets.len();

        for target in targets {
            self.execute_cycle_type(target)?;
        }

        self.set_status(format!("Cycled type on {} entries", count));
        Ok(())
    }

    /// Check if in selection mode and get selection state
    pub fn get_selection_state(&self) -> Option<&SelectionState> {
        if let InputMode::Selection(ref state) = self.input_mode {
            Some(state)
        } else {
            None
        }
    }
}
