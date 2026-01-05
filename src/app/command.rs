use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};

use super::{App, InputMode};
use crate::config;

impl App {
    pub fn execute_command(&mut self) -> io::Result<()> {
        let cmd = self.command_buffer.content().to_string();
        self.command_buffer.clear();
        let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
        let command = parts.first().copied().unwrap_or("");
        let arg = parts.get(1).copied().unwrap_or("").trim();

        match command {
            "q" | "quit" => {
                self.save();
                self.should_quit = true;
            }
            "d" | "date" => {
                if arg.is_empty() {
                    self.set_status("Usage: :date MM/DD");
                } else if let Some(date) = Self::parse_goto_date(arg) {
                    self.goto_day(date)?;
                } else {
                    self.set_status(format!("Invalid date: {arg}"));
                }
            }
            "o" | "open" => {
                self.handle_open_command(arg)?;
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

    fn handle_open_command(&mut self, arg: &str) -> io::Result<()> {
        let parts: Vec<&str> = arg.split_whitespace().collect();
        let target = parts.first().copied().unwrap_or("");
        let scope = parts.get(1).copied();

        let (path, is_config) = match target {
            "config" => (self.resolve_config_path(scope), true),
            "journal" => (self.resolve_journal_path(scope), false),
            "" => {
                self.set_status("Usage: :open <config|journal> [project|global]");
                return Ok(());
            }
            _ => {
                self.set_status(format!(
                    "Unknown target: {target}. Use 'config' or 'journal'"
                ));
                return Ok(());
            }
        };

        match path {
            Ok(p) => self.open_in_editor(&p, is_config)?,
            Err(msg) => self.set_status(msg),
        }
        Ok(())
    }

    fn resolve_config_path(&self, scope: Option<&str>) -> Result<PathBuf, String> {
        match scope {
            None | Some("global") => Ok(config::get_config_path()),
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
            Some(other) => Err(format!("Unknown scope: {other}. Use 'project' or 'global'")),
        }
    }

    fn resolve_journal_path(&self, scope: Option<&str>) -> Result<PathBuf, String> {
        match scope {
            None => Ok(self.journal_context.active_path().to_path_buf()),
            Some("global") => Ok(self.journal_context.global_path().to_path_buf()),
            Some("project") => self
                .journal_context
                .project_path()
                .map(|p| p.to_path_buf())
                .ok_or_else(|| "No project journal active".to_string()),
            Some(other) => Err(format!("Unknown scope: {other}. Use 'project' or 'global'")),
        }
    }

    fn open_in_editor(&mut self, path: &std::path::Path, is_config: bool) -> io::Result<()> {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

        // Save before opening editor
        self.save();

        // Suspend TUI
        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

        let status = Command::new(&editor).arg(path).status();

        // Restore TUI
        enable_raw_mode()?;
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            EnableMouseCapture,
            Clear(ClearType::All)
        )?;
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
