use std::io;

use super::super::App;
use super::types::{Action, ActionDescription, StatusVisibility};

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
            StatusVisibility::OnUndo | StatusVisibility::Always => Some(original_desc.past_reversed),
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
            StatusVisibility::OnUndo | StatusVisibility::Always => Some(original_desc.past_reversed),
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
