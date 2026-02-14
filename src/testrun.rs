use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn parse_arg(args: &[String]) -> (Option<PathBuf>, Vec<String>) {
    let args = &args[1..];
    let Some(i) = args.iter().position(|a| a == "--testrun") else {
        return (None, args.to_vec());
    };

    let testrun_path = args.get(i + 1).map(PathBuf::from);
    let remaining = args
        .iter()
        .enumerate()
        .filter(|&(j, _)| j != i && j != i + 1)
        .map(|(_, a)| a.clone())
        .collect();
    (testrun_path, remaining)
}

pub fn create_temp_profile(source: &Path) -> io::Result<PathBuf> {
    let temp_dir = std::env::temp_dir().join(format!("corner-testrun-{}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;

    for file in [
        "config.toml",
        "hub_config.toml",
        "hub_journal.md",
        "scratchpad.md",
    ] {
        let src = source.join(file);
        if src.exists() {
            fs::copy(&src, temp_dir.join(file))?;
        }
    }

    for subdir in ["calendars", "project/.corner"] {
        let src_dir = source.join(subdir);
        if src_dir.exists() {
            let dst_dir = temp_dir.join(subdir);
            fs::create_dir_all(&dst_dir)?;
            for entry in fs::read_dir(&src_dir)? {
                let entry = entry?;
                if entry.path().is_file() {
                    fs::copy(entry.path(), dst_dir.join(entry.file_name()))?;
                }
            }
        }
    }

    Ok(temp_dir)
}

pub fn cleanup(temp_dir: PathBuf) {
    let _ = fs::remove_dir_all(temp_dir);
}
