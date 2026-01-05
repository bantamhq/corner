use std::io;

use crate::app::{App, EntryLocation, ViewMode};
use crate::storage::{self, EntryType, Line};

use super::types::{Action, ActionDescription, StatusVisibility};

/// Target for edit operations - includes location and both original and new content
#[derive(Clone)]
pub struct EditTarget {
    pub location: EntryLocation,
    pub original_content: String,
    pub new_content: String,
    pub entry_type: EntryType,
}

/// Action to edit entry content (captures before/after for undo)
pub struct EditEntry {
    target: EditTarget,
}

impl EditEntry {
    #[must_use]
    pub fn new(target: EditTarget) -> Self {
        Self { target }
    }
}

impl Action for EditEntry {
    fn execute(&mut self, _app: &mut App) -> io::Result<Box<dyn Action>> {
        // Content was already saved when this action was created,
        // so just return the reverse action
        Ok(Box::new(RestoreEdit::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        ActionDescription {
            past: "Edited entry".to_string(),
            past_reversed: "Reverted edit".to_string(),
            visibility: StatusVisibility::OnUndo,
        }
    }
}

/// Action to restore original content (reverse of edit)
pub struct RestoreEdit {
    target: EditTarget,
}

impl RestoreEdit {
    fn new(target: EditTarget) -> Self {
        Self { target }
    }
}

impl Action for RestoreEdit {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        // Restore original content
        set_entry_content_raw(app, &self.target)?;

        // Return action to redo the edit
        Ok(Box::new(RedoEdit::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        ActionDescription {
            past: "Reverted edit".to_string(),
            past_reversed: "Edited entry".to_string(),
            visibility: StatusVisibility::OnUndo,
        }
    }
}

/// Action to redo an edit
pub struct RedoEdit {
    target: EditTarget,
}

impl RedoEdit {
    fn new(target: EditTarget) -> Self {
        Self { target }
    }
}

impl Action for RedoEdit {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        // Restore new content
        let mut redo_target = self.target.clone();
        std::mem::swap(&mut redo_target.original_content, &mut redo_target.new_content);
        set_entry_content_raw(app, &redo_target)?;

        // Return action to undo again
        Ok(Box::new(RestoreEdit::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        ActionDescription {
            past: "Edited entry".to_string(),
            past_reversed: "Reverted edit".to_string(),
            visibility: StatusVisibility::OnUndo,
        }
    }
}

/// Set entry content for undo/redo
fn set_entry_content_raw(app: &mut App, target: &EditTarget) -> io::Result<()> {
    let path = app.active_path().to_path_buf();

    match &target.location {
        EntryLocation::Later {
            source_date,
            line_index,
        } => {
            storage::mutate_entry(*source_date, &path, *line_index, |entry| {
                entry.content = target.original_content.clone();
            })?;

            if let ViewMode::Daily(state) = &mut app.view {
                state.later_entries =
                    storage::collect_later_entries_for_date(app.current_date, &path)?;
            }
        }
        EntryLocation::Daily { line_idx } => {
            if let Line::Entry(entry) = &mut app.lines[*line_idx] {
                entry.content = target.original_content.clone();
                app.save();
            }
        }
        EntryLocation::Filter {
            index,
            source_date,
            line_index,
        } => {
            storage::mutate_entry(*source_date, &path, *line_index, |entry| {
                entry.content = target.original_content.clone();
            })?;

            if let ViewMode::Filter(state) = &mut app.view
                && let Some(filter_entry) = state.entries.get_mut(*index) {
                    filter_entry.content = target.original_content.clone();
                }

            if *source_date == app.current_date {
                app.reload_current_day()?;
            }
        }
    }
    Ok(())
}
