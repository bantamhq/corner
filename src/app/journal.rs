use std::io;

use chrono::Local;

use crate::config::{Config, resolve_path};
use crate::dispatch::Keymap;
use crate::storage::{JournalSlot, ProjectRegistry};

use super::{App, ConfirmContext, InputMode};

impl App {
    fn reset_journal_view(&mut self) -> io::Result<()> {
        self.reset_daily_view(Local::now().date_naive())?;
        // Trigger calendar refresh for the new journal context
        self.trigger_calendar_fetch();
        Ok(())
    }

    fn apply_config(&mut self, config: Config) {
        self.keymap = Keymap::new(&config.keys).unwrap_or_default();
        self.hide_completed = config.hide_completed;
        self.config = config;
    }

    pub fn open_journal(&mut self, path: &str) -> io::Result<()> {
        self.save();

        let path = resolve_path(path);
        self.journal_context.set_project_path(path.clone());
        self.journal_context.set_active_slot(JournalSlot::Project);
        self.reset_journal_view()?;
        self.set_status(format!("Opened: {}", path.display()));
        Ok(())
    }

    fn switch_to_journal(&mut self, slot: JournalSlot) -> io::Result<()> {
        if self.active_journal() == slot {
            return Ok(());
        }
        self.save();
        self.journal_context.set_active_slot(slot);

        let config = match slot {
            JournalSlot::Hub => Config::load_hub()?,
            JournalSlot::Project => Config::load_merged()?,
        };
        self.apply_config(config);

        self.reset_journal_view()?;
        self.set_status(match slot {
            JournalSlot::Hub => "Switched to Hub journal",
            JournalSlot::Project => "Switched to Project journal",
        });
        Ok(())
    }

    pub fn switch_to_hub(&mut self) -> io::Result<()> {
        self.switch_to_journal(JournalSlot::Hub)
    }

    pub fn switch_to_project(&mut self) -> io::Result<()> {
        self.switch_to_journal(JournalSlot::Project)
    }

    pub fn toggle_journal(&mut self) -> io::Result<()> {
        match self.active_journal() {
            JournalSlot::Hub => {
                if self.journal_context.project_path().is_some() {
                    self.switch_to_project()?;
                } else {
                    self.input_mode = InputMode::Confirm(ConfirmContext::CreateProjectJournal);
                }
            }
            JournalSlot::Project => {
                self.switch_to_hub()?;
            }
        }
        Ok(())
    }

    pub fn switch_to_registered_project(&mut self, id: &str) -> io::Result<()> {
        let registry = ProjectRegistry::load();
        let project = registry.find_by_id(id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("Project '{}' not found in registry", id),
            )
        })?;

        if !project.available {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Project '{}' is unavailable (path missing)", id),
            ));
        }

        self.save();
        self.journal_context
            .set_project_path(project.journal_path());
        self.journal_context.set_active_slot(JournalSlot::Project);

        let config = Config::load_merged_from(&project.root)?;
        self.apply_config(config);

        self.reset_journal_view()?;
        self.set_status(format!("Switched to: {}", project.name));
        Ok(())
    }

    #[must_use]
    pub fn current_project_id(&self) -> Option<String> {
        let path = self.journal_context.project_path()?;
        let registry = ProjectRegistry::load();
        registry.find_by_path(path).map(|p| p.id.clone())
    }
}
