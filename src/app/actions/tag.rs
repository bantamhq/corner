use std::io;

use crate::app::{App, EntryLocation, ViewMode};
use crate::storage::{self, Line};
use crate::ui::{remove_all_trailing_tags, remove_last_trailing_tag};

use super::types::{Action, ActionDescription, StatusVisibility};

/// Target for tag operations - includes original content for undo
#[derive(Clone)]
pub struct TagTarget {
    pub location: EntryLocation,
    pub original_content: String,
}

/// Action to remove the last trailing tag from entries
pub struct RemoveLastTag {
    targets: Vec<TagTarget>,
}

impl RemoveLastTag {
    #[must_use]
    pub fn new(targets: Vec<TagTarget>) -> Self {
        Self { targets }
    }

    #[must_use]
    pub fn single(location: EntryLocation, original_content: String) -> Self {
        Self::new(vec![TagTarget {
            location,
            original_content,
        }])
    }
}

impl Action for RemoveLastTag {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        for target in &self.targets {
            execute_tag_removal_raw(app, &target.location, remove_last_trailing_tag)?;
        }

        Ok(Box::new(RestoreContent::new(
            self.targets.clone(),
            TagOperation::RemoveLast,
        )))
    }

    fn description(&self) -> ActionDescription {
        let count = self.targets.len();
        if count == 1 {
            ActionDescription {
                past: "Removed tag".to_string(),
                past_reversed: "Restored tag".to_string(),
                visibility: StatusVisibility::Always,
            }
        } else {
            ActionDescription {
                past: format!("Removed tags from {} entries", count),
                past_reversed: format!("Restored tags on {} entries", count),
                visibility: StatusVisibility::Always,
            }
        }
    }
}

/// Action to remove all trailing tags from entries
pub struct RemoveAllTags {
    targets: Vec<TagTarget>,
}

impl RemoveAllTags {
    #[must_use]
    pub fn new(targets: Vec<TagTarget>) -> Self {
        Self { targets }
    }

    #[must_use]
    pub fn single(location: EntryLocation, original_content: String) -> Self {
        Self::new(vec![TagTarget {
            location,
            original_content,
        }])
    }
}

impl Action for RemoveAllTags {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        for target in &self.targets {
            execute_tag_removal_raw(app, &target.location, remove_all_trailing_tags)?;
        }

        Ok(Box::new(RestoreContent::new(
            self.targets.clone(),
            TagOperation::RemoveAll,
        )))
    }

    fn description(&self) -> ActionDescription {
        let count = self.targets.len();
        if count == 1 {
            ActionDescription {
                past: "Removed all tags".to_string(),
                past_reversed: "Restored tags".to_string(),
                visibility: StatusVisibility::Always,
            }
        } else {
            ActionDescription {
                past: format!("Removed all tags from {} entries", count),
                past_reversed: format!("Restored tags on {} entries", count),
                visibility: StatusVisibility::Always,
            }
        }
    }
}

/// Action to append a tag to entries
pub struct AppendTag {
    targets: Vec<TagTarget>,
    tag: String,
}

impl AppendTag {
    #[must_use]
    pub fn new(targets: Vec<TagTarget>, tag: String) -> Self {
        Self { targets, tag }
    }

    #[must_use]
    pub fn single(location: EntryLocation, original_content: String, tag: String) -> Self {
        Self::new(
            vec![TagTarget {
                location,
                original_content,
            }],
            tag,
        )
    }
}

impl Action for AppendTag {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        for target in &self.targets {
            execute_append_tag_raw(app, &target.location, &self.tag)?;
        }

        Ok(Box::new(RestoreContent::new(
            self.targets.clone(),
            TagOperation::Append(self.tag.clone()),
        )))
    }

    fn description(&self) -> ActionDescription {
        let count = self.targets.len();
        if count == 1 {
            ActionDescription {
                past: format!("Added #{}", self.tag),
                past_reversed: format!("Removed #{}", self.tag),
                visibility: StatusVisibility::Always,
            }
        } else {
            ActionDescription {
                past: format!("Added #{} to {} entries", self.tag, count),
                past_reversed: format!("Removed #{} from {} entries", self.tag, count),
                visibility: StatusVisibility::Always,
            }
        }
    }
}

