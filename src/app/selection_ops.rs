use std::io;

use crate::storage::{Entry, EntryType, RawEntry, SourceType};

use super::{
    App, DeleteTarget, InputMode, Line, SelectionState, TagRemovalTarget, ToggleTarget, ViewMode,
    YankTarget,
};

/// Represents an entry at a selected visible index, used for batch operations.
enum SelectedEntry<'a> {
    Projected(&'a Entry),
    Daily {
        line_idx: usize,
        entry: &'a RawEntry,
    },
    Filter {
        index: usize,
        entry: &'a Entry,
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

    /// Get projected entry at visible index
    fn get_projected_at_visible_index(&self, visible_idx: usize) -> Option<&Entry> {
        let ViewMode::Daily(state) = &self.view else {
            return None;
        };

        let mut current_visible = 0;
        for projected in &state.projected_entries {
            if !self.should_show_entry(projected) {
                continue;
            }
            if current_visible == visible_idx {
                return Some(projected);
            }
            current_visible += 1;
        }
        None
    }

    /// Get daily entry at visible entry index (after later entries)
    fn get_daily_at_visible_entry_index(
        &self,
        visible_entry_idx: usize,
    ) -> Option<(usize, &RawEntry)> {
        let mut current_visible = 0;
        for &line_idx in &self.entry_indices {
            if let Line::Entry(raw_entry) = &self.lines[line_idx] {
                if !self.should_show_raw_entry(raw_entry) {
                    continue;
                }
                if current_visible == visible_entry_idx {
                    return Some((line_idx, raw_entry));
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
                let projected_count = self.visible_projected_count();
                if visible_idx < projected_count {
                    self.get_projected_at_visible_index(visible_idx)
                        .map(SelectedEntry::Projected)
                } else {
                    let entry_idx = visible_idx - projected_count;
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
        let current_date = self.current_date;
        self.collect_targets_from_selected(|entry| match entry {
            SelectedEntry::Projected(projected) => Some(DeleteTarget::Projected(projected.clone())),
            SelectedEntry::Daily { line_idx, entry } => Some(DeleteTarget::Daily {
                line_idx,
                entry: Entry::from_raw(entry, current_date, line_idx, SourceType::Local),
            }),
            SelectedEntry::Filter { index, entry } => {
                Some(DeleteTarget::Filter { index, entry: entry.clone() })
            }
        })
    }

    /// Collect toggle targets for all selected entries (tasks only)
    fn collect_toggle_targets_from_selected(&self) -> Vec<ToggleTarget> {
        self.collect_targets_from_selected(|entry| {
            fn is_task(entry_type: &EntryType) -> bool {
                matches!(entry_type, EntryType::Task { .. })
            }

            match entry {
                SelectedEntry::Projected(projected) if is_task(&projected.entry_type) => {
                    Some(ToggleTarget::Projected(projected.clone()))
                }
                SelectedEntry::Daily { line_idx, entry } if is_task(&entry.entry_type) => {
                    Some(ToggleTarget::Daily { line_idx })
                }
                SelectedEntry::Filter { index, entry } if is_task(&entry.entry_type) => {
                    Some(ToggleTarget::Filter { index, entry: entry.clone() })
                }
                _ => None,
            }
        })
    }

    /// Collect yank targets for all selected entries (includes prefix for round-trip paste)
    fn collect_yank_targets_from_selected(&self) -> Vec<YankTarget> {
        self.collect_targets_from_selected(|entry| {
            let content = match entry {
                SelectedEntry::Projected(projected) => {
                    format!(
                        "{}{}",
                        projected.entry_type.prefix(),
                        projected.content
                    )
                }
                SelectedEntry::Daily { entry, .. } => {
                    format!("{}{}", entry.prefix(), entry.content)
                }
                SelectedEntry::Filter { entry, .. } => {
                    format!("{}{}", entry.entry_type.prefix(), entry.content)
                }
            };
            Some(YankTarget { content })
        })
    }

    /// Collect cycle targets (with entry types) for all selected entries
    fn collect_cycle_targets_from_selected(&self) -> Vec<super::actions::CycleTarget> {
        self.collect_targets_from_selected(|entry| match entry {
            SelectedEntry::Projected(projected) => Some(super::actions::CycleTarget {
                location: TagRemovalTarget::Projected(projected.clone()),
                original_type: projected.entry_type.clone(),
            }),
            SelectedEntry::Daily { line_idx, entry } => Some(super::actions::CycleTarget {
                location: TagRemovalTarget::Daily { line_idx },
                original_type: entry.entry_type.clone(),
            }),
            SelectedEntry::Filter { index, entry } => Some(super::actions::CycleTarget {
                location: TagRemovalTarget::Filter { index, entry: entry.clone() },
                original_type: entry.entry_type.clone(),
            }),
        })
    }

    /// Collect tag targets (with content) for all selected entries
    fn collect_tag_targets_from_selected(&self) -> Vec<super::actions::TagTarget> {
        self.collect_targets_from_selected(|entry| match entry {
            SelectedEntry::Projected(projected) => Some(super::actions::TagTarget {
                location: TagRemovalTarget::Projected(projected.clone()),
                original_content: projected.content.clone(),
            }),
            SelectedEntry::Daily { line_idx, entry } => Some(super::actions::TagTarget {
                location: TagRemovalTarget::Daily { line_idx },
                original_content: entry.content.clone(),
            }),
            SelectedEntry::Filter { index, entry } => Some(super::actions::TagTarget {
                location: TagRemovalTarget::Filter { index, entry: entry.clone() },
                original_content: entry.content.clone(),
            }),
        })
    }

    /// Delete all selected entries (yanks to clipboard first)
    pub fn delete_selected(&mut self) -> io::Result<()> {
        // Yank before deleting (like Vim)
        let yank_targets = self.collect_yank_targets_from_selected();
        if !yank_targets.is_empty() {
            let contents: Vec<&str> = yank_targets.iter().map(Self::yank_target_content).collect();
            let combined = contents.join("\n");
            let _ = Self::copy_to_clipboard(&combined);
        }

        let targets = self.collect_delete_targets_from_selected();
        if targets.is_empty() {
            self.exit_selection_mode();
            return Ok(());
        }

        let action = super::actions::DeleteEntries::new(targets);
        self.execute_action(Box::new(action))?;
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
        let targets = self.collect_tag_targets_from_selected();
        if targets.is_empty() {
            return Ok(());
        }

        let action = super::actions::RemoveLastTag::new(targets);
        self.execute_action(Box::new(action))
    }

    /// Remove all trailing tags from all selected entries
    pub fn remove_all_tags_from_selected(&mut self) -> io::Result<()> {
        let targets = self.collect_tag_targets_from_selected();
        if targets.is_empty() {
            return Ok(());
        }

        let action = super::actions::RemoveAllTags::new(targets);
        self.execute_action(Box::new(action))
    }

    /// Append a tag to all selected entries
    pub fn append_tag_to_selected(&mut self, tag: &str) -> io::Result<()> {
        let targets = self.collect_tag_targets_from_selected();
        if targets.is_empty() {
            return Ok(());
        }

        let action = super::actions::AppendTag::new(targets, tag.to_string());
        self.execute_action(Box::new(action))
    }

    /// Cycle entry type on all selected entries
    pub fn cycle_selected_entry_types(&mut self) -> io::Result<()> {
        let targets = self.collect_cycle_targets_from_selected();
        if targets.is_empty() {
            return Ok(());
        }

        let action = super::actions::CycleEntryType::new(targets);
        self.execute_action(Box::new(action))
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
