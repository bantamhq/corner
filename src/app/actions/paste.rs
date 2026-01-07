use std::io;

use chrono::NaiveDate;

use crate::app::{App, ViewMode};
use crate::storage::{self, Entry, Line};

use super::types::{Action, ActionDescription, StatusVisibility};

/// Target for paste operations - captures where entries were inserted
#[derive(Clone)]
pub struct PasteTarget {
    /// Date where entries were pasted
    pub date: NaiveDate,
    /// Line index of first inserted entry
    pub start_line_index: usize,
    /// The pasted entries
    pub entries: Vec<Entry>,
}

fn pluralize(count: usize) -> &'static str {
    if count == 1 { "entry" } else { "entries" }
}

/// Action to paste entries (used after save completes)
pub struct PasteEntries {
    target: PasteTarget,
}

impl PasteEntries {
    #[must_use]
    pub fn new(target: PasteTarget) -> Self {
        Self { target }
    }
}

impl Action for PasteEntries {
    fn execute(&mut self, _app: &mut App) -> io::Result<Box<dyn Action>> {
        // Entries were already pasted when this action was made,
        // just return the reverse action
        Ok(Box::new(UnpasteEntries::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        let count = self.target.entries.len();
        ActionDescription {
            past: format!("Pasted {} {}", count, pluralize(count)),
            past_reversed: format!("Removed {} pasted {}", count, pluralize(count)),
            visibility: StatusVisibility::Always,
        }
    }
}

/// Action to remove pasted entries (reverse of paste)
struct UnpasteEntries {
    target: PasteTarget,
}

impl UnpasteEntries {
    fn new(target: PasteTarget) -> Self {
        Self { target }
    }
}

impl Action for UnpasteEntries {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let path = app.active_path().to_path_buf();

        // Delete entries in reverse order to maintain indices
        for i in (0..self.target.entries.len()).rev() {
            let line_index = self.target.start_line_index + i;
            storage::delete_entry(self.target.date, &path, line_index)?;
        }

        if self.target.date == app.current_date {
            app.reload_current_day()?;
            app.clamp_selection_to_visible();
        }

        if let ViewMode::Filter(_) = &app.view {
            let _ = app.refresh_filter();
        }

        Ok(Box::new(RepasteEntries::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        let count = self.target.entries.len();
        ActionDescription {
            past: format!("Removed {} pasted {}", count, pluralize(count)),
            past_reversed: format!("Pasted {} {}", count, pluralize(count)),
            visibility: StatusVisibility::Always,
        }
    }
}

/// Action to repaste entries (redo of paste)
struct RepasteEntries {
    target: PasteTarget,
}

impl RepasteEntries {
    fn new(target: PasteTarget) -> Self {
        Self { target }
    }
}

impl Action for RepasteEntries {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let path = app.active_path().to_path_buf();

        let mut lines = storage::load_day_lines(self.target.date, &path)?;

        for (i, entry) in self.target.entries.iter().enumerate() {
            let insert_pos = (self.target.start_line_index + i).min(lines.len());
            lines.insert(insert_pos, Line::Entry(entry.to_raw()));
        }

        storage::save_day_lines(self.target.date, &path, &lines)?;

        if self.target.date == app.current_date {
            app.reload_current_day()?;
        }

        if let ViewMode::Filter(_) = &app.view {
            let _ = app.refresh_filter();
        }

        Ok(Box::new(UnpasteEntries::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        let count = self.target.entries.len();
        ActionDescription {
            past: format!("Pasted {} {}", count, pluralize(count)),
            past_reversed: format!("Removed {} pasted {}", count, pluralize(count)),
            visibility: StatusVisibility::Always,
        }
    }
}
