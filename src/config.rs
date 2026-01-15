use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::storage::find_git_root;

const VALID_TIDY_TYPES: &[&str] = &["completed", "uncompleted", "notes", "events"];

/// Configuration for a single calendar source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConfig {
    /// ICS URL to fetch calendar data from
    pub url: String,
    /// Whether this calendar is enabled (defaults to true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Optional color override (ANSI color name)
    #[serde(default, skip_serializing, deserialize_with = "deserialize_color")]
    pub color: Option<Color>,
}

fn deserialize_color<'de, D>(deserializer: D) -> Result<Option<Color>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) => parse_ansi_color(&s)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid color: {s}"))),
    }
}

/// Parse an ANSI color name (case-insensitive).
#[must_use]
pub fn parse_ansi_color(s: &str) -> Option<Color> {
    match s.to_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" => Some(Color::DarkGray),
        "lightred" => Some(Color::LightRed),
        "lightgreen" => Some(Color::LightGreen),
        "lightyellow" => Some(Color::LightYellow),
        "lightblue" => Some(Color::LightBlue),
        "lightmagenta" => Some(Color::LightMagenta),
        "lightcyan" => Some(Color::LightCyan),
        "white" => Some(Color::White),
        _ => None,
    }
}

fn default_true() -> bool {
    true
}

/// Calendar visibility mode for projects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CalendarVisibilityMode {
    /// Show all enabled calendars by default
    #[default]
    All,
    /// Show no calendars by default (must be explicitly enabled per-project)
    None,
}

/// Default sidebar to show on launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SidebarDefault {
    /// No sidebar on launch
    None,
    /// Agenda sidebar
    Agenda,
    /// Calendar sidebar (default)
    #[default]
    Calendar,
}

/// Global calendar visibility settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalendarVisibilityConfig {
    /// Default visibility mode for projects
    #[serde(default)]
    pub default_mode: CalendarVisibilityMode,
    /// Whether to display cancelled events (with strikethrough)
    #[serde(default)]
    pub display_cancelled: bool,
    /// Whether to display declined events (with strikethrough)
    #[serde(default)]
    pub display_declined: bool,
}

fn default_tidy_order() -> Vec<String> {
    vec![
        "completed".to_string(),
        "events".to_string(),
        "notes".to_string(),
        "uncompleted".to_string(),
    ]
}

fn default_favorite_tags() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("1".to_string(), "feature".to_string());
    m.insert("2".to_string(), "bug".to_string());
    m.insert("3".to_string(), "idea".to_string());
    m
}

fn default_default_filter() -> String {
    "!tasks".to_string()
}

fn default_header_date_format() -> String {
    "%A, %b %-d".to_string()
}

fn default_scratchpad_file() -> PathBuf {
    get_config_dir().join("scratchpad.md")
}

fn default_auto_init_project() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub hub_file: Option<String>,
    #[serde(default)]
    pub journal_file: Option<String>,
    #[serde(default)]
    pub scratchpad_file: Option<String>,
    #[serde(default = "default_tidy_order")]
    pub tidy_order: Vec<String>,
    #[serde(default = "default_favorite_tags")]
    pub favorite_tags: HashMap<String, String>,
    #[serde(default)]
    pub filters: HashMap<String, String>,
    #[serde(default = "default_default_filter")]
    pub default_filter: String,
    #[serde(default = "default_header_date_format")]
    pub header_date_format: String,
    #[serde(default)]
    pub hide_completed: bool,
    #[serde(default)]
    pub keys: HashMap<String, HashMap<String, String>>,
    #[serde(default = "default_auto_init_project")]
    pub auto_init_project: bool,
    /// Calendar sources (only loaded from base config for security)
    #[serde(default)]
    pub calendars: HashMap<String, CalendarConfig>,
    /// Calendar visibility settings
    #[serde(default)]
    pub calendar_visibility: CalendarVisibilityConfig,
    /// Default sidebar to show on launch
    #[serde(default)]
    pub sidebar_default: SidebarDefault,
}

/// Raw config for deserialization - all fields are Option to distinguish "not set" from "set to default"
#[derive(Debug, Clone, Deserialize, Default)]
struct RawConfig {
    pub hub_file: Option<String>,
    pub journal_file: Option<String>,
    pub scratchpad_file: Option<String>,
    pub tidy_order: Option<Vec<String>>,
    pub favorite_tags: Option<HashMap<String, String>>,
    pub filters: Option<HashMap<String, String>>,
    pub default_filter: Option<String>,
    pub header_date_format: Option<String>,
    pub hide_completed: Option<bool>,
    pub keys: Option<HashMap<String, HashMap<String, String>>>,
    pub auto_init_project: Option<bool>,
    /// Calendar sources (base config only for security)
    pub calendars: Option<HashMap<String, CalendarConfig>>,
    /// Calendar visibility settings
    pub calendar_visibility: Option<CalendarVisibilityConfig>,
    /// Default sidebar to show on launch
    pub sidebar_default: Option<SidebarDefault>,
}

