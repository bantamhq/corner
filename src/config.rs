use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const VALID_SORT_TYPES: &[&str] = &["completed", "uncompleted", "notes", "events"];

fn default_sort_order() -> Vec<String> {
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub default_file: Option<String>,
    #[serde(default = "default_sort_order")]
    pub sort_order: Vec<String>,
    #[serde(default = "default_favorite_tags")]
    pub favorite_tags: HashMap<String, String>,
    #[serde(default)]
    pub filters: HashMap<String, String>,
}

impl Config {
    #[must_use]
    pub fn validated_sort_order(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let result: Vec<String> = self
            .sort_order
            .iter()
            .filter(|s| VALID_SORT_TYPES.contains(&s.as_str()) && seen.insert(s.as_str()))
            .cloned()
            .collect();

        if result.is_empty() {
            default_sort_order()
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

    pub fn load() -> io::Result<Self> {
        let path = get_config_path();
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            toml::from_str(&content).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        } else {
            Ok(Config::default())
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

    pub fn get_journal_path(&self) -> PathBuf {
        if let Some(ref file) = self.default_file {
            resolve_path(file)
        } else {
            get_default_journal_path()
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

pub fn get_config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("caliber")
}

pub fn get_config_path() -> PathBuf {
    get_config_dir().join("config.toml")
}

pub fn get_default_journal_path() -> PathBuf {
    get_config_dir().join("journals").join("journal.md")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_favorite_tag() {
        let mut tags = HashMap::new();
        tags.insert("1".to_string(), "work".to_string());
        tags.insert("2".to_string(), "personal".to_string());

        let config = Config {
            favorite_tags: tags,
            ..Default::default()
        };

        assert_eq!(config.get_favorite_tag('1'), Some("work"));
        assert_eq!(config.get_favorite_tag('2'), Some("personal"));
        assert_eq!(config.get_favorite_tag('3'), None);
        assert_eq!(config.get_favorite_tag('0'), None);
    }

    #[test]
    fn test_get_favorite_tag_empty_string() {
        let mut tags = HashMap::new();
        tags.insert("1".to_string(), "".to_string());

        let config = Config {
            favorite_tags: tags,
            ..Default::default()
        };

        assert_eq!(config.get_favorite_tag('1'), None);
    }

    #[test]
    fn test_get_favorite_tag_zero_key() {
        let mut tags = HashMap::new();
        tags.insert("0".to_string(), "zeroth".to_string());

        let config = Config {
            favorite_tags: tags,
            ..Default::default()
        };

        assert_eq!(config.get_favorite_tag('0'), Some("zeroth"));
    }
}