/// Which operation was performed (for redo)
#[derive(Clone)]
enum TagOperation {
    RemoveLast,
    RemoveAll,
    Append(String),
}

/// Action to restore original content (reverse of tag operations)
pub struct RestoreContent {
    targets: Vec<TagTarget>,
    operation: TagOperation,
}

impl RestoreContent {
    fn new(targets: Vec<TagTarget>, operation: TagOperation) -> Self {
        Self { targets, operation }
    }
}

impl Action for RestoreContent {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        // Capture current content for redo
        let mut new_targets = Vec::with_capacity(self.targets.len());
        for target in &self.targets {
            let current_content = get_entry_content(app, &target.location)?;
            set_entry_content_raw(app, &target.location, &target.original_content)?;
            new_targets.push(TagTarget {
                location: target.location.clone(),
                original_content: current_content,
            });
        }

        // Return action to redo the original operation
        let redo_action: Box<dyn Action> = match &self.operation {
            TagOperation::RemoveLast => Box::new(RemoveLastTag::new(new_targets)),
            TagOperation::RemoveAll => Box::new(RemoveAllTags::new(new_targets)),
            TagOperation::Append(tag) => Box::new(AppendTag::new(new_targets, tag.clone())),
        };

        Ok(redo_action)
    }

    fn description(&self) -> ActionDescription {
        let count = self.targets.len();
        match &self.operation {
            TagOperation::RemoveLast => {
                if count == 1 {
                    ActionDescription {
                        past: "Restored tag".to_string(),
                        past_reversed: "Removed tag".to_string(),
                        visibility: StatusVisibility::Always,
                    }
                } else {
                    ActionDescription {
                        past: format!("Restored tags on {} entries", count),
                        past_reversed: format!("Removed tags from {} entries", count),
                        visibility: StatusVisibility::Always,
                    }
                }
            }
            TagOperation::RemoveAll => {
                if count == 1 {
                    ActionDescription {
                        past: "Restored tags".to_string(),
                        past_reversed: "Removed all tags".to_string(),
                        visibility: StatusVisibility::Always,
                    }
                } else {
                    ActionDescription {
                        past: format!("Restored tags on {} entries", count),
                        past_reversed: format!("Removed all tags from {} entries", count),
                        visibility: StatusVisibility::Always,
                    }
                }
            }
            TagOperation::Append(tag) => {
                if count == 1 {
                    ActionDescription {
                        past: format!("Removed #{}", tag),
                        past_reversed: format!("Added #{}", tag),
                        visibility: StatusVisibility::Always,
                    }
                } else {
                    ActionDescription {
                        past: format!("Removed #{} from {} entries", tag, count),
                        past_reversed: format!("Added #{} to {} entries", tag, count),
                        visibility: StatusVisibility::Always,
                    }
                }
            }
        }
    }
}

/// Get entry content at a location
fn get_entry_content(app: &App, location: &EntryLocation) -> io::Result<String> {
    match location {
        EntryLocation::Later {
            source_date,
            line_index,
        } => {
            let lines = storage::load_day_lines(*source_date, app.active_path())?;
            if let Some(Line::Entry(entry)) = lines.get(*line_index) {
                Ok(entry.content.clone())
            } else {
                Ok(String::new())
            }
        }
        EntryLocation::Daily { line_idx } => {
            if let Line::Entry(entry) = &app.lines[*line_idx] {
                Ok(entry.content.clone())
            } else {
                Ok(String::new())
            }
        }
        EntryLocation::Filter {
            source_date,
            line_index,
            ..
        } => {
            let lines = storage::load_day_lines(*source_date, app.active_path())?;
            if let Some(Line::Entry(entry)) = lines.get(*line_index) {
                Ok(entry.content.clone())
            } else {
                Ok(String::new())
            }
        }
    }
}

