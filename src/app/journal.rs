use std::io;

use chrono::Local;

use crate::config::{Config, resolve_path};
use crate::storage::JournalSlot;

use super::{App, ConfirmContext, DailyState, InputMode, ViewMode};

impl App {
    pub fn open_journal(&mut self, path: &str) -> io::Result<()> {
        self.save();

        let path = resolve_path(path);
        self.journal_context.set_project_path(path.clone());
        self.journal_context.set_active_slot(JournalSlot::Project);
        let later_entries = self.load_day(Local::now().date_naive())?;
        self.view = ViewMode::Daily(DailyState::new(self.entry_indices.len(), later_entries));
        self.refresh_tag_cache();
        self.set_status(format!("Opened: {}", path.display()));
        Ok(())
    }

    fn switch_to_journal(&mut self, slot: JournalSlot) -> io::Result<()> {
        if self.active_journal() == slot {
            return Ok(());
        }
        self.save();
        self.journal_context.set_active_slot(slot);
        let later_entries = self.load_day(Local::now().date_naive())?;
        self.view = ViewMode::Daily(DailyState::new(self.entry_indices.len(), later_entries));
        self.refresh_tag_cache();
        self.set_status(match slot {
            JournalSlot::Global => "Switched to Global journal",
            JournalSlot::Project => "Switched to Project journal",
        });
        Ok(())
    }

    pub fn switch_to_global(&mut self) -> io::Result<()> {
        self.switch_to_journal(JournalSlot::Global)
    }

    pub fn switch_to_project(&mut self) -> io::Result<()> {
        self.switch_to_journal(JournalSlot::Project)
    }

    pub fn toggle_journal(&mut self) -> io::Result<()> {
        match self.active_journal() {
            JournalSlot::Global => {
                if self.journal_context.project_path().is_some() {
                    self.switch_to_project()?;
                } else if self.in_git_repo {
                    self.input_mode = InputMode::Confirm(ConfirmContext::CreateProjectJournal);
                } else {
                    self.set_status("Not in a git repository - no project journal available");
                }
            }
            JournalSlot::Project => {
                self.switch_to_global()?;
            }
        }
        Ok(())
    }

    pub fn reload_config(&mut self) -> io::Result<()> {
        self.config = Config::load()?;
        self.set_status("Config reloaded");
        Ok(())
    }
}