impl RawConfig {
    fn into_config(self) -> Config {
        Config {
            hub_file: self.hub_file,
            journal_file: self.journal_file,
            scratchpad_file: self.scratchpad_file,
            tidy_order: self.tidy_order.unwrap_or_else(default_tidy_order),
            favorite_tags: self.favorite_tags.unwrap_or_else(default_favorite_tags),
            filters: self.filters.unwrap_or_default(),
            default_filter: self.default_filter.unwrap_or_else(default_default_filter),
            header_date_format: self
                .header_date_format
                .unwrap_or_else(default_header_date_format),
            hide_completed: self.hide_completed.unwrap_or(false),
            keys: self.keys.unwrap_or_default(),
            auto_init_project: self
                .auto_init_project
                .unwrap_or_else(default_auto_init_project),
            calendars: self.calendars.unwrap_or_default(),
            calendar_visibility: self.calendar_visibility.unwrap_or_default(),
            sidebar_default: self.sidebar_default.unwrap_or_default(),
        }
    }

    /// Merge project config over base config.
    /// Some fields are context-specific and don't merge:
    /// - hub_file: base only (hub-specific)
    /// - journal_file: overlay only (project-specific)
    /// - auto_init_project: base only (global setting)
    /// - calendars: base only (security - URLs shouldn't be in repos)
    /// - calendar_visibility: base only (global setting)
    fn merge_over(self, base: RawConfig) -> RawConfig {
        RawConfig {
            hub_file: base.hub_file,
            journal_file: self.journal_file,
            scratchpad_file: self.scratchpad_file.or(base.scratchpad_file),
            tidy_order: self.tidy_order.or(base.tidy_order),
            default_filter: self.default_filter.or(base.default_filter),
            header_date_format: self.header_date_format.or(base.header_date_format),
            hide_completed: self.hide_completed.or(base.hide_completed),
            favorite_tags: Some(merge_hashmaps(base.favorite_tags, self.favorite_tags)),
            filters: Some(merge_hashmaps(base.filters, self.filters)),
            keys: Some(merge_keys(base.keys, self.keys)),
            auto_init_project: base.auto_init_project,
            calendars: base.calendars,
            calendar_visibility: base.calendar_visibility,
            sidebar_default: base.sidebar_default,
        }
    }
}

fn merge_hashmaps(
    base: Option<HashMap<String, String>>,
    overlay: Option<HashMap<String, String>>,
) -> HashMap<String, String> {
    match (base, overlay) {
        (Some(mut b), Some(o)) => {
            b.extend(o);
            b
        }
        (Some(b), None) => b,
        (None, Some(o)) => o,
        (None, None) => HashMap::new(),
    }
}

fn merge_keys(
    base: Option<HashMap<String, HashMap<String, String>>>,
    overlay: Option<HashMap<String, HashMap<String, String>>>,
) -> HashMap<String, HashMap<String, String>> {
    match (base, overlay) {
        (Some(mut b), Some(o)) => {
            for (context, keys) in o {
                b.entry(context).or_default().extend(keys);
            }
            b
        }
        (Some(b), None) => b,
        (None, Some(o)) => o,
        (None, None) => HashMap::new(),
    }
}

impl Config {
    #[must_use]
    pub fn validated_tidy_order(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let result: Vec<String> = self
            .tidy_order
            .iter()
            .filter(|s| VALID_TIDY_TYPES.contains(&s.as_str()) && seen.insert(s.as_str()))
            .cloned()
            .collect();

        if result.is_empty() {
            default_tidy_order()
        } else {
            result
        }
    }

    /// Get favorite tag by number key (0-9)
    #[must_use]
    pub fn get_favorite_tag(&self, key: char) -> Option<&str> {
        if !key.is_ascii_digit() {
            return None;
        }
        self.favorite_tags
            .get(&key.to_string())
            .map(String::as_str)
            .filter(|s| !s.is_empty())
    }

