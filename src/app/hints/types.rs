use crate::registry::{Command, DateScope, DateValue, FilterSyntax};

/// What kind of hints to display
#[derive(Clone, Debug, PartialEq)]
pub enum HintContext {
    /// No hints to display
    Inactive,
    /// Guidance text (help message only, shown at bottom)
    GuidanceMessage { message: &'static str },
    /// Tag hints from current journal
    Tags {
        prefix: String,
        matches: Vec<String>,
        selected: usize,
        scroll_offset: usize,
    },
    /// Command hints (from registry)
    Commands {
        prefix: String,
        matches: Vec<&'static Command>,
    },
    /// Filter type hints (!tasks, !notes, etc.)
    FilterTypes {
        prefix: String,
        matches: Vec<&'static FilterSyntax>,
        selected: usize,
        scroll_offset: usize,
    },
    /// Content pattern hints (@recurring)
    DateOps {
        prefix: String,
        matches: Vec<&'static FilterSyntax>,
        selected: usize,
        scroll_offset: usize,
    },
    /// Date value hints (entry dates or filter date values)
    DateValues {
        prefix: String,
        scope: DateScope,
        matches: Vec<&'static DateValue>,
        selected: usize,
        scroll_offset: usize,
    },
    /// Saved filter hints ($name)
    SavedFilters {
        prefix: String,
        matches: Vec<String>,
        selected: usize,
        scroll_offset: usize,
    },
    /// Negation hints - wraps inner context for recursive hints
    Negation { inner: Box<HintContext> },
}

/// Which input context we're computing hints for
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HintMode {
    /// Command mode (:)
    Command,
    /// Filter query mode (/)
    Filter,
    /// Entry editing mode
    Entry,
}

#[derive(Clone, Debug)]
pub struct HintItem {
    pub label: String,
    pub selectable: bool,
}
