use std::io;
use std::path::PathBuf;

use crate::storage::{self, Entry, JournalSlot, Line, ProjectRegistry};

use super::{
    App, CombinedGroup, CombinedPosition, DailyState, EntryLocation, InputMode, SelectedItem,
    ViewMode,
};

impl App {
    pub fn toggle_combined_view(&mut self) -> io::Result<()> {
        if self.active_journal() != JournalSlot::Hub {
            self.set_error("Combined view is only available from the Hub journal");
            return Ok(());
        }
        if !matches!(self.input_mode, InputMode::Normal) {
            return Ok(());
        }

        self.combined_view = !self.combined_view;

        if self.combined_view {
            self.load_combined_data()?;
            self.set_status("Combined view");
        } else {
            self.combined_groups.clear();
            self.reload_current_view();
            self.set_status("Hub journal");
        }

        self.executor.clear();
        Ok(())
    }

    pub(super) fn load_combined_data(&mut self) -> io::Result<()> {
        match &self.view {
            ViewMode::Daily(_) => self.load_combined_daily(),
            ViewMode::Filter(_) => self.load_combined_filter().map(|_| ()),
        }
    }

    pub(super) fn load_combined_daily(&mut self) -> io::Result<()> {
        let date = self.current_date;
        let mut groups = Vec::new();

        let hub_path = self.journal_context.hub_path().to_path_buf();
        if let Some(group) = self.build_group_for_journal("Hub", &hub_path, date)? {
            groups.push(group);
        }

        let registry = ProjectRegistry::load();
        for project in &registry.projects {
            if !project.available || project.hide_from_registry {
                continue;
            }
            let journal_path = project.journal_path();
            if !journal_path.exists() {
                continue;
            }
            if let Some(group) =
                self.build_group_for_journal(&project.name, &journal_path, date)?
            {
                groups.push(group);
            }
        }

        self.combined_groups = groups;

        let total = self.combined_visible_count();
        let selected = total.saturating_sub(1);
        self.view = ViewMode::Daily(DailyState {
            selected,
            scroll_offset: 0,
            original_lines: None,
            projected_entries: Vec::new(),
        });

        if self.hide_completed {
            self.clamp_selection_to_visible();
        }
        Ok(())
    }

    fn build_group_for_journal(
        &self,
        name: &str,
        path: &std::path::Path,
        date: chrono::NaiveDate,
    ) -> io::Result<Option<CombinedGroup>> {
        let lines = storage::load_day_lines(date, path)?;
        let entry_indices = Self::compute_entry_indices(&lines);
        let mut projected = storage::collect_projected_entries_for_date(date, path)?;

        for entry in &mut projected {
            entry.source_journal = path.to_path_buf();
        }

        let has_entries = !entry_indices.is_empty() || !projected.is_empty();
        if !has_entries {
            return Ok(None);
        }

        Ok(Some(CombinedGroup {
            project_name: name.to_string(),
            journal_path: path.to_path_buf(),
            lines,
            entry_indices,
            projected_entries: projected,
        }))
    }

    pub(super) fn load_combined_filter(&mut self) -> io::Result<Vec<Entry>> {
        let ViewMode::Filter(state) = &self.view else {
            return Ok(Vec::new());
        };

        let (query, unknown_filters) =
            storage::expand_saved_filters(&state.query, &self.config.filters);
        let mut filter = storage::parse_filter_query(&query);
        filter.invalid_tokens.extend(unknown_filters);

        if !filter.invalid_tokens.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_entries = self.collect_entries_from_journal(&filter, self.journal_context.hub_path())?;

        let registry = ProjectRegistry::load();
        for project in &registry.projects {
            if !project.available || project.hide_from_registry {
                continue;
            }
            let journal_path = project.journal_path();
            if journal_path.exists() {
                all_entries.extend(self.collect_entries_from_journal(&filter, &journal_path)?);
            }
        }

        // Sort by journal path first (matches BTreeMap grouping in render), then date
        all_entries.sort_by(|a, b| {
            a.source_journal
                .cmp(&b.source_journal)
                .then(a.source_date.cmp(&b.source_date))
        });

        if let ViewMode::Filter(state) = &mut self.view {
            state.entries = all_entries.clone();
            state.selected = state.selected.min(state.entries.len().saturating_sub(1));
            state.scroll_offset = 0;
        }

        Ok(all_entries)
    }

    fn collect_entries_from_journal(
        &self,
        filter: &storage::Filter,
        path: &std::path::Path,
    ) -> io::Result<Vec<Entry>> {
        let mut entries = storage::collect_filtered_entries(filter, path)?;
        for entry in &mut entries {
            entry.source_journal = path.to_path_buf();
        }
        Ok(entries)
    }

