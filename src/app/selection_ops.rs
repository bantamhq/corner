use std::io;

use chrono::NaiveDate;

use crate::storage::{self, Entry, EntryType, RawEntry, SourceType};

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
    fn is_read_only_projected_index(&self, visible_idx: usize) -> bool {
        matches!(&self.view, ViewMode::Daily(_))
            && self
                .get_projected_at_visible_index(visible_idx)
                .is_some_and(|e| matches!(e.source_type, SourceType::Recurring))
    }

    fn visible_recurring_count(&self) -> usize {
        let ViewMode::Daily(state) = &self.view else {
            return 0;
        };
        state
            .projected_entries
            .iter()
            .filter(|e| matches!(e.source_type, SourceType::Recurring) && self.should_show_entry(e))
            .count()
    }

    pub fn enter_selection_mode(&mut self) {
        let current = self.current_visible_index();
        if current < self.visible_entry_count() && !self.is_read_only_projected_index(current) {
            self.input_mode = InputMode::Selection(SelectionState::new(current));
        }
    }

    pub fn cancel_selection_mode(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub fn selection_move_down(&mut self) {
        if let InputMode::Selection(ref mut state) = self.input_mode {
            state.on_cursor_move();
        }
        self.move_down();
    }

    pub fn selection_move_up(&mut self) {
        if let InputMode::Selection(ref mut state) = self.input_mode {
            state.on_cursor_move();
        }
        let recurring_count = self.visible_recurring_count();
        let current = self.current_visible_index();
        if current > recurring_count {
            self.move_up();
        }
    }

    pub fn selection_jump_to_first(&mut self) {
        if let InputMode::Selection(ref mut state) = self.input_mode {
            state.on_cursor_move();
        }
        let recurring_count = self.visible_recurring_count();
        match &mut self.view {
            ViewMode::Daily(state) => state.selected = recurring_count,
            ViewMode::Filter(state) => state.selected = 0,
        }
    }

    pub fn selection_jump_to_last(&mut self) {
        if let InputMode::Selection(ref mut state) = self.input_mode {
            state.on_cursor_move();
        }
        self.jump_to_last();
    }

    pub fn selection_extend_to_cursor(&mut self) {
        let current = self.current_visible_index();
        let recurring_count = self.visible_recurring_count();
        if let InputMode::Selection(ref mut state) = self.input_mode {
            state.extend_to(current);
            if matches!(&self.view, ViewMode::Daily(_)) {
                state.selected_indices.retain(|&idx| idx >= recurring_count);
            }
        }
    }

    pub fn selection_toggle_current(&mut self) {
        let current = self.current_visible_index();
        if self.is_read_only_projected_index(current) {
            return;
        }
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

    /// Get daily entry at visible entry index (after projected entries)
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

    fn collect_delete_targets_from_selected(&self) -> Vec<DeleteTarget> {
        let current_date = self.current_date;
        let active_path = self.active_path().to_path_buf();
        self.collect_targets_from_selected(|entry| match entry {
            SelectedEntry::Projected(projected) => {
                if matches!(projected.source_type, SourceType::Recurring) {
                    None
                } else {
                    Some(DeleteTarget::Projected(projected.clone()))
                }
            }
            SelectedEntry::Daily { line_idx, entry } => Some(DeleteTarget::Daily {
                line_idx,
                entry: Entry::from_raw(
                    entry,
                    current_date,
                    line_idx,
                    SourceType::Local,
                    active_path.clone(),
                ),
            }),
            SelectedEntry::Filter { index, entry } => Some(DeleteTarget::Filter {
                index,
                entry: entry.clone(),
            }),
        })
    }

    fn collect_toggle_targets_from_selected(&self) -> Vec<ToggleTarget> {
        self.collect_targets_from_selected(|entry| {
            let (entry_type, target) = match entry {
                SelectedEntry::Projected(projected) => (
                    &projected.entry_type,
                    ToggleTarget::Projected(projected.clone()),
                ),
                SelectedEntry::Daily { line_idx, entry } => (
                    &entry.entry_type,
                    ToggleTarget::Daily {
                        line_idx,
                        source_path: None,
                    },
                ),
                SelectedEntry::Filter { index, entry } => (
                    &entry.entry_type,
                    ToggleTarget::Filter {
                        index,
                        entry: entry.clone(),
                    },
                ),
            };
            matches!(entry_type, EntryType::Task { .. }).then_some(target)
        })
    }

    /// Collect yank targets for all selected entries (includes prefix for round-trip paste)
    fn collect_yank_targets_from_selected(&self) -> Vec<YankTarget> {
        self.collect_targets_from_selected(|entry| {
            let content = match entry {
                SelectedEntry::Projected(projected) => {
                    format!("{}{}", projected.entry_type.prefix(), projected.content)
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
                location: TagRemovalTarget::Daily { line_idx, source_path: None },
                original_type: entry.entry_type.clone(),
            }),
            SelectedEntry::Filter { index, entry } => Some(super::actions::CycleTarget {
                location: TagRemovalTarget::Filter {
                    index,
                    entry: entry.clone(),
                },
                original_type: entry.entry_type.clone(),
            }),
        })
    }

    /// Collect content targets (location + content) for all selected entries.
    /// Used for tag operations, date operations, and other content transformations.
    fn collect_content_targets_from_selected(&self) -> Vec<super::actions::ContentTarget> {
        self.collect_targets_from_selected(|entry| match entry {
            SelectedEntry::Projected(projected) => Some(super::actions::ContentTarget::new(
                TagRemovalTarget::Projected(projected.clone()),
                projected.content.clone(),
            )),
            SelectedEntry::Daily { line_idx, entry } => Some(super::actions::ContentTarget::new(
                TagRemovalTarget::Daily { line_idx, source_path: None },
                entry.content.clone(),
            )),
            SelectedEntry::Filter { index, entry } => Some(super::actions::ContentTarget::new(
                TagRemovalTarget::Filter {
                    index,
                    entry: entry.clone(),
                },
                entry.content.clone(),
            )),
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
            self.cancel_selection_mode();
            return Ok(());
        }

        let action = super::actions::DeleteEntries::new(targets);
        self.execute_action(Box::new(action))?;
        self.cancel_selection_mode();
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
        let targets = self.collect_content_targets_from_selected();
        if targets.is_empty() {
            return Ok(());
        }

        let action = super::actions::RemoveLastTag::new(targets);
        self.execute_action(Box::new(action))
    }

    /// Remove all trailing tags from all selected entries
    pub fn remove_all_tags_from_selected(&mut self) -> io::Result<()> {
        let targets = self.collect_content_targets_from_selected();
        if targets.is_empty() {
            return Ok(());
        }

        let action = super::actions::RemoveAllTags::new(targets);
        self.execute_action(Box::new(action))
    }

    /// Append a tag to all selected entries
    pub fn append_tag_to_selected(&mut self, tag: &str) -> io::Result<()> {
        let targets = self.collect_content_targets_from_selected();
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

    /// Collect raw entries from selected entries for move operations
    fn collect_raw_entries_from_selected(&self) -> Vec<RawEntry> {
        self.collect_targets_from_selected(|entry| match entry {
            SelectedEntry::Projected(projected) => {
                // Skip read-only recurring entries
                if matches!(projected.source_type, SourceType::Recurring) {
                    return None;
                }
                Some(RawEntry {
                    entry_type: projected.entry_type.clone(),
                    content: projected.content.clone(),
                })
            }
            SelectedEntry::Daily { entry, .. } => Some(entry.clone()),
            SelectedEntry::Filter { entry, .. } => {
                let content = storage::get_entry_content(
                    entry.source_date,
                    self.active_path(),
                    entry.line_index,
                )
                .unwrap_or_default();
                Some(RawEntry {
                    entry_type: entry.entry_type.clone(),
                    content,
                })
            }
        })
    }

    /// Move all selected entries to a target date
    fn move_selected_to_date(&mut self, target_date: NaiveDate) -> io::Result<()> {
        let raw_entries = self.collect_raw_entries_from_selected();
        if raw_entries.is_empty() {
            self.cancel_selection_mode();
            self.set_status("No movable entries selected");
            return Ok(());
        }

        let count = raw_entries.len();

        // Yank before deleting (like Vim)
        let yank_targets = self.collect_yank_targets_from_selected();
        if !yank_targets.is_empty() {
            let contents: Vec<&str> = yank_targets.iter().map(Self::yank_target_content).collect();
            let combined = contents.join("\n");
            let _ = Self::copy_to_clipboard(&combined);
        }

        // Delete from sources
        let delete_targets = self.collect_delete_targets_from_selected();
        if !delete_targets.is_empty() {
            let action = super::actions::DeleteEntries::new(delete_targets);
            self.execute_action(Box::new(action))?;
        }

        // Add to target date
        self.add_entries_to_date(raw_entries, target_date)?;
        self.cancel_selection_mode();
        self.set_status(format!(
            "Moved {} entries to {}",
            count,
            target_date.format("%m/%d")
        ));
        Ok(())
    }

    /// Move all selected entries to today
    pub fn move_selected_to_today(&mut self) -> io::Result<()> {
        let today = chrono::Local::now().date_naive();
        self.move_selected_to_date(today)
    }

    pub fn defer_selected(&mut self) -> io::Result<()> {
        // Defer from the current viewed date (selection is always in daily view)
        let target = self.defer_date_from(self.current_date);
        self.move_selected_to_date(target)
    }
}
