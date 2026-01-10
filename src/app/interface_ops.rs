use std::io;

use super::{App, InputMode, InterfaceContext, LastInteraction, ProjectInterfaceState, TagInterfaceState};

/// Visible height for list interfaces (must match PopupLayout content area)
pub const LIST_VISIBLE_HEIGHT: usize = 8;

/// Common behavior for list-based interfaces (Project, Tag)
trait ListInterface {
    fn list_len(&self) -> usize;
    fn selected(&self) -> usize;
    fn set_selected(&mut self, idx: usize);
    fn scroll_offset(&self) -> usize;
    fn set_scroll_offset(&mut self, offset: usize);
}

impl ListInterface for ProjectInterfaceState {
    fn list_len(&self) -> usize {
        self.projects.len()
    }
    fn selected(&self) -> usize {
        self.selected
    }
    fn set_selected(&mut self, idx: usize) {
        self.selected = idx;
    }
    fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }
    fn set_scroll_offset(&mut self, offset: usize) {
        self.scroll_offset = offset;
    }
}

impl ListInterface for TagInterfaceState {
    fn list_len(&self) -> usize {
        self.tags.len()
    }
    fn selected(&self) -> usize {
        self.selected
    }
    fn set_selected(&mut self, idx: usize) {
        self.selected = idx;
    }
    fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }
    fn set_scroll_offset(&mut self, offset: usize) {
        self.scroll_offset = offset;
    }
}

/// Move selection up in a list interface, adjusting scroll if needed
fn list_move_up(state: &mut impl ListInterface) {
    if state.selected() > 0 {
        state.set_selected(state.selected() - 1);
        if state.selected() < state.scroll_offset() {
            state.set_scroll_offset(state.selected());
        }
    }
}

/// Move selection down in a list interface, adjusting scroll if needed
fn list_move_down(state: &mut impl ListInterface) {
    if state.selected() + 1 < state.list_len() {
        state.set_selected(state.selected() + 1);
        if state.selected() >= state.scroll_offset() + LIST_VISIBLE_HEIGHT {
            state.set_scroll_offset(state.selected() - LIST_VISIBLE_HEIGHT + 1);
        }
    }
}

impl App {
    /// Unified move up for all interfaces
    pub fn interface_move_up(&mut self) {
        match &mut self.input_mode {
            InputMode::Interface(InterfaceContext::Date(_)) => {
                self.date_interface_move(0, -1);
            }
            InputMode::Interface(InterfaceContext::Project(state)) => {
                list_move_up(state);
            }
            InputMode::Interface(InterfaceContext::Tag(state)) => {
                list_move_up(state);
            }
            _ => {}
        }
    }

    /// Unified move down for all interfaces
    pub fn interface_move_down(&mut self) {
        match &mut self.input_mode {
            InputMode::Interface(InterfaceContext::Date(_)) => {
                self.date_interface_move(0, 1);
            }
            InputMode::Interface(InterfaceContext::Project(state)) => {
                list_move_down(state);
            }
            InputMode::Interface(InterfaceContext::Tag(state)) => {
                list_move_down(state);
            }
            _ => {}
        }
    }

    /// Unified move left (date interface only)
    pub fn interface_move_left(&mut self) {
        if matches!(
            self.input_mode,
            InputMode::Interface(InterfaceContext::Date(_))
        ) {
            self.date_interface_move(-1, 0);
        }
    }

    /// Unified move right (date interface only)
    pub fn interface_move_right(&mut self) {
        if matches!(
            self.input_mode,
            InputMode::Interface(InterfaceContext::Date(_))
        ) {
            self.date_interface_move(1, 0);
        }
    }

    /// Unified submit/select action (context-aware)
    pub fn interface_submit(&mut self) -> io::Result<()> {
        match &self.input_mode {
            InputMode::Interface(InterfaceContext::Date(state)) => {
                match state.last_interaction {
                    LastInteraction::Typed if !state.query.is_empty() => {
                        self.date_interface_submit_input()?;
                    }
                    _ => {
                        self.confirm_date_interface()?;
                    }
                }
            }
            InputMode::Interface(InterfaceContext::Project(_)) => {
                self.project_interface_select()?;
            }
            InputMode::Interface(InterfaceContext::Tag(_)) => {
                self.tag_interface_select()?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Delete/remove action (project/tag only)
    pub fn interface_delete(&mut self) {
        match &mut self.input_mode {
            InputMode::Interface(InterfaceContext::Project(state)) => {
                if let Some(id) = state.remove_selected() {
                    self.set_status(format!("Removed {id} from registry"));
                }
            }
            InputMode::Interface(InterfaceContext::Tag(_)) => {
                self.tag_interface_delete();
            }
            _ => {}
        }
    }

    /// Rename action (tag only)
    pub fn interface_rename(&mut self) {
        if matches!(
            self.input_mode,
            InputMode::Interface(InterfaceContext::Tag(_))
        ) {
            self.tag_interface_rename();
        }
    }

    /// Hide action (project only)
    pub fn interface_hide(&mut self) {
        if let InputMode::Interface(InterfaceContext::Project(ref mut state)) = self.input_mode
            && let Some(id) = state.hide_selected()
        {
            self.set_status(format!("Hidden {id} from registry"));
        }
    }
}
