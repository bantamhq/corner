use std::io;

use crate::storage::{self, expand_favorite_tags, normalize_entry_structure, Line};

use super::{App, EntryLocation, ViewMode};

impl App {
    /// Normalize content with all preprocessing steps:
    /// 1. Expand favorite tags (#0-9 -> configured tags)
    /// 2. Normalize entry structure ([content] [recurring] [#tags])
    ///
    /// Returns (normalized_content, optional_warning).
    #[must_use]
    pub fn normalize_content(&self, content: &str) -> (String, Option<String>) {
        let content = expand_favorite_tags(content, &self.config.favorite_tags);
        let (content, warning) = normalize_entry_structure(&content);
        (content.trim_end().to_string(), warning)
    }

    /// The single entry point for all content modifications.
    /// Normalizes content and persists to the appropriate location.
    pub fn save_entry_content(
        &mut self,
        location: &EntryLocation,
        content: String,
    ) -> io::Result<Option<String>> {
        let (normalized, warning) = self.normalize_content(&content);
        self.persist_entry_content(location, &normalized)?;
        Ok(warning)
    }

    /// Persist content to storage without normalization.
    /// Used internally by save_entry_content and for undo/restore operations
    /// where we want to restore exact content.
    pub fn persist_entry_content(
        &mut self,
        location: &EntryLocation,
        content: &str,
    ) -> io::Result<()> {
        let path = self.active_path().to_path_buf();

        match location {
            EntryLocation::Daily { line_idx } => {
                if let Line::Entry(raw_entry) = &mut self.lines[*line_idx] {
                    raw_entry.content = content.to_string();
                    self.save();
                }
            }
            EntryLocation::Projected(entry) => {
                storage::mutate_entry(entry.source_date, &path, entry.line_index, |raw_entry| {
                    raw_entry.content = content.to_string();
                })?;
                self.refresh_projected_entries();
            }
            EntryLocation::Filter { index, entry } => {
                storage::mutate_entry(entry.source_date, &path, entry.line_index, |raw_entry| {
                    raw_entry.content = content.to_string();
                })?;

                if let ViewMode::Filter(state) = &mut self.view
                    && let Some(filter_entry) = state.entries.get_mut(*index)
                {
                    filter_entry.content = content.to_string();
                }

                if entry.source_date == self.current_date {
                    self.reload_current_day()?;
                }
            }
        }
        Ok(())
    }

    pub fn get_entry_content(&self, location: &EntryLocation) -> io::Result<String> {
        match location {
            EntryLocation::Daily { line_idx } => {
                if let Line::Entry(raw_entry) = &self.lines[*line_idx] {
                    Ok(raw_entry.content.clone())
                } else {
                    Ok(String::new())
                }
            }
            EntryLocation::Projected(entry) | EntryLocation::Filter { entry, .. } => {
                let lines = storage::load_day_lines(entry.source_date, self.active_path())?;
                if let Some(Line::Entry(raw_entry)) = lines.get(entry.line_index) {
                    Ok(raw_entry.content.clone())
                } else {
                    Ok(String::new())
                }
            }
        }
    }
}
