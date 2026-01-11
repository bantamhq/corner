use std::collections::HashMap;
use std::io;

use chrono::NaiveDate;

use super::super::{App, DeleteTarget, Line, ViewMode};
use super::types::{Action, ActionDescription};
use crate::storage::{self, Entry, EntryType};

fn pluralize(count: usize) -> &'static str {
    if count == 1 { "entry" } else { "entries" }
}

pub struct DeleteEntries {
    pub targets: Vec<DeleteTarget>,
}

impl DeleteEntries {
    #[must_use]
    pub fn new(targets: Vec<DeleteTarget>) -> Self {
        Self { targets }
    }

    #[must_use]
    pub fn single(target: DeleteTarget) -> Self {
        Self {
            targets: vec![target],
        }
    }
}

impl Action for DeleteEntries {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let mut deleted_entries = Vec::new();

        // Sort targets by line index descending for safe deletion
        self.targets.sort_by(|a, b| {
            let idx_a = match a {
                DeleteTarget::Daily { line_idx, .. } => *line_idx,
                DeleteTarget::Projected(entry) => entry.line_index,
                DeleteTarget::Filter { entry, .. } => entry.line_index,
            };
            let idx_b = match b {
                DeleteTarget::Daily { line_idx, .. } => *line_idx,
                DeleteTarget::Projected(entry) => entry.line_index,
                DeleteTarget::Filter { entry, .. } => entry.line_index,
            };
            idx_b.cmp(&idx_a)
        });

        for target in &self.targets {
            let entry_data = execute_delete_raw(app, target)?;
            deleted_entries.push(entry_data);
        }

        // Reverse so entries are in ascending order for restore
        deleted_entries.reverse();

        Ok(Box::new(RestoreEntries {
            entries: deleted_entries,
        }))
    }

    fn description(&self) -> ActionDescription {
        let count = self.targets.len();
        ActionDescription::always(
            format!("Deleted {}", pluralize(count)),
            format!("Restored {}", pluralize(count)),
        )
    }
}

pub struct RestoreEntries {
    entries: Vec<(NaiveDate, usize, Entry)>,
}

impl Action for RestoreEntries {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let mut delete_targets = Vec::new();

        // Sort by line index ascending for correct insertion order
        self.entries.sort_by_key(|(_, line_idx, _)| *line_idx);

        match &app.view {
            ViewMode::Daily(_) => {
                let all_same_day = self
                    .entries
                    .iter()
                    .all(|(date, _, _)| *date == app.current_date);

                if !all_same_day {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Cannot restore entries from different days in daily view",
                    ));
                }

                let mut any_completed = false;
                let mut last_insert_idx = 0;

                for (i, (_date, line_idx, entry)) in self.entries.iter().enumerate() {
                    let insert_idx = (line_idx + i).min(app.lines.len());
                    if matches!(entry.entry_type, EntryType::Task { completed: true }) {
                        any_completed = true;
                    }

                    delete_targets.push(DeleteTarget::Daily {
                        line_idx: insert_idx,
                        entry: entry.clone(),
                    });

                    app.lines.insert(insert_idx, Line::Entry(entry.to_raw()));
                    last_insert_idx = insert_idx;
                }

                app.entry_indices = App::compute_entry_indices(&app.lines);

                if app.hide_completed && any_completed {
                    app.hide_completed = false;
                }

                let visible_idx = app
                    .entry_indices
                    .iter()
                    .position(|&i| i == last_insert_idx)
                    .map(|actual_idx| app.actual_to_visible_index(actual_idx));

                if let ViewMode::Daily(state) = &mut app.view
                    && let Some(idx) = visible_idx
                {
                    state.selected = idx;
                }
                app.save();
            }
            ViewMode::Filter(_) => {
                let path = app.active_path().to_path_buf();

                // Group entries by date
                let mut entries_by_date: HashMap<NaiveDate, Vec<(usize, Entry)>> = HashMap::new();
                for (date, line_idx, entry) in &self.entries {
                    entries_by_date
                        .entry(*date)
                        .or_default()
                        .push((*line_idx, entry.clone()));
                }

                for (date, date_entries) in entries_by_date {
                    if let Ok(mut lines) = storage::load_day_lines(date, &path) {
                        for (i, (line_idx, entry)) in date_entries.into_iter().enumerate() {
                            let insert_idx = (line_idx + i).min(lines.len());

                            // Create the restored entry with updated line_index
                            let restored_entry = Entry {
                                entry_type: entry.entry_type.clone(),
                                content: entry.content.clone(),
                                source_date: date,
                                line_index: insert_idx,
                                source_type: entry.source_type.clone(),
                            };
                            lines.insert(insert_idx, Line::Entry(entry.to_raw()));

                            if let ViewMode::Filter(state) = &mut app.view {
                                let filter_index = state.entries.len();
                                state.entries.push(restored_entry.clone());
                                state.selected = filter_index;

                                delete_targets.push(DeleteTarget::Filter {
                                    index: filter_index,
                                    entry: restored_entry,
                                });
                            }
                        }
                        let _ = storage::save_day_lines(date, &path, &lines);

                        if date == app.current_date {
                            let _ = app.reload_current_day();
                        }
                    }
                }
            }
        }

        Ok(Box::new(DeleteEntries {
            targets: delete_targets,
        }))
    }

    fn description(&self) -> ActionDescription {
        let count = self.entries.len();
        ActionDescription::always(
            format!("Restored {}", pluralize(count)),
            format!("Deleted {}", pluralize(count)),
        )
    }
}

/// Execute a single delete without modifying undo state
fn execute_delete_raw(
    app: &mut App,
    target: &DeleteTarget,
) -> io::Result<(NaiveDate, usize, Entry)> {
    let path = app.active_path().to_path_buf();

    match target {
        DeleteTarget::Projected(entry) => {
            storage::delete_entry(entry.source_date, &path, entry.line_index)?;

            app.refresh_projected_entries();
            clamp_daily_selection(app);

            Ok((entry.source_date, entry.line_index, entry.clone()))
        }
        DeleteTarget::Daily { line_idx, entry } => {
            let result = (app.current_date, *line_idx, entry.clone());
            app.lines.remove(*line_idx);
            app.entry_indices = App::compute_entry_indices(&app.lines);
            clamp_daily_selection(app);
            app.save();
            // Refresh projected entries in case we deleted a ↺ entry that was hiding a recurring
            app.refresh_projected_entries();
            Ok(result)
        }
        DeleteTarget::Filter { index, entry } => {
            storage::delete_entry(entry.source_date, &path, entry.line_index)?;

            if let ViewMode::Filter(state) = &mut app.view {
                state.entries.remove(*index);

                for filter_entry in &mut state.entries {
                    if filter_entry.source_date == entry.source_date
                        && filter_entry.line_index > entry.line_index
                    {
                        filter_entry.line_index -= 1;
                    }
                }

                if !state.entries.is_empty() && state.selected >= state.entries.len() {
                    state.selected = state.entries.len() - 1;
                }
            }

            if entry.source_date == app.current_date {
                app.reload_current_day()?;
            }

            Ok((entry.source_date, entry.line_index, entry.clone()))
        }
    }
}

fn clamp_daily_selection(app: &mut App) {
    let visible = app.visible_entry_count();
    if let ViewMode::Daily(state) = &mut app.view
        && visible > 0
        && state.selected >= visible
    {
        state.selected = visible - 1;
    }
}
