use std::io;

use crate::registry::{COMMANDS, Command};

use super::{App, CommandPaletteMode, CommandPaletteState, InputMode};

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPaletteState {
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_mode(super::CommandPaletteMode::Commands)
    }

    #[must_use]
    pub fn new_with_mode(mode: CommandPaletteMode) -> Self {
        Self { mode, selected: 0 }
    }

    #[must_use]
    pub fn filtered_commands(&self) -> Vec<&'static Command> {
        if self.mode != CommandPaletteMode::Commands {
            return Vec::new();
        }
        COMMANDS.iter().collect()
    }

    pub fn reset_selection(&mut self) {
        self.selected = 0;
    }

    pub fn select_prev_tab(&mut self) {
        self.mode = match self.mode {
            CommandPaletteMode::Commands => CommandPaletteMode::Tags,
            CommandPaletteMode::Projects => CommandPaletteMode::Commands,
            CommandPaletteMode::Tags => CommandPaletteMode::Projects,
        };
        self.reset_selection();
    }

    pub fn select_next_tab(&mut self) {
        self.mode = match self.mode {
            CommandPaletteMode::Commands => CommandPaletteMode::Projects,
            CommandPaletteMode::Projects => CommandPaletteMode::Tags,
            CommandPaletteMode::Tags => CommandPaletteMode::Commands,
        };
        self.reset_selection();
    }

    pub fn select_next(&mut self, count: usize) {
        if count == 0 {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected + 1).min(count - 1);
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn selected_command(&self) -> Option<&'static Command> {
        let commands = self.filtered_commands();
        commands.get(self.selected).copied()
    }
}

impl App {
    pub fn open_command_palette(&mut self) {
        self.open_palette(super::CommandPaletteMode::Commands);
    }

    pub fn open_palette(&mut self, mode: super::CommandPaletteMode) {
        self.input_mode = InputMode::CommandPalette(CommandPaletteState::new_with_mode(mode));
    }

    pub fn close_command_palette(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub fn command_palette_prev_tab(&mut self) {
        if let InputMode::CommandPalette(state) = &mut self.input_mode {
            state.select_prev_tab();
        }
    }

    pub fn command_palette_next_tab(&mut self) {
        if let InputMode::CommandPalette(state) = &mut self.input_mode {
            state.select_next_tab();
        }
    }

    pub fn command_palette_select_next(&mut self) {
        if let InputMode::CommandPalette(state) = &mut self.input_mode {
            let count = state.filtered_commands().len();
            state.select_next(count);
        }
    }

    pub fn command_palette_select_prev(&mut self) {
        if let InputMode::CommandPalette(state) = &mut self.input_mode {
            state.select_prev();
        }
    }

    pub fn execute_selected_command(&mut self) -> io::Result<()> {
        let command = match &self.input_mode {
            InputMode::CommandPalette(state) => state.selected_command(),
            _ => None,
        };
        if let Some(command) = command {
            self.execute_command(command)?;
        }
        Ok(())
    }
}