    /// Get all enabled calendar IDs.
    #[must_use]
    pub fn enabled_calendar_ids(&self) -> Vec<String> {
        self.calendars
            .iter()
            .filter(|(_, cfg)| cfg.enabled)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get a calendar config by ID.
    #[must_use]
    pub fn get_calendar(&self, id: &str) -> Option<&CalendarConfig> {
        self.calendars.get(id)
    }

    /// Get the color for a calendar, using explicit color or cycling through defaults.
    #[must_use]
    pub fn calendar_color(&self, id: &str) -> Color {
        use crate::ui::theme::CALENDAR_COLORS;

        if let Some(cfg) = self.calendars.get(id)
            && let Some(color) = cfg.color
        {
            return color;
        }

        // Sort calendar IDs for deterministic ordering
        let mut sorted_ids: Vec<_> = self.calendars.keys().collect();
        sorted_ids.sort();

        let index = sorted_ids.iter().position(|&k| k == id).unwrap_or(0);
        CALENDAR_COLORS[index % CALENDAR_COLORS.len()]
    }

    /// Check if any calendars are configured.
    #[must_use]
    pub fn has_calendars(&self) -> bool {
        !self.calendars.is_empty()
    }

    /// Load hub config (base + optional hub_config.toml overlay)
    pub fn load_hub() -> io::Result<Self> {
        let base = load_raw_config(get_config_path())?;

        if let Some(hub) = load_hub_config() {
            Ok(hub.merge_over(base).into_config())
        } else {
            Ok(base.into_config())
        }
    }

    /// Load project config (base + project config overlay)
    pub fn load_merged() -> io::Result<Self> {
        let base = load_raw_config(get_config_path())?;

        if let Some(project) = load_project_config() {
            Ok(project.merge_over(base).into_config())
        } else {
            Ok(base.into_config())
        }
    }

    /// Load project config from a specific project root path
    pub fn load_merged_from(project_root: &Path) -> io::Result<Self> {
        let base = load_raw_config(get_config_path())?;
        let project_config_path = project_root.join(".caliber").join("config.toml");

        if project_config_path.exists() {
            let project = load_raw_config(project_config_path)?;
            Ok(project.merge_over(base).into_config())
        } else {
            Ok(base.into_config())
        }
    }

    pub fn init() -> io::Result<bool> {
        let path = get_config_path();
        if path.exists() {
            return Ok(false);
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&path, "")?;
        Ok(true)
    }

    pub fn get_hub_journal_path(&self) -> PathBuf {
        if let Some(ref file) = self.hub_file {
            resolve_path(file)
        } else {
            get_default_journal_path()
        }
    }

    pub fn get_scratchpad_path(&self) -> PathBuf {
        if let Some(ref file) = self.scratchpad_file {
            expand_tilde(file)
        } else {
            default_scratchpad_file()
        }
    }

    /// Get project journal path, defaulting to .caliber/journal.md if not configured.
    #[must_use]
    pub fn get_project_journal_path(&self, project_root: &Path) -> PathBuf {
        if let Some(ref file) = self.journal_file {
            expand_tilde(file)
        } else {
            project_root.join(".caliber").join("journal.md")
        }
    }
}

/// Resolve a path to absolute, joining with cwd if relative.
#[must_use]
pub fn resolve_path(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    }
}

/// Expand ~ to home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(stripped)
    } else if path == "~" {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    } else {
        resolve_path(path)
    }
}

pub fn get_config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("caliber")
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
            .join("caliber")
    }
}

pub fn get_config_path() -> PathBuf {
    get_config_dir().join("config.toml")
}

pub fn get_hub_config_path() -> PathBuf {
    get_config_dir().join("hub_config.toml")
}

pub fn get_default_journal_path() -> PathBuf {
    get_config_dir().join("hub_journal.md")
}

fn load_raw_config(path: PathBuf) -> io::Result<RawConfig> {
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        toml::from_str(&content).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    } else {
        Ok(RawConfig::default())
    }
}

fn load_project_config() -> Option<RawConfig> {
    if let Some(root) = find_git_root() {
        let path = root.join(".caliber").join("config.toml");
        if path.exists() {
            let content = fs::read_to_string(&path).ok()?;
            return toml::from_str(&content).ok();
        }
        return None;
    }

    let cwd = std::env::current_dir().ok()?;
    let path = cwd.join(".caliber").join("config.toml");
    if path.exists() {
        let content = fs::read_to_string(&path).ok()?;
        return toml::from_str(&content).ok();
    }

    None
}

fn load_hub_config() -> Option<RawConfig> {
    let path = get_hub_config_path();
    if path.exists() {
        let content = fs::read_to_string(&path).ok()?;
        toml::from_str(&content).ok()
    } else {
        None
    }
}
