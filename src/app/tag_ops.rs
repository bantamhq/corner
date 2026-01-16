use std::io;

use regex::Regex;

use crate::storage;

use super::{App, ViewMode};

fn is_valid_tag_boundary(journal: &str, end_pos: usize) -> bool {
    end_pos >= journal.len() || {
        let remaining = &journal[end_pos..];
        remaining
            .chars()
            .next()
            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
    }
}

fn count_tag_occurrences(journal: &str, tag: &str, line_filter: Option<fn(&str) -> bool>) -> usize {
    let tag_lower = tag.to_lowercase();
    let lines: Box<dyn Iterator<Item = &str>> = match line_filter {
        Some(f) => Box::new(journal.lines().filter(move |line| f(line))),
        None => Box::new(std::iter::once(journal)),
    };
    lines
        .flat_map(|text| storage::TAG_REGEX.captures_iter(text))
        .filter(|cap| cap[1].to_lowercase() == tag_lower)
        .count()
}

fn is_completed_task(line: &str) -> bool {
    line.trim_start().starts_with("- [x] ")
}

fn replace_tag_matches(journal: &str, regex: &Regex, replacement: Option<&str>) -> String {
    let mut result = String::with_capacity(journal.len());
    let mut last_end = 0;

    for mat in regex.find_iter(journal) {
        if is_valid_tag_boundary(journal, mat.end()) {
            result.push_str(&journal[last_end..mat.start()]);
            if let Some(rep) = replacement {
                result.push_str(rep);
            }
            last_end = mat.end();
        }
    }
    result.push_str(&journal[last_end..]);
    result
}

impl App {
    pub(super) fn refresh_view_after_tag_change(&mut self) -> io::Result<()> {
        match &self.view {
            ViewMode::Daily(_) => {
                self.reload_current_day()?;
                self.refresh_projected_entries();
                self.clamp_selection_to_visible();
            }
            ViewMode::Filter(_) => {
                self.refresh_filter()?;
            }
        }
        Ok(())
    }

    pub(super) fn delete_all_tag_occurrences(&mut self, tag: &str) -> io::Result<usize> {
        let path = self.active_path().to_path_buf();
        let journal = storage::load_journal(&path)?;
        let count = count_tag_occurrences(&journal, tag, None);

        let tag_regex = storage::create_tag_delete_regex(tag).map_err(io::Error::other)?;
        let new_journal = replace_tag_matches(&journal, &tag_regex, None);
        let cleaned = Self::clean_empty_entries(&new_journal);

        storage::save_journal(&path, &cleaned)?;
        Ok(count)
    }

    pub fn confirm_delete_tag(&mut self, tag: &str) -> io::Result<()> {
        let count = self.delete_all_tag_occurrences(tag)?;
        self.refresh_view_after_tag_change()?;
        self.set_status(format!("Deleted {count} tag occurrences"));
        self.open_palette(super::CommandPaletteMode::Tags);
        Ok(())
    }

    pub(super) fn delete_tag_from_completed(&mut self, tag: &str) -> io::Result<usize> {
        let path = self.active_path().to_path_buf();
        let journal = storage::load_journal(&path)?;
        let count = count_tag_occurrences(&journal, tag, Some(is_completed_task));

        let tag_regex = storage::create_tag_delete_regex(tag).map_err(io::Error::other)?;
        let new_journal = journal
            .lines()
            .map(|line| {
                if is_completed_task(line) {
                    replace_tag_matches(line, &tag_regex, None)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let cleaned = Self::clean_empty_entries(&new_journal);

        storage::save_journal(&path, &cleaned)?;
        Ok(count)
    }

    pub fn confirm_delete_tag_from_completed(&mut self, tag: &str) -> io::Result<()> {
        let count = self.delete_tag_from_completed(tag)?;
        self.refresh_view_after_tag_change()?;
        self.set_status(format!("Removed {count} tag occurrences from completed"));
        self.open_palette(super::CommandPaletteMode::Tags);
        Ok(())
    }

    #[allow(dead_code)]
    pub(super) fn rename_tag_occurrences(
        &mut self,
        old_tag: &str,
        new_tag: &str,
    ) -> io::Result<usize> {
        let path = self.active_path().to_path_buf();
        let journal = storage::load_journal(&path)?;
        let count = count_tag_occurrences(&journal, old_tag, None);

        let tag_regex = storage::create_tag_match_regex(old_tag).map_err(io::Error::other)?;
        let replacement = format!("#{new_tag}");
        let new_journal = replace_tag_matches(&journal, &tag_regex, Some(&replacement));
        let cleaned = Self::clean_empty_entries(&new_journal);

        storage::save_journal(&path, &cleaned)?;
        Ok(count)
    }

    /// Remove entries that became empty after tag operations
    #[must_use]
    fn clean_empty_entries(journal: &str) -> String {
        journal
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                let is_entry = trimmed.starts_with('-') || trimmed.starts_with('*');
                if !is_entry {
                    return true;
                }

                let content = trimmed
                    .find("] ")
                    .map(|pos| &trimmed[pos + 2..])
                    .or_else(|| {
                        // Handle `- [ ]` or `- [x]` with no content after checkbox
                        if trimmed.ends_with(']') && trimmed.contains('[') {
                            Some("")
                        } else {
                            trimmed.find(' ').map(|pos| &trimmed[pos + 1..])
                        }
                    })
                    .unwrap_or("");

                !content.trim().is_empty()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
