use std::io;

use crate::app::{App, EntryLocation};
use crate::ui::{remove_all_trailing_tags, remove_last_trailing_tag};

use super::content_ops::{
    ContentTarget, execute_content_append, execute_content_operation, get_entry_content,
    set_entry_content,
};
use super::types::{Action, ActionDescription, StatusVisibility};

/// Target for tag operations (type alias for ContentTarget)
pub type TagTarget = ContentTarget;

/// Which tag removal operation to perform
#[derive(Clone, Copy)]
enum RemovalKind {
    Last,
    All,
}

/// Action to remove tags from entries (consolidated from RemoveLastTag and RemoveAllTags)
pub struct TagRemovalAction {
    targets: Vec<TagTarget>,
    kind: RemovalKind,
}

impl TagRemovalAction {
    fn new(targets: Vec<TagTarget>, kind: RemovalKind) -> Self {
        Self { targets, kind }
    }

    fn single(location: EntryLocation, original_content: String, kind: RemovalKind) -> Self {
        Self::new(
            vec![TagTarget {
                location,
                original_content,
            }],
            kind,
        )
    }
}

impl Action for TagRemovalAction {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let op_fn = match self.kind {
            RemovalKind::Last => remove_last_trailing_tag,
            RemovalKind::All => remove_all_trailing_tags,
        };
        for target in &self.targets {
            execute_content_operation(app, &target.location, op_fn)?;
        }

        let operation = match self.kind {
            RemovalKind::Last => TagOperation::RemoveLast,
            RemovalKind::All => TagOperation::RemoveAll,
        };
        Ok(Box::new(RestoreContent::new(
            self.targets.clone(),
            operation,
        )))
    }

    fn description(&self) -> ActionDescription {
        let count = self.targets.len();
        let (past, past_reversed) = match (self.kind, count == 1) {
            (RemovalKind::Last, true) => ("Removed tag".to_string(), "Restored tag".to_string()),
            (RemovalKind::Last, false) => (
                format!("Removed tags from {} entries", count),
                format!("Restored tags on {} entries", count),
            ),
            (RemovalKind::All, true) => {
                ("Removed all tags".to_string(), "Restored tags".to_string())
            }
            (RemovalKind::All, false) => (
                format!("Removed all tags from {} entries", count),
                format!("Restored tags on {} entries", count),
            ),
        };
        ActionDescription {
            past,
            past_reversed,
            visibility: StatusVisibility::Always,
        }
    }
}

pub struct RemoveLastTag(TagRemovalAction);

impl RemoveLastTag {
    #[must_use]
    pub fn new(targets: Vec<TagTarget>) -> Self {
        Self(TagRemovalAction::new(targets, RemovalKind::Last))
    }

    #[must_use]
    pub fn single(location: EntryLocation, original_content: String) -> Self {
        Self(TagRemovalAction::single(
            location,
            original_content,
            RemovalKind::Last,
        ))
    }
}

impl Action for RemoveLastTag {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        self.0.execute(app)
    }

    fn description(&self) -> ActionDescription {
        self.0.description()
    }
}

pub struct RemoveAllTags(TagRemovalAction);

impl RemoveAllTags {
    #[must_use]
    pub fn new(targets: Vec<TagTarget>) -> Self {
        Self(TagRemovalAction::new(targets, RemovalKind::All))
    }

    #[must_use]
    pub fn single(location: EntryLocation, original_content: String) -> Self {
        Self(TagRemovalAction::single(
            location,
            original_content,
            RemovalKind::All,
        ))
    }
}

impl Action for RemoveAllTags {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        self.0.execute(app)
    }

    fn description(&self) -> ActionDescription {
        self.0.description()
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
        let suffix = format!(" #{}", self.tag);
        for target in &self.targets {
            execute_content_append(app, &target.location, &suffix)?;
        }

        Ok(Box::new(RestoreContent::new(
            self.targets.clone(),
            TagOperation::Append(self.tag.clone()),
        )))
    }

    fn description(&self) -> ActionDescription {
        let count = self.targets.len();
        let (past, past_reversed) = if count == 1 {
            (
                format!("Added #{}", self.tag),
                format!("Removed #{}", self.tag),
            )
        } else {
            (
                format!("Added #{} to {} entries", self.tag, count),
                format!("Removed #{} from {} entries", self.tag, count),
            )
        };
        ActionDescription {
            past,
            past_reversed,
            visibility: StatusVisibility::Always,
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
        let mut new_targets = Vec::with_capacity(self.targets.len());
        for target in &self.targets {
            let current_content = get_entry_content(app, &target.location)?;
            set_entry_content(app, &target.location, &target.original_content)?;
            new_targets.push(ContentTarget::new(target.location.clone(), current_content));
        }

        let redo_action: Box<dyn Action> = match &self.operation {
            TagOperation::RemoveLast => Box::new(RemoveLastTag::new(new_targets)),
            TagOperation::RemoveAll => Box::new(RemoveAllTags::new(new_targets)),
            TagOperation::Append(tag) => Box::new(AppendTag::new(new_targets, tag.clone())),
        };

        Ok(redo_action)
    }

    fn description(&self) -> ActionDescription {
        let count = self.targets.len();
        let (past, past_reversed) = match (&self.operation, count == 1) {
            (TagOperation::RemoveLast, true) => {
                ("Restored tag".to_string(), "Removed tag".to_string())
            }
            (TagOperation::RemoveLast, false) => (
                format!("Restored tags on {} entries", count),
                format!("Removed tags from {} entries", count),
            ),
            (TagOperation::RemoveAll, true) => {
                ("Restored tags".to_string(), "Removed all tags".to_string())
            }
            (TagOperation::RemoveAll, false) => (
                format!("Restored tags on {} entries", count),
                format!("Removed all tags from {} entries", count),
            ),
            (TagOperation::Append(tag), true) => {
                (format!("Removed #{}", tag), format!("Added #{}", tag))
            }
            (TagOperation::Append(tag), false) => (
                format!("Removed #{} from {} entries", tag, count),
                format!("Added #{} to {} entries", tag, count),
            ),
        };
        ActionDescription {
            past,
            past_reversed,
            visibility: StatusVisibility::Always,
        }
    }
}
