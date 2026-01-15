use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::get_config_dir;

/// Entry in the registry file - stores path and optional calendar visibility
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegisteredProject {
    pub path: PathBuf,
    /// Calendar IDs visible in this project (None = use default_mode from config)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendars: Option<Vec<String>>,
}

/// Registry file format
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ProjectRegistryFile {
    #[serde(default)]
    pub project: Vec<RegisteredProject>,
}

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub path: PathBuf,
    pub root: PathBuf,
    pub name: String,
    pub id: String,
    pub available: bool,
    pub hide_from_registry: bool,
    /// Calendar IDs visible in this project (None = use default_mode from config)
    pub calendars: Option<Vec<String>>,
}

impl ProjectInfo {
    /// Get journal path, checking config for custom location.
    #[must_use]
    pub fn journal_path(&self) -> PathBuf {
        use crate::config::Config;

        if let Ok(config) = Config::load_merged_from(&self.root) {
            config.get_project_journal_path(&self.root)
        } else {
            self.path.join("journal.md")
        }
    }
}

/// Project registry with resolved project info
#[derive(Debug, Clone, Default)]
pub struct ProjectRegistry {
    pub projects: Vec<ProjectInfo>,
}

impl ProjectRegistry {
    /// Load registry from disk and resolve all project info
    #[must_use]
    pub fn load() -> Self {
        let file = load_registry_file().unwrap_or_default();
        let mut projects = Vec::new();
        let mut seen_ids = Vec::new();

        for reg in file.project {
            let caliber_path = normalize_to_caliber_dir(&reg.path);
            if let Some(mut info) = resolve_project_info(&caliber_path, reg.calendars) {
                let base_id = info.id.clone();
                let mut final_id = base_id.clone();
                let mut counter = 2;
                while seen_ids
                    .iter()
                    .any(|id: &String| id.eq_ignore_ascii_case(&final_id))
                {
                    final_id = format!("{}-{}", base_id, counter);
                    counter += 1;
                }
                info.id = final_id.clone();
                seen_ids.push(final_id);
                projects.push(info);
            }
        }

        Self { projects }
    }

    /// Save registry to disk (persists paths and calendar visibility)
    pub fn save(&self) -> io::Result<()> {
        let file = ProjectRegistryFile {
            project: self
                .projects
                .iter()
                .map(|p| RegisteredProject {
                    path: p.path.clone(),
                    calendars: p.calendars.clone(),
                })
                .collect(),
        };

        let path = get_registry_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(&file)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(&path, content)
    }

    pub fn register(&mut self, path: PathBuf) -> io::Result<ProjectInfo> {
        let caliber_path = normalize_to_caliber_dir(&path);

        if self.find_by_path(&caliber_path).is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Project already registered",
            ));
        }

        let Some(mut info) = resolve_project_info(&caliber_path, None) else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Invalid project path - must be a .caliber/ directory",
            ));
        };

        let unique_id = self.generate_unique_id(&info.id);
        info.id = unique_id;

        self.projects.push(info.clone());
        Ok(info)
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len_before = self.projects.len();
        self.projects.retain(|p| !p.id.eq_ignore_ascii_case(id));
        self.projects.len() < len_before
    }

    /// Find project by ID (case-insensitive)
    #[must_use]
    pub fn find_by_id(&self, id: &str) -> Option<&ProjectInfo> {
        self.projects.iter().find(|p| p.id.eq_ignore_ascii_case(id))
    }

    /// Find project by path (accepts either .caliber/ or .caliber/journal.md)
    #[must_use]
    pub fn find_by_path(&self, path: &Path) -> Option<&ProjectInfo> {
        let caliber_path = normalize_to_caliber_dir(path);
        self.projects.iter().find(|p| p.path == caliber_path)
    }

    /// Generate a unique ID from a base, adding suffix for collisions
    #[must_use]
    pub fn generate_unique_id(&self, base: &str) -> String {
        let base_id = sanitize_id(base);

        if !self.id_exists(&base_id) {
            return base_id;
        }

        for n in 2..=99 {
            let candidate = format!("{}-{}", base_id, n);
            if !self.id_exists(&candidate) {
                return candidate;
            }
        }

        // Extremely unlikely fallback - use timestamp
        format!(
            "{}-{}",
            base_id,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        )
    }

    fn id_exists(&self, id: &str) -> bool {
        self.projects.iter().any(|p| p.id.eq_ignore_ascii_case(id))
    }
}

#[must_use]
pub fn get_registry_path() -> PathBuf {
    get_config_dir().join("projects.toml")
}

fn load_registry_file() -> io::Result<ProjectRegistryFile> {
    let path = get_registry_path();
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        toml::from_str(&content).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    } else {
        Ok(ProjectRegistryFile::default())
    }
}

/// Accepts either /path/.caliber/ or /path/.caliber/journal.md
fn normalize_to_caliber_dir(path: &Path) -> PathBuf {
    if let Some(parent) = path.parent()
        && parent.file_name().and_then(|n| n.to_str()) == Some(".caliber")
    {
        return parent.to_path_buf();
    }
    path.to_path_buf()
}

fn resolve_project_info(
    caliber_path: &Path,
    calendars: Option<Vec<String>>,
) -> Option<ProjectInfo> {
    use crate::config::Config;

    if caliber_path.file_name()?.to_str()? != ".caliber" {
        return None;
    }
    let root = caliber_path.parent()?;

    // Check config for custom journal location
    let journal_path = Config::load_merged_from(root)
        .map(|c| c.get_project_journal_path(root))
        .unwrap_or_else(|_| caliber_path.join("journal.md"));
    let available = journal_path.exists();

    let (name, id) = derive_identity(root);
    let hide_from_registry = load_hide_from_registry(&caliber_path.join("config.toml"));

    Some(ProjectInfo {
        path: caliber_path.to_path_buf(),
        root: root.to_path_buf(),
        name,
        id,
        available,
        hide_from_registry,
        calendars,
    })
}

fn load_hide_from_registry(config_path: &Path) -> bool {
    #[derive(Deserialize)]
    struct ProjectConfig {
        #[serde(default)]
        hide_from_registry: bool,
    }

    fs::read_to_string(config_path)
        .ok()
        .and_then(|content| toml::from_str::<ProjectConfig>(&content).ok())
        .map(|c| c.hide_from_registry)
        .unwrap_or(false)
}

/// Set hide_from_registry in a project's config file, preserving other settings.
pub fn set_hide_from_registry(caliber_path: &Path, hide: bool) -> io::Result<()> {
    let config_path = caliber_path.join("config.toml");

    let mut config: toml::Table = if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        toml::from_str(&content).unwrap_or_default()
    } else {
        toml::Table::new()
    };

    config.insert("hide_from_registry".to_string(), toml::Value::Boolean(hide));

    let content = toml::to_string_pretty(&config)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(&config_path, content)
}

fn derive_identity(root: &Path) -> (String, String) {
    let folder = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");

    let name = capitalize_first(folder);
    let id = sanitize_id(folder);

    (name, id)
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => "Project".to_string(),
    }
}

/// Lowercase, alphanumeric + hyphens, no leading/trailing/consecutive hyphens
fn sanitize_id(s: &str) -> String {
    let id: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();

    let mut result = String::new();
    let mut prev_hyphen = true;
    for c in id.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push(c);
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }

    if result.ends_with('-') {
        result.pop();
    }

    if result.is_empty() {
        "project".to_string()
    } else {
        result
    }
}
