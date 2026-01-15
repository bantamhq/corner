use std::io;

use crate::app::{App, EntryLocation};

/// Target for content operations - includes original content for undo.
/// Used as a base type for TagTarget and DateTarget.
#[derive(Clone)]
pub struct ContentTarget {
    pub location: EntryLocation,
    pub original_content: String,
}

impl ContentTarget {
    #[must_use]
    pub fn new(location: EntryLocation, original_content: String) -> Self {
        Self {
            location,
            original_content,
        }
    }
}

pub fn get_entry_content(app: &App, location: &EntryLocation) -> io::Result<String> {
    app.get_entry_content(location)
}

/// Bypasses normalization for undo/restore operations.
pub fn set_entry_content(app: &mut App, location: &EntryLocation, content: &str) -> io::Result<()> {
    app.persist_entry_content(location, content)
}

/// Returns Some(new_content) from operation to trigger save with normalization.
pub fn execute_content_operation<F>(
    app: &mut App,
    location: &EntryLocation,
    operation: F,
) -> io::Result<()>
where
    F: Fn(&str) -> Option<String>,
{
    let current_content = app.get_entry_content(location)?;
    if let Some(new_content) = operation(&current_content) {
        app.save_entry_content(location, new_content)?;
    }
    Ok(())
}

pub fn execute_content_append(
    app: &mut App,
    location: &EntryLocation,
    suffix: &str,
) -> io::Result<()> {
    let current_content = app.get_entry_content(location)?;
    let new_content = format!("{current_content}{suffix}");
    app.save_entry_content(location, new_content)?;
    Ok(())
}
