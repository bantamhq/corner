use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use crossterm::{
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};

use super::{App, InputMode};
use crate::config;
use crate::storage::ProjectRegistry;

impl App {
    pub fn execute_command(&mut self) -> io::Result<()> {
        let cmd = self.command_buffer.content().to_string();
        self.command_buffer.clear();
        let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
        let command = parts.first().copied().unwrap_or("");
        let arg = parts.get(1).copied().unwrap_or("").trim();

        match command {
            "quit" => {
                self.save();
                self.should_quit = true;
            }
            "config" => {
                self.handle_config_command(arg)?;
            }
            "journal" => {
                self.handle_journal_command(arg)?;
            }
            "scratchpad" => {
                self.handle_scratchpad_command()?;
            }
            "project" => {
                self.handle_project_command(arg)?;
            }
            _ => {
                if !command.is_empty() {
                    self.set_status(format!("Unknown command: {command}"));
                }
            }
        }
        self.input_mode = InputMode::Normal;
        Ok(())
    }

    fn handle_config_command(&mut self, arg: &str) -> io::Result<()> {
        let scope = if arg.is_empty() { None } else { Some(arg) };
        match self.resolve_config_path(scope) {
            Ok(path) => self.open_in_editor(&path, true)?,
            Err(msg) => self.set_status(msg),
        }
        Ok(())
    }

    fn handle_journal_command(&mut self, arg: &str) -> io::Result<()> {
        let scope = if arg.is_empty() { None } else { Some(arg) };
        match self.resolve_journal_path(scope) {
            Ok(path) => self.open_in_editor(&path, false)?,
            Err(msg) => self.set_status(msg),
        }
        Ok(())
    }

    fn handle_scratchpad_command(&mut self) -> io::Result<()> {
        let path = self.config.get_scratchpad_path();
        self.open_in_editor(&path, false)
    }

    fn handle_project_command(&mut self, arg: &str) -> io::Result<()> {
        let parts: Vec<&str> = arg.trim().splitn(2, ' ').collect();
        let subcommand = parts.first().copied().unwrap_or("");
        let subarg = parts.get(1).copied().unwrap_or("").trim();

        match subcommand {
            "" => {
                self.set_status("Usage: :project [init|remove|<id>]");
            }
            "init" => {
                self.handle_project_init()?;
            }
            "remove" => {
                self.handle_project_remove(subarg)?;
            }
            id => {
                self.switch_to_registered_project(id)?;
            }
        }
        Ok(())
    }

    fn handle_project_init(&mut self) -> io::Result<()> {
        self.init_project()
    }

    fn handle_project_remove(&mut self, id: &str) -> io::Result<()> {
        let mut registry = ProjectRegistry::load();

        let target_id = if id.is_empty() {
            let Some(path) = self.journal_context.project_path() else {
                self.set_status("No project to remove. Specify an ID or switch to a project first.");
                return Ok(());
            };
            registry
                .find_by_path(path)
                .map(|p| p.id.clone())
                .unwrap_or_default()
        } else {
            id.to_string()
        };

        if target_id.is_empty() {
            self.set_status("Current project is not registered");
            return Ok(());
        }

        if registry.remove(&target_id) {
            registry.save()?;
            self.set_status(format!("Removed project: {}", target_id));
        } else {
            self.set_status(format!("Project not found: {}", target_id));
        }
        Ok(())
    }

    fn resolve_config_path(&self, scope: Option<&str>) -> Result<PathBuf, String> {
        match scope {
            None => Ok(config::get_config_path()),
            Some("hub") => Ok(config::get_hub_config_path()),
            Some("project") => {
                if let Some(project_path) = self.journal_context.project_path() {
                    let config_path = project_path
                        .parent()
                        .map(|p| p.join("config.toml"))
                        .ok_or_else(|| "Cannot determine project config path".to_string())?;
                    Ok(config_path)
                } else {
                    Err("No project journal active".to_string())
                }
            }
            Some(other) => Err(format!("Unknown scope: {other}. Use 'hub' or 'project'")),
        }
    }

    fn resolve_journal_path(&self, scope: Option<&str>) -> Result<PathBuf, String> {
        match scope {
            None => Ok(self.journal_context.active_path().to_path_buf()),
            Some("hub") => Ok(self.journal_context.hub_path().to_path_buf()),
            Some("project") => self
                .journal_context
                .project_path()
                .map(|p| p.to_path_buf())
                .ok_or_else(|| "No project journal active".to_string()),
            Some(other) => Err(format!("Unknown scope: {other}. Use 'hub' or 'project'")),
        }
    }

    fn open_in_editor(&mut self, path: &std::path::Path, is_config: bool) -> io::Result<()> {
        let editor = std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| "vi".to_string());

        let mut parts = editor.split_whitespace();
        let program = parts.next().unwrap_or("vi");
        let editor_args: Vec<&str> = parts.collect();

        self.save();

        // Suspend TUI
        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;

        let status = Command::new(program).args(&editor_args).arg(path).status();

        // Restore TUI
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Clear(ClearType::All))?;
        io::stdout().flush()?;

        self.needs_redraw = true;

        match status {
            Ok(exit) if exit.success() => {
                if is_config {
                    self.reload_config()?;
                } else {
                    self.reload_current_day()?;
                }
            }
            Ok(_) => {
                self.set_status("Editor exited with error");
            }
            Err(e) => {
                self.set_status(format!("Failed to open editor: {e}"));
            }
        }

        Ok(())
    }
}
