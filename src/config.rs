use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub default_file: Option<String>,
}

impl Config {
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

        let default_config = r#"# Caliber configuration

# Set a custom default journal file path (optional)
# default_file = "/path/to/journal.md"
"#;

        fs::write(&path, default_config)?;
        Ok(true)
    }

    pub fn get_journal_path(&self) -> PathBuf {
        if let Some(ref file) = self.default_file {
            let path = PathBuf::from(file);
            if path.is_absolute() {
                path
            } else {
                std::env::current_dir().unwrap_or_default().join(path)
            }
        } else {
            get_default_journal_path()
        }
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

fn get_default_journal_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("caliber")
        .join("journal.md")
}
