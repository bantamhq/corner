use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalSlot {
    Global,
    Project,
}

pub struct JournalContext {
    global_path: PathBuf,
    project_path: Option<PathBuf>,
    active: JournalSlot,
}

impl JournalContext {
    #[must_use]
    pub fn new(global_path: PathBuf, project_path: Option<PathBuf>, active: JournalSlot) -> Self {
        Self {
            global_path,
            project_path,
            active,
        }
    }

    #[must_use]
    pub fn active_path(&self) -> &std::path::Path {
        match self.active {
            JournalSlot::Global => &self.global_path,
            JournalSlot::Project => self.project_path.as_deref().unwrap_or(&self.global_path),
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
    pub fn global_path(&self) -> &std::path::Path {
        &self.global_path
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

/// Detects if a project journal exists and returns its path.
/// Returns Some(path) if .caliber/journal.md exists, None otherwise.
#[must_use]
pub fn detect_project_journal() -> Option<PathBuf> {
    // First check for git root
    if let Some(root) = find_git_root() {
        let project_journal = root.join(".caliber").join("journal.md");
        if project_journal.exists() {
            return Some(project_journal);
        }
        return None;
    }

    // Fallback: check current directory for .caliber/
    let cwd = std::env::current_dir().ok()?;
    let project_journal = cwd.join(".caliber").join("journal.md");
    if project_journal.exists() {
        return Some(project_journal);
    }

    None
}

/// Creates a project journal at .caliber/journal.md in the git root.
/// Also creates an empty config.toml for project-specific settings.
pub fn create_project_journal() -> io::Result<PathBuf> {
    let root = find_git_root()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Not in a git repository"))?;

    let caliber_dir = root.join(".caliber");
    fs::create_dir_all(&caliber_dir)?;

    let journal_path = caliber_dir.join("journal.md");
    if !journal_path.exists() {
        fs::write(&journal_path, "")?;
    }

    let config_path = caliber_dir.join("config.toml");
    if !config_path.exists() {
        fs::write(&config_path, "")?;
    }

    Ok(journal_path)
}

/// Adds .caliber/ to .gitignore if not already present.
pub fn add_caliber_to_gitignore() -> io::Result<()> {
    let root = find_git_root()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Not in a git repository"))?;

    let gitignore_path = root.join(".gitignore");
    let entry = ".caliber/";

    if gitignore_path.exists() {
        let content = fs::read_to_string(&gitignore_path)?;
        if content.lines().any(|line| line.trim() == entry) {
            return Ok(());
        }
        let mut new_content = content;
        if !new_content.ends_with('\n') && !new_content.is_empty() {
            new_content.push('\n');
        }
        new_content.push_str(entry);
        new_content.push('\n');
        fs::write(&gitignore_path, new_content)?;
    } else {
        fs::write(&gitignore_path, format!("{entry}\n"))?;
    }

    Ok(())
}
