mod command;
mod edit_mode;
mod entry_ops;
mod filter_ops;
pub mod hints;
mod journal;
mod navigation;
mod reorder;
mod selection_ops;

pub use entry_ops::{DeleteTarget, EntryLocation, TagRemovalTarget, ToggleTarget, YankTarget};
pub use hints::{HintContext, HintMode};

use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use chrono::{Local, NaiveDate};

use crate::config::Config;
use crate::cursor::CursorBuffer;
use crate::storage::{
    self, Entry, EntryType, FilterEntry, JournalContext, JournalSlot, LaterEntry, Line,
};

pub const DAILY_HEADER_LINES: usize = 1;
pub const FILTER_HEADER_LINES: usize = 1;
pub const DATE_SUFFIX_WIDTH: usize = " (MM/DD)".len();

/// State specific to the Daily view
#[derive(Clone)]
pub struct DailyState {
    pub selected: usize,
    pub scroll_offset: usize,
    pub original_lines: Option<Vec<Line>>,
    pub later_entries: Vec<LaterEntry>,
}

impl DailyState {
    #[must_use]
    pub fn new(entry_count: usize, later_entries: Vec<LaterEntry>) -> Self {
        let selected = if later_entries.is_empty() {
            entry_count.saturating_sub(1)
        } else {
            0
        };

        Self {
            selected,
            scroll_offset: 0,
            original_lines: None,
            later_entries,
        }
    }
}

/// State specific to the Filter view
#[derive(Clone)]
pub struct FilterState {
    pub query: String,
    pub query_buffer: CursorBuffer,
    pub entries: Vec<FilterEntry>,
    pub selected: usize,
    pub scroll_offset: usize,
}

/// State for multi-select operations
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionState {
    /// The anchor index where selection started (visible index)
    pub anchor: usize,
    /// Set of all currently selected visible indices
    pub selected_indices: BTreeSet<usize>,
}

impl SelectionState {
    #[must_use]
    pub fn new(anchor: usize) -> Self {
        let mut selected_indices = BTreeSet::new();
        selected_indices.insert(anchor);
        Self {
            anchor,
            selected_indices,
        }
    }

    /// Toggle range from anchor to new index, then update anchor to new index
    /// If any in range are unselected, select all; otherwise deselect all
    pub fn extend_to(&mut self, new_index: usize) {
        let (start, end) = if new_index < self.anchor {
            (new_index, self.anchor)
        } else {
            (self.anchor, new_index)
        };
        let all_selected = (start..=end).all(|i| self.selected_indices.contains(&i));
        if all_selected {
            for i in start..=end {
                self.selected_indices.remove(&i);
            }
        } else {
            for i in start..=end {
                self.selected_indices.insert(i);
            }
        }
        self.anchor = new_index;
    }

    /// Toggle a single index in selection and update anchor
    pub fn toggle(&mut self, index: usize) {
        if self.selected_indices.contains(&index) {
            self.selected_indices.remove(&index);
        } else {
            self.selected_indices.insert(index);
        }
        self.anchor = index;
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.selected_indices.len()
    }

    #[must_use]
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_indices.contains(&index)
    }

    /// Returns indices in descending order (for safe deletion)
    #[must_use]
    pub fn indices_descending(&self) -> Vec<usize> {
        self.selected_indices.iter().copied().rev().collect()
    }
}

/// Which view is currently active and its state
#[derive(Clone)]
pub enum ViewMode {
    Daily(DailyState),
    Filter(FilterState),
}

/// Context for what is being edited
#[derive(Clone, Debug, PartialEq)]
pub enum EditContext {
    /// Editing an entry in Daily view
    Daily { entry_index: usize },
    /// Editing an existing entry from Filter view
    FilterEdit {
        date: NaiveDate,
        line_index: usize,
        filter_index: usize,
    },
    /// Quick-adding a new entry from Filter view
    FilterQuickAdd {
        date: NaiveDate,
        entry_type: EntryType,
    },
    /// Editing a later entry from Daily view
    LaterEdit {
        source_date: NaiveDate,
        line_index: usize,
        later_index: usize,
    },
}

/// Context for confirmation dialogs
#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmContext {
    CreateProjectJournal,
    AddToGitignore,
}

/// What keyboard handler to use
#[derive(Clone, Debug, PartialEq)]
pub enum InputMode {
    Normal,
    Edit(EditContext),
    Command,
    Reorder,
    QueryInput,
    Confirm(ConfirmContext),
    Selection(SelectionState),
}

/// Where to insert a new entry
pub enum InsertPosition {
    Bottom,
    Below,
    Above,
}

/// The currently selected item, accounting for hidden completed entries
pub enum SelectedItem<'a> {
    Later {
        index: usize,
        entry: &'a LaterEntry,
    },
    Daily {
        index: usize,
        line_idx: usize,
        entry: &'a Entry,
    },
    Filter {
        index: usize,
        entry: &'a FilterEntry,
    },
    None,
}

