use std::io;

use crate::app::{App, EntryLocation};
use crate::storage::{SourceType, bring_to_today, defer_date, remove_date};

use super::content_ops::{
    ContentTarget, execute_content_operation, get_entry_content, set_entry_content,
};
use super::types::{Action, ActionDescription, StatusVisibility};

/// Target for date operations (type alias for ContentTarget)
pub type DateTarget = ContentTarget;

fn date_action_description(count: usize, singular: &str, plural: &str) -> ActionDescription {
    if count == 1 {
        ActionDescription {
            past: singular.to_string(),
            past_reversed: "Restored date".to_string(),
            visibility: StatusVisibility::Always,
        }
    } else {
        ActionDescription {
            past: format!("{plural} {count} entries"),
            past_reversed: format!("Restored dates on {count} entries"),
            visibility: StatusVisibility::Always,
        }
    }
}

/// Action to defer the @date by 1 day
pub struct DeferDate {
    targets: Vec<DateTarget>,
}

impl DeferDate {
    #[must_use]
    pub fn new(targets: Vec<DateTarget>) -> Self {
        Self { targets }
    }

    #[must_use]
    pub fn single(location: EntryLocation, original_content: String) -> Self {
        Self::new(vec![DateTarget {
            location,
            original_content,
        }])
    }
}

impl Action for DeferDate {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let today = chrono::Local::now().date_naive();
        for target in &self.targets {
            execute_content_operation(app, &target.location, |content| defer_date(content, today))?;
        }

        Ok(Box::new(RestoreDate::new(
            self.targets.clone(),
            DateOperation::Defer,
        )))
    }

    fn description(&self) -> ActionDescription {
        date_action_description(self.targets.len(), "Deferred date", "Deferred dates on")
    }
}

/// Action to remove the @date from entries
pub struct RemoveDate {
    targets: Vec<DateTarget>,
}

impl RemoveDate {
    #[must_use]
    pub fn new(targets: Vec<DateTarget>) -> Self {
        Self { targets }
    }

    #[must_use]
    pub fn single(location: EntryLocation, original_content: String) -> Self {
        Self::new(vec![DateTarget {
            location,
            original_content,
        }])
    }
}

impl Action for RemoveDate {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        for target in &self.targets {
            execute_content_operation(app, &target.location, remove_date)?;
        }

        Ok(Box::new(RestoreDate::new(
            self.targets.clone(),
            DateOperation::Remove,
        )))
    }

    fn description(&self) -> ActionDescription {
        date_action_description(self.targets.len(), "Removed date", "Removed dates from")
    }
}

/// Action to set @date to today
pub struct BringToToday {
    targets: Vec<DateTarget>,
}

impl BringToToday {
    #[must_use]
    pub fn new(targets: Vec<DateTarget>) -> Self {
        Self { targets }
    }

    #[must_use]
    pub fn single(location: EntryLocation, original_content: String) -> Self {
        Self::new(vec![DateTarget {
            location,
            original_content,
        }])
    }
}

impl Action for BringToToday {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let today = chrono::Local::now().date_naive();
        for target in &self.targets {
            execute_content_operation(app, &target.location, |content| {
                Some(bring_to_today(content, today))
            })?;
        }

        Ok(Box::new(RestoreDate::new(
            self.targets.clone(),
            DateOperation::BringToToday,
        )))
    }

    fn description(&self) -> ActionDescription {
        date_action_description(self.targets.len(), "Set date to today", "Set date to today on")
    }
}

/// Which operation was performed (for redo)
#[derive(Clone)]
enum DateOperation {
    Defer,
    Remove,
    BringToToday,
}

/// Action to restore original content (reverse of date operations)
pub struct RestoreDate {
    targets: Vec<DateTarget>,
    operation: DateOperation,
}

impl RestoreDate {
    fn new(targets: Vec<DateTarget>, operation: DateOperation) -> Self {
        Self { targets, operation }
    }
}

impl Action for RestoreDate {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let mut new_targets = Vec::with_capacity(self.targets.len());
        for target in &self.targets {
            let current_content = get_entry_content(app, &target.location)?;
            set_entry_content(app, &target.location, &target.original_content)?;
            new_targets.push(ContentTarget::new(target.location.clone(), current_content));
        }

        let redo_action: Box<dyn Action> = match &self.operation {
            DateOperation::Defer => Box::new(DeferDate::new(new_targets)),
            DateOperation::Remove => Box::new(RemoveDate::new(new_targets)),
            DateOperation::BringToToday => Box::new(BringToToday::new(new_targets)),
        };

        Ok(redo_action)
    }

    fn description(&self) -> ActionDescription {
        let count = self.targets.len();
        let (singular, plural) = match &self.operation {
            DateOperation::Defer => ("Deferred date", "Deferred dates on"),
            DateOperation::Remove => ("Removed date", "Removed dates from"),
            DateOperation::BringToToday => ("Set date to today", "Set date to today on"),
        };

        if count == 1 {
            ActionDescription {
                past: "Restored date".to_string(),
                past_reversed: singular.to_string(),
                visibility: StatusVisibility::Always,
            }
        } else {
            ActionDescription {
                past: format!("Restored dates on {count} entries"),
                past_reversed: format!("{plural} {count} entries"),
                visibility: StatusVisibility::Always,
            }
        }
    }
}

/// Check if an entry is a recurring entry (cannot be deferred)
#[must_use]
pub fn is_recurring_entry(location: &EntryLocation) -> bool {
    match location {
        EntryLocation::Projected(entry) => matches!(entry.source_type, SourceType::Recurring),
        EntryLocation::Daily { .. } | EntryLocation::Filter { .. } => false,
    }
}
