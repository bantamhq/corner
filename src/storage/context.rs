use std::path::PathBuf;

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalSlot {
    Hub,
    Project,
}

pub struct JournalContext {
    hub_path: PathBuf,
    project_path: Option<PathBuf>,
    active: JournalSlot,
}

impl JournalContext {
    #[must_use]
    pub fn new(hub_path: PathBuf, project_path: Option<PathBuf>, active: JournalSlot) -> Self {
        Self {
            hub_path,
            project_path,
            active,
        }
    }

    #[must_use]
    pub fn active_path(&self) -> &std::path::Path {
        match self.active {
            JournalSlot::Hub => &self.hub_path,
            JournalSlot::Project => self.project_path.as_deref().unwrap_or(&self.hub_path),
        }
    }

    #[must_use]
    pub fn active_slot(&self) -> JournalSlot {
        self.active
    }

    pub fn set_active_slot(&mut self, slot: JournalSlot) {
        self.active = slot;
    }

    #[must_use]
    pub fn hub_path(&self) -> &std::path::Path {
        &self.hub_path
    }

    #[must_use]
    pub fn project_path(&self) -> Option<&std::path::Path> {
        self.project_path.as_deref()
    }

    pub fn set_project_path(&mut self, path: PathBuf) {
        self.project_path = Some(path);
    }

    pub fn reset_project_path(&mut self) {
        self.project_path = detect_project_journal();
    }
}

/// Detects if we're in a git repository and returns the project root path.
#[must_use]
pub fn find_git_root() -> Option<PathBuf> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// Detect project journal path, checking config for custom location.
#[must_use]
pub fn detect_project_journal() -> Option<PathBuf> {
    let root = find_git_root().or_else(|| std::env::current_dir().ok())?;
    let caliber_dir = root.join(".caliber");

    if !caliber_dir.exists() {
        return None;
    }

    // Load merged config to get journal_file setting
    let config = Config::load_merged_from(&root).ok()?;
    let journal_path = config.get_project_journal_path(&root);

    if journal_path.exists() {
        Some(journal_path)
    } else {
        None
    }
}