pub struct App {
    pub current_date: NaiveDate,
    pub lines: Vec<Line>,
    pub entry_indices: Vec<usize>,
    pub view: ViewMode,
    pub input_mode: InputMode,
    pub edit_buffer: Option<CursorBuffer>,
    pub command_buffer: CursorBuffer,
    pub should_quit: bool,
    pub needs_redraw: bool,
    pub status_message: Option<String>,
    pub show_help: bool,
    pub help_scroll: usize,
    pub help_visible_height: usize,
    pub last_deleted: Option<(NaiveDate, usize, Entry)>,
    pub last_filter_query: Option<String>,
    pub config: Config,
    pub journal_context: JournalContext,
    pub in_git_repo: bool,
    pub hide_completed: bool,
    pub hint_state: HintContext,
    pub cached_journal_tags: Vec<String>,
}

impl App {
    pub fn new(config: Config) -> io::Result<Self> {
        Self::new_with_date(config, Local::now().date_naive())
    }

    /// Creates a new App with a specific date, detecting paths from config
    pub fn new_with_date(config: Config, date: NaiveDate) -> io::Result<Self> {
        let global_path = config
            .global_file
            .as_ref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(crate::config::get_default_journal_path);
        let project_path = storage::detect_project_journal();
        let context = JournalContext::new(global_path, project_path, JournalSlot::Global);
        Self::new_with_context(config, date, context)
    }

    /// Creates a new App with explicit context (for testing and main)
    pub fn new_with_context(
        config: Config,
        date: NaiveDate,
        journal_context: JournalContext,
    ) -> io::Result<Self> {
        let path = journal_context.active_path().to_path_buf();
        let lines = storage::load_day_lines(date, &path)?;
        let entry_indices = Self::compute_entry_indices(&lines);
        let later_entries = storage::collect_later_entries_for_date(date, &path)?;
        let in_git_repo = storage::find_git_root().is_some();
        let cached_journal_tags = storage::collect_journal_tags(&path).unwrap_or_default();
        let hide_completed = config.hide_completed;

        Ok(Self {
            current_date: date,
            lines,
            view: ViewMode::Daily(DailyState::new(entry_indices.len(), later_entries)),
            entry_indices,
            input_mode: InputMode::Normal,
            edit_buffer: None,
            command_buffer: CursorBuffer::empty(),
            should_quit: false,
            needs_redraw: false,
            status_message: None,
            show_help: false,
            help_scroll: 0,
            help_visible_height: 0,
            last_deleted: None,
            last_filter_query: None,
            config,
            journal_context,
            in_git_repo,
            hide_completed,
            hint_state: HintContext::Inactive,
            cached_journal_tags,
        })
    }

    /// Returns the active journal path
    #[must_use]
    pub fn active_path(&self) -> &Path {
        self.journal_context.active_path()
    }

    /// Returns the active journal slot
    #[must_use]
    pub fn active_journal(&self) -> JournalSlot {
        self.journal_context.active_slot()
    }

