use std::io;

use super::super::App;

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