    #[must_use]
    pub fn combined_visible_count(&self) -> usize {
        self.combined_groups
            .iter()
            .map(|group| self.group_visible_count(group))
            .sum()
    }

    fn group_visible_count(&self, group: &CombinedGroup) -> usize {
        let visible_projected = if self.hide_completed {
            group
                .projected_entries
                .iter()
                .filter(|e| self.should_show_entry(e))
                .count()
        } else {
            group.projected_entries.len()
        };
        let visible_entries = if self.hide_completed {
            group
                .entry_indices
                .iter()
                .filter(|&&i| {
                    if let Line::Entry(raw_entry) = &group.lines[i] {
                        self.should_show_raw_entry(raw_entry)
                    } else {
                        true
                    }
                })
                .count()
        } else {
            group.entry_indices.len()
        };
        visible_projected + visible_entries
    }

    #[must_use]
    pub fn resolve_combined_position(&self, visible_idx: usize) -> Option<CombinedPosition> {
        let mut running = 0;

        for (group_index, group) in self.combined_groups.iter().enumerate() {
            // Projected entries
            for (entry_idx, entry) in group.projected_entries.iter().enumerate() {
                if self.hide_completed && !self.should_show_entry(entry) {
                    continue;
                }
                if running == visible_idx {
                    return Some(CombinedPosition {
                        group_index,
                        is_projected: true,
                        entry_index: entry_idx,
                    });
                }
                running += 1;
            }

            // Regular entries
            for (actual_idx, &line_idx) in group.entry_indices.iter().enumerate() {
                if let Line::Entry(raw_entry) = &group.lines[line_idx] {
                    if self.hide_completed && !self.should_show_raw_entry(raw_entry) {
                        continue;
                    }
                    if running == visible_idx {
                        return Some(CombinedPosition {
                            group_index,
                            is_projected: false,
                            entry_index: actual_idx,
                        });
                    }
                    running += 1;
                }
            }
        }

        None
    }

    #[must_use]
    pub fn get_combined_selected_item(&self) -> SelectedItem<'_> {
        let ViewMode::Daily(state) = &self.view else {
            return SelectedItem::None;
        };

        let Some(pos) = self.resolve_combined_position(state.selected) else {
            return SelectedItem::None;
        };

        let group = &self.combined_groups[pos.group_index];

        if pos.is_projected {
            let entry = &group.projected_entries[pos.entry_index];
            return SelectedItem::Projected {
                index: pos.entry_index,
                entry,
            };
        }

        let line_idx = group.entry_indices[pos.entry_index];
        if let Line::Entry(raw_entry) = &group.lines[line_idx] {
            SelectedItem::Daily {
                index: pos.entry_index,
                line_idx,
                entry: raw_entry,
            }
        } else {
            SelectedItem::None
        }
    }

    #[must_use]
    pub fn resolve_combined_journal_path(&self, visible_idx: usize) -> Option<PathBuf> {
        let pos = self.resolve_combined_position(visible_idx)?;
        Some(self.combined_groups[pos.group_index].journal_path.clone())
    }

    #[must_use]
    pub fn resolve_entry_path(&self, location: &EntryLocation) -> PathBuf {
        if !self.combined_view {
            if let EntryLocation::Daily { source_path: Some(path), .. } = location {
                return path.clone();
            }
            return self.active_path().to_path_buf();
        }

        match location {
            EntryLocation::Projected(entry) | EntryLocation::Filter { entry, .. } => {
                entry.source_journal.clone()
            }
            EntryLocation::Daily { source_path, .. } => source_path
                .clone()
                .unwrap_or_else(|| self.active_path().to_path_buf()),
        }
    }

    pub fn combined_hidden_completed_count(&self) -> usize {
        self.combined_groups
            .iter()
            .map(|group| self.group_hidden_completed_count(group))
            .sum()
    }

    fn group_hidden_completed_count(&self, group: &CombinedGroup) -> usize {
        let hidden_projected = group
            .projected_entries
            .iter()
            .filter(|e| matches!(e.entry_type, crate::storage::EntryType::Task { completed: true }))
            .count();
        let hidden_regular = group
            .entry_indices
            .iter()
            .filter(|&&i| {
                if let Line::Entry(raw_entry) = &group.lines[i] {
                    matches!(
                        raw_entry.entry_type,
                        crate::storage::EntryType::Task { completed: true }
                    )
                } else {
                    false
                }
            })
            .count();
        hidden_projected + hidden_regular
    }

    pub(super) fn deactivate_combined(&mut self) {
        self.combined_view = false;
        self.combined_groups.clear();
    }
}
