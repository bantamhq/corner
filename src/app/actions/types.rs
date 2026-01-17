use std::io;

use super::super::App;
use crate::app::EntryLocation;

#[derive(Clone)]
pub enum StatusVisibility {
    Silent,
    OnUndo,
    Always,
}

#[derive(Clone)]
pub struct ActionDescription {
    pub past: String,
    pub past_reversed: String,
    pub visibility: StatusVisibility,
}

impl ActionDescription {
    #[must_use]
    pub fn always(past: impl Into<String>, past_reversed: impl Into<String>) -> Self {
        Self {
            past: past.into(),
            past_reversed: past_reversed.into(),
            visibility: StatusVisibility::Always,
        }
    }

    #[must_use]
    pub fn on_undo(past: impl Into<String>, past_reversed: impl Into<String>) -> Self {
        Self {
            past: past.into(),
            past_reversed: past_reversed.into(),
            visibility: StatusVisibility::OnUndo,
        }
    }

    #[must_use]
    pub fn silent() -> Self {
        Self {
            past: String::new(),
            past_reversed: String::new(),
            visibility: StatusVisibility::Silent,
        }
    }
}

pub trait Action: Send {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>>;
    fn description(&self) -> ActionDescription;
}

const MAX_UNDO_DEPTH: usize = 50;

pub struct ActionExecutor {
    undo_stack: Vec<(Box<dyn Action>, ActionDescription)>,
    redo_stack: Vec<(Box<dyn Action>, ActionDescription)>,
}

impl Default for ActionExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionExecutor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn execute(
        &mut self,
        mut action: Box<dyn Action>,
        app: &mut App,
    ) -> io::Result<Option<String>> {
        let description = action.description();
        let reverse_action = action.execute(app)?;

        self.undo_stack.push((reverse_action, description.clone()));
        if self.undo_stack.len() > MAX_UNDO_DEPTH {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();

        let message = match description.visibility {
            StatusVisibility::Always => Some(description.past),
            StatusVisibility::OnUndo | StatusVisibility::Silent => None,
        };

        Ok(message)
    }

    pub fn undo(&mut self, app: &mut App) -> io::Result<Option<String>> {
        let Some((mut action, original_desc)) = self.undo_stack.pop() else {
            return Ok(None);
        };

        let new_desc = action.description();
        let reverse_action = action.execute(app)?;
        self.redo_stack.push((reverse_action, new_desc));

        let message = match original_desc.visibility {
            StatusVisibility::Silent => None,
            StatusVisibility::OnUndo | StatusVisibility::Always => {
                Some(original_desc.past_reversed)
            }
        };

        Ok(message)
    }

    pub fn redo(&mut self, app: &mut App) -> io::Result<Option<String>> {
        let Some((mut action, original_desc)) = self.redo_stack.pop() else {
            return Ok(None);
        };

        let new_desc = action.description();
        let reverse_action = action.execute(app)?;
        self.undo_stack.push((reverse_action, new_desc));

        let message = match original_desc.visibility {
            StatusVisibility::Silent => None,
            StatusVisibility::OnUndo | StatusVisibility::Always => {
                Some(original_desc.past_reversed)
            }
        };

        Ok(message)
    }

    pub fn clear_redo(&mut self) {
        self.redo_stack.clear();
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

#[derive(Clone)]
pub struct ContentTarget {
    pub location: EntryLocation,
    pub original_content: String,
}

impl ContentTarget {
    #[must_use]
    pub fn new(location: EntryLocation, original_content: String) -> Self {
        Self {
            location,
            original_content,
        }
    }
}

pub fn get_entry_content(app: &App, location: &EntryLocation) -> io::Result<String> {
    app.get_entry_content(location)
}

/// Bypasses normalization for undo/restore operations.
pub fn set_entry_content(app: &mut App, location: &EntryLocation, content: &str) -> io::Result<()> {
    app.persist_entry_content(location, content)
}

/// Returns Some(new_content) from operation to trigger save with normalization.
pub fn execute_content_operation<F>(
    app: &mut App,
    location: &EntryLocation,
    operation: F,
) -> io::Result<()>
where
    F: Fn(&str) -> Option<String>,
{
    let current_content = app.get_entry_content(location)?;
    if let Some(new_content) = operation(&current_content) {
        app.save_entry_content(location, new_content)?;
    }
    Ok(())
}

pub fn execute_content_append(
    app: &mut App,
    location: &EntryLocation,
    suffix: &str,
) -> io::Result<()> {
    let current_content = app.get_entry_content(location)?;
    let new_content = format!("{current_content}{suffix}");
    app.save_entry_content(location, new_content)?;
    Ok(())
}
