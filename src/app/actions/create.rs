use std::io;

use chrono::NaiveDate;

use crate::app::{App, ViewMode};
use crate::storage::{self, Entry, Line};

use super::types::{Action, ActionDescription, StatusVisibility};

/// Target for create operations - captures where and what was created
#[derive(Clone)]
pub struct CreateTarget {
    /// Date where entry was created
    pub date: NaiveDate,
    /// Line index of created entry
    pub line_index: usize,
    /// The created entry
    pub entry: Entry,
    /// Whether this was created in filter view (vs daily view)
    pub is_filter_quick_add: bool,
}

/// Action to create an entry (used after save completes)
pub struct CreateEntry {
    target: CreateTarget,
}

impl CreateEntry {
    #[must_use]
    pub fn new(target: CreateTarget) -> Self {
        Self { target }
    }
}

impl Action for CreateEntry {
    fn execute(&mut self, _app: &mut App) -> io::Result<Box<dyn Action>> {
        // Entry was already created when this action was made,
        // just return the reverse action
        Ok(Box::new(UncreateEntry::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        ActionDescription {
            past: "Created entry".to_string(),
            past_reversed: "Removed entry".to_string(),
            visibility: StatusVisibility::OnUndo,
        }
    }
}

/// Action to remove a created entry (reverse of create)
pub struct UncreateEntry {
    target: CreateTarget,
}

impl UncreateEntry {
    fn new(target: CreateTarget) -> Self {
        Self { target }
    }
}

impl Action for UncreateEntry {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let path = app.active_path().to_path_buf();

        // Delete the entry
        storage::delete_entry(self.target.date, &path, self.target.line_index)?;

        // Update app state
        if self.target.date == app.current_date {
            app.reload_current_day()?;
            app.clamp_selection_to_visible();
        }

        if self.target.is_filter_quick_add
            && let ViewMode::Filter(state) = &mut app.view {
                // Remove from filter entries and adjust indices
                state.entries.retain(|e| {
                    !(e.source_date == self.target.date && e.line_index == self.target.line_index)
                });
                for entry in &mut state.entries {
                    if entry.source_date == self.target.date
                        && entry.line_index > self.target.line_index
                    {
                        entry.line_index -= 1;
                    }
                }
                if !state.entries.is_empty() && state.selected >= state.entries.len() {
                    state.selected = state.entries.len() - 1;
                }
            }

        // Return action to redo the create
        Ok(Box::new(RecreateEntry::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        ActionDescription {
            past: "Removed entry".to_string(),
            past_reversed: "Created entry".to_string(),
            visibility: StatusVisibility::OnUndo,
        }
    }
}

/// Action to recreate an entry (redo of create)
pub struct RecreateEntry {
    target: CreateTarget,
}

impl RecreateEntry {
    fn new(target: CreateTarget) -> Self {
        Self { target }
    }
}

impl Action for RecreateEntry {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let path = app.active_path().to_path_buf();

        // Recreate the entry at the original position
        let mut lines = storage::load_day_lines(self.target.date, &path)?;

        // Insert at original position (or end if position is beyond current length)
        let insert_pos = self.target.line_index.min(lines.len());
        lines.insert(insert_pos, Line::Entry(self.target.entry.clone()));

        storage::save_day_lines(self.target.date, &path, &lines)?;

        // Update app state
        if self.target.date == app.current_date {
            app.reload_current_day()?;
        }

        if self.target.is_filter_quick_add {
            // Refresh filter to pick up the recreated entry
            let _ = app.refresh_filter();
        }

        // Return action to undo again
        Ok(Box::new(UncreateEntry::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        ActionDescription {
            past: "Created entry".to_string(),
            past_reversed: "Removed entry".to_string(),
            visibility: StatusVisibility::OnUndo,
        }
    }
}