    #[must_use]
    pub fn compute_entry_indices(lines: &[Line]) -> Vec<usize> {
        lines
            .iter()
            .enumerate()
            .filter_map(|(i, line)| {
                if matches!(line, Line::Entry(_)) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect()
    }

    pub(super) fn get_daily_entry(&self, entry_index: usize) -> Option<&Entry> {
        let line_idx = self.entry_indices.get(entry_index)?;
        if let Line::Entry(entry) = &self.lines[*line_idx] {
            Some(entry)
        } else {
            None
        }
    }

    pub(super) fn get_daily_entry_mut(&mut self, entry_index: usize) -> Option<&mut Entry> {
        let line_idx = *self.entry_indices.get(entry_index)?;
        if let Line::Entry(entry) = &mut self.lines[line_idx] {
            Some(entry)
        } else {
            None
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
    }

    /// Saves current day's lines to storage, displaying any error as a status message.
    pub fn save(&mut self) {
        if let Err(e) = storage::save_day_lines(self.current_date, self.active_path(), &self.lines)
        {
            self.set_status(format!("Failed to save: {e}"));
        }
    }

    pub fn undo(&mut self) {
        let Some((date, line_idx, entry)) = self.last_deleted.take() else {
            return;
        };

        match &self.view {
            ViewMode::Daily(_) => {
                if date != self.current_date {
                    self.set_status(format!(
                        "Undo: entry was from {}, go to that day first",
                        date.format("%m/%d")
                    ));
                    self.last_deleted = Some((date, line_idx, entry));
                    return;
                }
                let insert_idx = line_idx.min(self.lines.len());
                let is_completed = matches!(entry.entry_type, EntryType::Task { completed: true });
                self.lines.insert(insert_idx, Line::Entry(entry));
                self.entry_indices = Self::compute_entry_indices(&self.lines);

                if self.hide_completed && is_completed {
                    self.hide_completed = false;
                }

                let visible_idx = self
                    .entry_indices
                    .iter()
                    .position(|&i| i == insert_idx)
                    .map(|actual_idx| self.actual_to_visible_index(actual_idx));

                if let ViewMode::Daily(state) = &mut self.view
                    && let Some(idx) = visible_idx
                {
                    state.selected = idx;
                }
                self.save();
            }
            ViewMode::Filter(_) => {
                let path = self.active_path();
                if let Ok(mut lines) = storage::load_day_lines(date, path) {
                    let insert_idx = line_idx.min(lines.len());
                    lines.insert(insert_idx, Line::Entry(entry.clone()));
                    let _ = storage::save_day_lines(date, path, &lines);

                    let filter_entry = FilterEntry {
                        source_date: date,
                        line_index: insert_idx,
                        entry_type: entry.entry_type.clone(),
                        content: entry.content,
                        completed: matches!(entry.entry_type, EntryType::Task { completed: true }),
                    };

                    if let ViewMode::Filter(state) = &mut self.view {
                        state.entries.push(filter_entry);
                        state.selected = state.entries.len() - 1;
                    }

                    if date == self.current_date {
                        let _ = self.reload_current_day();
                    }
                }
            }
        }
        self.refresh_tag_cache();
    }

    pub fn sort_entries(&mut self) {
        let entry_positions: Vec<usize> = self
            .lines
            .iter()
            .enumerate()
            .filter_map(|(i, l)| matches!(l, Line::Entry(_)).then_some(i))
            .collect();

        if entry_positions.is_empty() {
            return;
        }

        let sort_order = self.config.validated_sort_order();
        let get_priority = |line: &Line| -> usize {
            let Line::Entry(entry) = line else {
                return sort_order.len();
            };
            for (i, type_name) in sort_order.iter().enumerate() {
                match (type_name.as_str(), &entry.entry_type) {
                    ("completed", EntryType::Task { completed: true }) => return i,
                    ("uncompleted", EntryType::Task { completed: false }) => return i,
                    ("notes", EntryType::Note) => return i,
                    ("events", EntryType::Event) => return i,
                    _ => {}
                }
            }
            sort_order.len()
        };

        let mut entries: Vec<Line> = entry_positions
            .iter()
            .map(|&i| self.lines[i].clone())
            .collect();

        entries.sort_by_key(|line| get_priority(line));

        for (pos, entry) in entry_positions.iter().zip(entries.into_iter()) {
            self.lines[*pos] = entry;
        }

        self.entry_indices = Self::compute_entry_indices(&self.lines);
        self.save();
    }

    pub(crate) fn reload_current_day(&mut self) -> io::Result<()> {
        self.lines = storage::load_day_lines(self.current_date, self.active_path())?;
        self.entry_indices = Self::compute_entry_indices(&self.lines);
        Ok(())
    }

    /// Update hints based on current input buffer and mode.
    pub fn update_hints(&mut self) {
        let (input, mode) = match &self.input_mode {
            InputMode::Command => (self.command_buffer.content(), HintMode::Command),
            InputMode::QueryInput => {
                let buffer = match &self.view {
                    ViewMode::Filter(state) => state.query_buffer.content(),
                    ViewMode::Daily(_) => self.command_buffer.content(),
                };
                (buffer, HintMode::Filter)
            }
            InputMode::Edit(_) => {
                if let Some(ref buffer) = self.edit_buffer {
                    (buffer.content(), HintMode::Entry)
                } else {
                    self.hint_state = HintContext::Inactive;
                    return;
                }
            }
            _ => {
                self.hint_state = HintContext::Inactive;
                return;
            }
        };

        self.hint_state = HintContext::compute(input, mode, &self.cached_journal_tags);
    }

    /// Clear any active hints.
    pub fn clear_hints(&mut self) {
        self.hint_state = HintContext::Inactive;
    }

    pub fn refresh_tag_cache(&mut self) {
        self.cached_journal_tags =
            storage::collect_journal_tags(self.active_path()).unwrap_or_default();
    }

    pub fn accept_hint(&mut self) -> bool {
        let Some(completion) = self.hint_state.first_completion() else {
            return false;
        };

        if completion.is_empty() {
            return false;
        }

        match &self.input_mode {
            InputMode::Command => {
                for c in completion.chars() {
                    self.command_buffer.insert_char(c);
                }
            }
            InputMode::QueryInput => match &mut self.view {
                ViewMode::Filter(state) => {
                    for c in completion.chars() {
                        state.query_buffer.insert_char(c);
                    }
                }
                ViewMode::Daily(_) => {
                    for c in completion.chars() {
                        self.command_buffer.insert_char(c);
                    }
                }
            },
            InputMode::Edit(_) => {
                if let Some(ref mut buffer) = self.edit_buffer {
                    for c in completion.chars() {
                        buffer.insert_char(c);
                    }
                }
            }
            _ => return false,
        }

        self.clear_hints();
        true
    }
}
