use std::io;

use crate::app::{App, EntryLocation, ViewMode};
use crate::storage::{self, EntryType, Line};

use super::types::{Action, ActionDescription, StatusVisibility};

/// Target for cycle type operations - includes original type for undo
#[derive(Clone)]
pub struct CycleTarget {
    pub location: EntryLocation,
    pub original_type: EntryType,
}

/// Action to cycle entry type(s)
pub struct CycleEntryType {
    targets: Vec<CycleTarget>,
}

impl CycleEntryType {
    #[must_use]
    pub fn new(targets: Vec<CycleTarget>) -> Self {
        Self { targets }
    }

    #[must_use]
    pub fn single(location: EntryLocation, original_type: EntryType) -> Self {
        Self::new(vec![CycleTarget {
            location,
            original_type,
        }])
    }
}

impl Action for CycleEntryType {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        for target in &self.targets {
            execute_cycle_raw(app, &target.location)?;
        }

        Ok(Box::new(RestoreEntryType::new(self.targets.clone())))
    }

    fn description(&self) -> ActionDescription {
        let count = self.targets.len();
        if count == 1 {
            ActionDescription {
                past: "Cycled entry type".to_string(),
                past_reversed: "Restored entry type".to_string(),
                visibility: StatusVisibility::Silent,
            }
        } else {
            ActionDescription {
                past: format!("Cycled type on {} entries", count),
                past_reversed: format!("Restored type on {} entries", count),
                visibility: StatusVisibility::Silent,
            }
        }
    }
}

/// Action to restore entry type(s) to their original values (reverse of cycle)
pub struct RestoreEntryType {
    /// Original targets (with types before cycling) - used for redo
    original_targets: Vec<CycleTarget>,
}

impl RestoreEntryType {
    fn new(original_targets: Vec<CycleTarget>) -> Self {
        Self { original_targets }
    }
}

impl Action for RestoreEntryType {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        // Restore each entry to its original type
        for target in &self.original_targets {
            set_entry_type_raw(app, &target.location, &target.original_type)?;
        }

        // Return an action that will re-cycle (redo)
        Ok(Box::new(CycleEntryType::new(self.original_targets.clone())))
    }

    fn description(&self) -> ActionDescription {
        let count = self.original_targets.len();
        if count == 1 {
            ActionDescription {
                past: "Restored entry type".to_string(),
                past_reversed: "Cycled entry type".to_string(),
                visibility: StatusVisibility::Silent,
            }
        } else {
            ActionDescription {
                past: format!("Restored type on {} entries", count),
                past_reversed: format!("Cycled type on {} entries", count),
                visibility: StatusVisibility::Silent,
            }
        }
    }
}

/// Execute cycle on a single target, returning the new type
fn execute_cycle_raw(app: &mut App, location: &EntryLocation) -> io::Result<Option<EntryType>> {
    let path = app.active_path().to_path_buf();

    match location {
        EntryLocation::Later {
            source_date,
            line_index,
        } => {
            let new_type = storage::cycle_entry_type(*source_date, &path, *line_index)?;
            if let Some(ref new_type) = new_type
                && let ViewMode::Daily(state) = &mut app.view
                    && let Some(later_entry) = state
                        .later_entries
                        .iter_mut()
                        .find(|e| e.source_date == *source_date && e.line_index == *line_index)
                    {
                        later_entry.entry_type = new_type.clone();
                        later_entry.completed =
                            matches!(later_entry.entry_type, EntryType::Task { completed: true });
                    }
            Ok(new_type)
        }
        EntryLocation::Daily { line_idx } => {
            if let Line::Entry(entry) = &mut app.lines[*line_idx] {
                let new_type = entry.entry_type.cycle();
                entry.entry_type = new_type.clone();
                app.save();
                Ok(Some(new_type))
            } else {
                Ok(None)
            }
        }
        EntryLocation::Filter {
            index,
            source_date,
            line_index,
        } => {
            let new_type = storage::cycle_entry_type(*source_date, &path, *line_index)?;
            if let Some(ref new_type) = new_type {
                if let ViewMode::Filter(state) = &mut app.view
                    && let Some(filter_entry) = state.entries.get_mut(*index) {
                        filter_entry.entry_type = new_type.clone();
                        filter_entry.completed =
                            matches!(filter_entry.entry_type, EntryType::Task { completed: true });
                    }

                if *source_date == app.current_date {
                    app.reload_current_day()?;
                }
            }
            Ok(new_type)
        }
    }
}

/// Set entry type directly (for undo/restore)
fn set_entry_type_raw(
    app: &mut App,
    location: &EntryLocation,
    entry_type: &EntryType,
) -> io::Result<()> {
    let path = app.active_path().to_path_buf();

    match location {
        EntryLocation::Later {
            source_date,
            line_index,
        } => {
            storage::mutate_entry(*source_date, &path, *line_index, |entry| {
                entry.entry_type = entry_type.clone();
            })?;
            if let ViewMode::Daily(state) = &mut app.view
                && let Some(later_entry) = state
                    .later_entries
                    .iter_mut()
                    .find(|e| e.source_date == *source_date && e.line_index == *line_index)
                {
                    later_entry.entry_type = entry_type.clone();
                    later_entry.completed =
                        matches!(entry_type, EntryType::Task { completed: true });
                }
        }
        EntryLocation::Daily { line_idx } => {
            if let Line::Entry(entry) = &mut app.lines[*line_idx] {
                entry.entry_type = entry_type.clone();
                app.save();
            }
        }
        EntryLocation::Filter {
            index,
            source_date,
            line_index,
        } => {
            storage::mutate_entry(*source_date, &path, *line_index, |entry| {
                entry.entry_type = entry_type.clone();
            })?;
            if let ViewMode::Filter(state) = &mut app.view
                && let Some(filter_entry) = state.entries.get_mut(*index) {
                    filter_entry.entry_type = entry_type.clone();
                    filter_entry.completed =
                        matches!(entry_type, EntryType::Task { completed: true });
                }

            if *source_date == app.current_date {
                app.reload_current_day()?;
            }
        }
    }
    Ok(())
}
