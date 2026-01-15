use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use crossterm::execute;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use crate::config::{Config, get_config_path, get_hub_config_path};
use crate::registry::Command as RegistryCommand;
use crate::storage::{JournalSlot, find_git_root};

use super::App;

impl App {
    pub fn execute_command(&mut self, command: &RegistryCommand) -> io::Result<()> {
        match command.name {
            "quit" => {
                self.should_quit = true;
            }
            "scratchpad" => {
                self.open_in_editor(&self.config.get_scratchpad_path())?;
            }
            "open-config" => {
                self.open_in_editor(&get_config_path())?;
            }
            "open-hub-config" => {
                self.open_in_editor(&get_hub_config_path())?;
            }
            "open-project-config" => {
                if let Some(project_path) = self.journal_context.project_path() {
                    let config_path = project_path
                        .parent()
                        .map(|p| p.join("config.toml"))
                        .unwrap_or_else(|| project_path.with_file_name("config.toml"));
                    self.open_in_editor(&config_path)?;
                } else {
                    self.set_error("No project found");
                }
            }
            "reload-config" => {
                let config = match self.active_journal() {
                    JournalSlot::Hub => Config::load_hub().ok(),
                    JournalSlot::Project => Config::load_merged().ok(),
                };
                if let Some(config) = config {
                    self.apply_config(config);
                    self.set_status("Configuration reloaded");
                } else {
                    self.set_error("Failed to reload configuration");
                }
            }
            "open-journal" => {
                let path = self.journal_context.active_path().to_path_buf();
                self.open_in_editor(&path)?;
            }
            "add-gitignore" => {
                self.add_caliber_to_gitignore();
            }
            _ => {}
        }
        Ok(())
    }

    fn open_in_editor(&mut self, path: &Path) -> io::Result<()> {
        let editor = std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| "vi".to_string());

        let mut parts = editor.split_whitespace();
        let program = parts.next().unwrap_or("vi");
        let editor_args: Vec<&str> = parts.collect();

        self.save();

        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;

        let status = Command::new(program).args(&editor_args).arg(path).status();

        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Clear(ClearType::All))?;
        io::stdout().flush()?;

        self.needs_redraw = true;

        match status {
            Ok(exit) if exit.success() => {}
            Ok(_) => {
                self.set_error("Editor exited with error");
            }
            Err(e) => {
                self.set_error(format!("Failed to open editor: {e}"));
            }
        }

        Ok(())
    }

    fn add_caliber_to_gitignore(&mut self) {
        let Some(git_root) = find_git_root() else {
            self.set_error("Not in a git repository");
            return;
        };

        let gitignore_path = git_root.join(".gitignore");
        let entry = ".caliber";

        if gitignore_path.exists()
            && let Ok(content) = fs::read_to_string(&gitignore_path)
            && content.lines().any(|line| line.trim() == entry)
        {
            self.set_status(".caliber already in .gitignore");
            return;
        }

        // Append to .gitignore
        let mut file = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&gitignore_path)
        {
            Ok(f) => f,
            Err(e) => {
                self.set_error(format!("Failed to open .gitignore: {e}"));
                return;
            }
        };

        let needs_newline = gitignore_path.exists()
            && fs::read_to_string(&gitignore_path)
                .ok()
                .is_some_and(|c| !c.is_empty() && !c.ends_with('\n'));

        let to_write = if needs_newline {
            format!("\n{entry}\n")
        } else {
            format!("{entry}\n")
        };

        if let Err(e) = file.write_all(to_write.as_bytes()) {
            self.set_error(format!("Failed to write .gitignore: {e}"));
            return;
        }

        self.set_status("Added .caliber to .gitignore");
    }
}
