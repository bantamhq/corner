use std::io;

use crate::registry::COMMANDS;
use crate::storage::{ProjectInfo, ProjectRegistry, set_hide_from_registry};

use super::{App, CommandPaletteMode, CommandPaletteState, ConfirmContext, InputMode};

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

    pub fn reset_selection(&mut self) {
        self.selected = 0;
    }

    pub fn clamp_selection(&mut self, count: usize) {
        if self.selected >= count && count > 0 {
            self.selected = count - 1;
        }
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
}

impl App {
    /// Returns visible projects sorted with current project first (matching palette display order)
    fn visible_projects_sorted(&self) -> Vec<ProjectInfo> {
        let current_path = self.journal_context.project_path();
        let mut projects: Vec<_> = ProjectRegistry::load()
            .projects
            .into_iter()
            .filter(|p| !p.hide_from_registry)
            .collect();

        projects.sort_by_key(|p| !p.matches_path(current_path));

        projects
    }

    pub fn open_command_palette(&mut self) {
        self.open_palette(super::CommandPaletteMode::Commands);
    }

    pub fn open_palette(&mut self, mode: super::CommandPaletteMode) {
        self.refresh_tag_cache();
        self.input_mode = InputMode::CommandPalette(CommandPaletteState::new_with_mode(mode));
    }

    pub fn toggle_command_palette(&mut self) {
        if matches!(self.input_mode, InputMode::CommandPalette(_)) {
            self.close_command_palette();
        } else {
            self.open_command_palette();
        }
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
        let mode = match &self.input_mode {
            InputMode::CommandPalette(state) => state.mode,
            _ => return,
        };
        let count = self.palette_item_count(mode);
        if let InputMode::CommandPalette(state) = &mut self.input_mode {
            state.select_next(count);
        }
    }

    fn palette_item_count(&self, mode: CommandPaletteMode) -> usize {
        match mode {
            CommandPaletteMode::Commands => COMMANDS.len(),
            CommandPaletteMode::Projects => self.visible_projects_sorted().len(),
            CommandPaletteMode::Tags => self.cached_journal_tags.len(),
        }
    }

    pub fn command_palette_select_prev(&mut self) {
        if let InputMode::CommandPalette(state) = &mut self.input_mode {
            state.select_prev();
        }
    }

    pub fn execute_selected_palette_item(&mut self) -> io::Result<()> {
        let (mode, selected) = match &self.input_mode {
            InputMode::CommandPalette(state) => (state.mode, state.selected),
            _ => return Ok(()),
        };

        match mode {
            CommandPaletteMode::Commands => {
                if let Some(command) = COMMANDS.get(selected) {
                    self.execute_command(command)?;
                }
            }
            CommandPaletteMode::Projects => {
                self.execute_selected_project(selected)?;
            }
            CommandPaletteMode::Tags => {
                self.execute_selected_tag(selected)?;
            }
        }
        Ok(())
    }

    fn execute_selected_project(&mut self, index: usize) -> io::Result<()> {
        let projects = self.visible_projects_sorted();
        if let Some(project) = projects.get(index) {
            let journal_path = project.journal_path();
            self.open_journal(&journal_path.to_string_lossy())?;
        }
        Ok(())
    }

    fn execute_selected_tag(&mut self, index: usize) -> io::Result<()> {
        if let Some(tag) = self.cached_journal_tags.get(index) {
            let query = format!("#{}", tag.name);
            self.quick_filter(&query)?;
        }
        Ok(())
    }

    pub fn palette_delete_selected(&mut self) -> io::Result<()> {
        let (mode, selected) = match &self.input_mode {
            InputMode::CommandPalette(state) => (state.mode, state.selected),
            _ => return Ok(()),
        };

        match mode {
            CommandPaletteMode::Commands => {
                // Commands cannot be deleted
            }
            CommandPaletteMode::Projects => {
                self.palette_delete_project(selected)?;
            }
            CommandPaletteMode::Tags => {
                self.palette_delete_tag(selected);
            }
        }
        Ok(())
    }

    fn palette_delete_project(&mut self, index: usize) -> io::Result<()> {
        let projects = self.visible_projects_sorted();
        let Some(project) = projects.get(index) else {
            return Ok(());
        };

        let mut registry = ProjectRegistry::load();
        registry.remove(&project.id);
        registry.save()?;
        self.set_status(format!("Removed '{}' from registry", project.name));
        let new_count = self.visible_projects_sorted().len();
        if let InputMode::CommandPalette(state) = &mut self.input_mode {
            state.clamp_selection(new_count);
        }
        Ok(())
    }

    fn palette_delete_tag(&mut self, index: usize) {
        self.confirm_tag_action(index, ConfirmContext::DeleteTag);
    }

    pub fn palette_delete_tag_from_completed(&mut self) {
        if let InputMode::CommandPalette(state) = &self.input_mode
            && state.mode == CommandPaletteMode::Tags
        {
            self.confirm_tag_action(state.selected, ConfirmContext::DeleteTagFromCompleted);
        }
    }

    fn confirm_tag_action(&mut self, index: usize, context_fn: fn(String) -> ConfirmContext) {
        if let Some(tag) = self.cached_journal_tags.get(index) {
            let tag_name = tag.name.clone();
            self.close_command_palette();
            self.input_mode = InputMode::Confirm(context_fn(tag_name));
        }
    }

    pub fn palette_hide_selected(&mut self) -> io::Result<()> {
        let (mode, selected) = match &self.input_mode {
            InputMode::CommandPalette(state) => (state.mode, state.selected),
            _ => return Ok(()),
        };

        if mode != CommandPaletteMode::Projects {
            return Ok(());
        }

        let projects = self.visible_projects_sorted();
        let Some(project) = projects.get(selected) else {
            return Ok(());
        };

        if !project.available {
            self.set_error("Cannot hide unavailable projects");
            return Ok(());
        }

        set_hide_from_registry(&project.path, true)?;
        self.set_status(format!("Hidden '{}' from palette", project.name));
        let new_count = self.visible_projects_sorted().len();
        if let InputMode::CommandPalette(state) = &mut self.input_mode {
            state.clamp_selection(new_count);
        }
        Ok(())
    }
}