/// Execute tag removal on a single target
fn execute_tag_removal_raw<F>(app: &mut App, location: &EntryLocation, remover: F) -> io::Result<()>
where
    F: Fn(&str) -> Option<String>,
{
    let path = app.active_path().to_path_buf();

    match location {
        EntryLocation::Later {
            source_date,
            line_index,
        } => {
            let changed = storage::mutate_entry(*source_date, &path, *line_index, |entry| {
                if let Some(new_content) = remover(&entry.content) {
                    entry.content = new_content;
                    true
                } else {
                    false
                }
            })?;

            if changed == Some(true) {
                if let ViewMode::Daily(state) = &mut app.view {
                    state.later_entries =
                        storage::collect_later_entries_for_date(app.current_date, &path)?;
                }
                app.refresh_tag_cache();
            }
        }
        EntryLocation::Daily { line_idx } => {
            if let Line::Entry(entry) = &mut app.lines[*line_idx]
                && let Some(new_content) = remover(&entry.content) {
                    entry.content = new_content;
                    app.save();
                    app.refresh_tag_cache();
                }
        }
        EntryLocation::Filter {
            index,
            source_date,
            line_index,
        } => {
            let changed = storage::mutate_entry(*source_date, &path, *line_index, |entry| {
                if let Some(new_content) = remover(&entry.content) {
                    entry.content = new_content;
                    true
                } else {
                    false
                }
            })?;

            if changed == Some(true) {
                if let ViewMode::Filter(state) = &mut app.view
                    && let Some(filter_entry) = state.entries.get_mut(*index)
                        && let Some(new_content) = remover(&filter_entry.content) {
                            filter_entry.content = new_content;
                        }

                if *source_date == app.current_date {
                    app.reload_current_day()?;
                }
                app.refresh_tag_cache();
            }
        }
    }
    Ok(())
}

/// Execute tag append on a single target
fn execute_append_tag_raw(app: &mut App, location: &EntryLocation, tag: &str) -> io::Result<()> {
    let path = app.active_path().to_path_buf();
    let tag_with_hash = format!(" #{tag}");

    match location {
        EntryLocation::Later {
            source_date,
            line_index,
        } => {
            storage::mutate_entry(*source_date, &path, *line_index, |entry| {
                entry.content.push_str(&tag_with_hash);
            })?;

            if let ViewMode::Daily(state) = &mut app.view {
                state.later_entries =
                    storage::collect_later_entries_for_date(app.current_date, &path)?;
            }
            app.refresh_tag_cache();
        }
        EntryLocation::Daily { line_idx } => {
            if let Line::Entry(entry) = &mut app.lines[*line_idx] {
                entry.content.push_str(&tag_with_hash);
                app.save();
                app.refresh_tag_cache();
            }
        }
        EntryLocation::Filter {
            index,
            source_date,
            line_index,
        } => {
            storage::mutate_entry(*source_date, &path, *line_index, |entry| {
                entry.content.push_str(&tag_with_hash);
            })?;

            if let ViewMode::Filter(state) = &mut app.view
                && let Some(filter_entry) = state.entries.get_mut(*index) {
                    filter_entry.content.push_str(&tag_with_hash);
                }

            if *source_date == app.current_date {
                app.reload_current_day()?;
            }
            app.refresh_tag_cache();
        }
    }
    Ok(())
}

/// Set entry content directly (for undo/restore)
fn set_entry_content_raw(app: &mut App, location: &EntryLocation, content: &str) -> io::Result<()> {
    let path = app.active_path().to_path_buf();

    match location {
        EntryLocation::Later {
            source_date,
            line_index,
        } => {
            storage::mutate_entry(*source_date, &path, *line_index, |entry| {
                entry.content = content.to_string();
            })?;

            if let ViewMode::Daily(state) = &mut app.view {
                state.later_entries =
                    storage::collect_later_entries_for_date(app.current_date, &path)?;
            }
            app.refresh_tag_cache();
        }
        EntryLocation::Daily { line_idx } => {
            if let Line::Entry(entry) = &mut app.lines[*line_idx] {
                entry.content = content.to_string();
                app.save();
                app.refresh_tag_cache();
            }
        }
        EntryLocation::Filter {
            index,
            source_date,
            line_index,
        } => {
            storage::mutate_entry(*source_date, &path, *line_index, |entry| {
                entry.content = content.to_string();
            })?;

            if let ViewMode::Filter(state) = &mut app.view
                && let Some(filter_entry) = state.entries.get_mut(*index) {
                    filter_entry.content = content.to_string();
                }

            if *source_date == app.current_date {
                app.reload_current_day()?;
            }
            app.refresh_tag_cache();
        }
    }
    Ok(())
}
