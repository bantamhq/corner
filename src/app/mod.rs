pub mod actions;
mod command;
mod date_interface;
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

use std::collections::{BTreeSet, HashMap};
use std::io;
use std::path::Path;

use chrono::{Datelike, Local, NaiveDate};

use crate::config::Config;
use crate::cursor::CursorBuffer;
use crate::dispatch::Keymap;
use crate::storage::{
    self, DayInfo, Entry, EntryType, JournalContext, JournalSlot, Line, RawEntry,
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
    pub projected_entries: Vec<Entry>,
}

impl DailyState {
    #[must_use]
    pub fn new(entry_count: usize, projected_entries: Vec<Entry>) -> Self {
        let selected = if entry_count > 0 {
            projected_entries.len() + entry_count - 1
        } else if !projected_entries.is_empty() {
            projected_entries.len() - 1
        } else {
            0
        };

        Self {
            selected,
            scroll_offset: 0,
            original_lines: None,
            projected_entries,
        }
    }
}

/// State specific to the Filter view
#[derive(Clone)]
pub struct FilterState {
    pub query: String,
    pub query_buffer: CursorBuffer,
    pub entries: Vec<Entry>,
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

/// State for the date interface popup
#[derive(Clone, Debug)]
pub struct DateInterfaceState {
    /// Currently selected date
    pub selected: NaiveDate,
    /// First day of the displayed month
    pub display_month: NaiveDate,
    /// Cached day info for the display month
    pub day_cache: HashMap<NaiveDate, DayInfo>,
    /// Query input for quick date entry
    pub query: CursorBuffer,
    /// Whether the input field is focused (vs calendar)
    pub input_focused: bool,
}

impl DateInterfaceState {
    #[must_use]
    pub fn new(date: NaiveDate) -> Self {
        Self {
            selected: date,
            display_month: NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap(),
            day_cache: HashMap::new(),
            query: CursorBuffer::empty(),
            input_focused: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProjectInterfaceState {
    pub query: CursorBuffer,
    pub filtered_indices: Vec<usize>,
    pub selected: usize,
    pub projects: Vec<storage::ProjectInfo>,
}

impl Default for ProjectInterfaceState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectInterfaceState {
    #[must_use]
    pub fn new() -> Self {
        let registry = storage::ProjectRegistry::load();
        let mut state = Self {
            query: CursorBuffer::empty(),
            filtered_indices: Vec::new(),
            selected: 0,
            projects: registry.projects,
        };
        state.update_filter();
        state
    }

    pub fn update_filter(&mut self) {
        let query = self.query.content().to_lowercase();
        self.filtered_indices = self
            .projects
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                !p.hide_from_registry
                    && (query.is_empty()
                        || p.name.to_lowercase().starts_with(&query)
                        || p.id.to_lowercase().starts_with(&query))
            })
            .map(|(i, _)| i)
            .collect();

        if self.selected >= self.filtered_indices.len() {
            self.selected = self.filtered_indices.len().saturating_sub(1);
        }
    }

    #[must_use]
    pub fn selected_project(&self) -> Option<&storage::ProjectInfo> {
        self.filtered_indices
            .get(self.selected)
            .and_then(|&i| self.projects.get(i))
    }
}

/// State for the tag interface popup (stub for future implementation)
#[derive(Clone, Debug)]
pub struct TagInterfaceState {
    pub query: CursorBuffer,
    pub selected: usize,
}

impl Default for TagInterfaceState {
    fn default() -> Self {
        Self {
            query: CursorBuffer::empty(),
            selected: 0,
        }
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
    // Note: LaterEdit removed - edit is blocked on projected entries
}

/// Context for confirmation dialogs
#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmContext {
    CreateProjectJournal,
    AddToGitignore,
}

/// Context for prompt input modes (owns its buffer)
#[derive(Clone, Debug)]
pub enum PromptContext {
    Command { buffer: CursorBuffer },
    Filter { buffer: CursorBuffer },
}

/// Context for interface overlays
#[derive(Clone, Debug)]
pub enum InterfaceContext {
    Date(DateInterfaceState),
    Project(ProjectInterfaceState),
    Tag(TagInterfaceState),
}

/// What keyboard handler to use
#[derive(Clone, Debug)]
pub enum InputMode {
    Normal,
    Edit(EditContext),
    Reorder,
    Selection(SelectionState),
    Confirm(ConfirmContext),
    Prompt(PromptContext),
    Interface(InterfaceContext),
}

/// Where to insert a new entry
pub enum InsertPosition {
    Bottom,
    Below,
    Above,
}

/// The currently selected item, accounting for hidden completed entries
pub enum SelectedItem<'a> {
    Projected {
        index: usize,
        entry: &'a Entry,
    },
    Daily {
        index: usize,
        line_idx: usize,
        entry: &'a RawEntry,
    },
    Filter {
        index: usize,
        entry: &'a Entry,
    },
    None,
}

pub struct App {
    pub current_date: NaiveDate,
    pub last_daily_date: NaiveDate,
    pub lines: Vec<Line>,
    pub entry_indices: Vec<usize>,
    pub view: ViewMode,
    pub input_mode: InputMode,
    pub edit_buffer: Option<CursorBuffer>,
    pub should_quit: bool,
    pub needs_redraw: bool,
    pub status_message: Option<String>,
    pub show_help: bool,
    pub help_scroll: usize,
    pub help_visible_height: usize,
    pub last_filter_query: Option<String>,
    pub config: Config,
    pub journal_context: JournalContext,
    pub in_git_repo: bool,
    pub hide_completed: bool,
    pub hint_state: HintContext,
    pub cached_journal_tags: Vec<String>,
    pub executor: actions::ActionExecutor,
    pub keymap: Keymap,
    original_edit_content: Option<String>,
}

impl App {
    pub fn new(config: Config) -> io::Result<Self> {
        Self::new_with_date(config, Local::now().date_naive())
    }

    /// Creates a new App with a specific date, detecting paths from config
    pub fn new_with_date(config: Config, date: NaiveDate) -> io::Result<Self> {
        let hub_path = config
            .hub_file
            .as_ref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(crate::config::get_default_journal_path);
        let project_path = storage::detect_project_journal();
        let context = JournalContext::new(hub_path, project_path, JournalSlot::Hub);
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
        let projected_entries = storage::collect_projected_entries_for_date(date, &path)?;
        let projected_entries = navigation::filter_done_today_recurring(projected_entries, &lines);
        let in_git_repo = storage::find_git_root().is_some();
        let cached_journal_tags = storage::collect_journal_tags(&path).unwrap_or_default();
        let hide_completed = config.hide_completed;

        let keymap = Keymap::new(&config.keys).unwrap_or_else(|e| {
            eprintln!("Invalid key config: {e}");
            Keymap::default()
        });

        let mut app = Self {
            current_date: date,
            last_daily_date: date,
            lines,
            view: ViewMode::Daily(DailyState::new(entry_indices.len(), projected_entries)),
            entry_indices,
            input_mode: InputMode::Normal,
            edit_buffer: None,
            should_quit: false,
            needs_redraw: false,
            status_message: None,
            show_help: false,
            help_scroll: 0,
            help_visible_height: 0,
            last_filter_query: None,
            config,
            journal_context,
            in_git_repo,
            hide_completed,
            hint_state: HintContext::Inactive,
            cached_journal_tags,
            executor: actions::ActionExecutor::new(),
            keymap,
            original_edit_content: None,
        };

        if hide_completed {
            app.clamp_selection_to_visible();
        }

        Ok(app)
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

    pub(super) fn get_daily_entry(&self, entry_index: usize) -> Option<&RawEntry> {
        let line_idx = self.entry_indices.get(entry_index)?;
        if let Line::Entry(entry) = &self.lines[*line_idx] {
            Some(entry)
        } else {
            None
        }
    }

    pub(super) fn get_daily_entry_mut(&mut self, entry_index: usize) -> Option<&mut RawEntry> {
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

    pub fn execute_action(&mut self, action: Box<dyn actions::Action>) -> io::Result<()> {
        let mut executor = std::mem::take(&mut self.executor);
        let result = executor.execute(action, self);
        self.executor = executor;

        match result {
            Ok(Some(msg)) => {
                self.refresh_tag_cache();
                self.set_status(msg);
            }
            Ok(None) => {
                self.refresh_tag_cache();
            }
            Err(e) => {
                self.set_status(format!("Action failed: {e}"));
                return Err(e);
            }
        }
        Ok(())
    }

    /// Saves current day's lines to storage, displaying any error as a status message.
    pub fn save(&mut self) {
        if let Err(e) = storage::save_day_lines(self.current_date, self.active_path(), &self.lines)
        {
            self.set_status(format!("Failed to save: {e}"));
        }
    }

    pub fn undo(&mut self) {
        let mut executor = std::mem::take(&mut self.executor);
        let result = executor.undo(self);
        self.executor = executor;

        match result {
            Ok(Some(msg)) => {
                self.refresh_tag_cache();
                self.set_status(msg);
            }
            Ok(None) => {}
            Err(e) => self.set_status(format!("Undo failed: {e}")),
        }
    }

    pub fn redo(&mut self) -> io::Result<()> {
        let mut executor = std::mem::take(&mut self.executor);
        let result = executor.redo(self);
        self.executor = executor;

        match result {
            Ok(Some(msg)) => {
                self.refresh_tag_cache();
                self.set_status(msg);
            }
            Ok(None) => {}
            Err(e) => self.set_status(format!("Redo failed: {e}")),
        }
        Ok(())
    }

    pub fn tidy_entries(&mut self) {
        let entry_positions: Vec<usize> = self
            .lines
            .iter()
            .enumerate()
            .filter_map(|(i, l)| matches!(l, Line::Entry(_)).then_some(i))
            .collect();

        if entry_positions.is_empty() {
            return;
        }

        let tidy_order = self.config.validated_tidy_order();
        let get_priority = |line: &Line| -> usize {
            let Line::Entry(entry) = line else {
                return tidy_order.len();
            };
            for (i, type_name) in tidy_order.iter().enumerate() {
                match (type_name.as_str(), &entry.entry_type) {
                    ("completed", EntryType::Task { completed: true }) => return i,
                    ("uncompleted", EntryType::Task { completed: false }) => return i,
                    ("notes", EntryType::Note) => return i,
                    ("events", EntryType::Event) => return i,
                    _ => {}
                }
            }
            tidy_order.len()
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
            InputMode::Prompt(PromptContext::Command { buffer }) => {
                (buffer.content(), HintMode::Command)
            }
            InputMode::Prompt(PromptContext::Filter { buffer }) => {
                (buffer.content(), HintMode::Filter)
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

        let saved_filters: Vec<String> = if mode == HintMode::Filter {
            self.config.filters.keys().cloned().collect()
        } else {
            Vec::new()
        };
        self.hint_state =
            HintContext::compute(input, mode, &self.cached_journal_tags, &saved_filters);
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

        match &mut self.input_mode {
            InputMode::Prompt(PromptContext::Command { buffer }) => {
                for c in completion.chars() {
                    buffer.insert_char(c);
                }
            }
            InputMode::Prompt(PromptContext::Filter { buffer }) => {
                for c in completion.chars() {
                    buffer.insert_char(c);
                }
            }
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

    /// Get the content of the current prompt buffer (if in prompt mode)
    #[must_use]
    pub fn prompt_content(&self) -> Option<&str> {
        match &self.input_mode {
            InputMode::Prompt(PromptContext::Command { buffer }) => Some(buffer.content()),
            InputMode::Prompt(PromptContext::Filter { buffer }) => Some(buffer.content()),
            _ => None,
        }
    }

    /// Check if the current prompt buffer is empty
    #[must_use]
    pub fn prompt_is_empty(&self) -> bool {
        match &self.input_mode {
            InputMode::Prompt(PromptContext::Command { buffer }) => buffer.is_empty(),
            InputMode::Prompt(PromptContext::Filter { buffer }) => buffer.is_empty(),
            _ => true,
        }
    }

    /// Get a mutable reference to the current prompt buffer (if in prompt mode)
    pub fn prompt_buffer_mut(&mut self) -> Option<&mut CursorBuffer> {
        match &mut self.input_mode {
            InputMode::Prompt(PromptContext::Command { buffer }) => Some(buffer),
            InputMode::Prompt(PromptContext::Filter { buffer }) => Some(buffer),
            _ => None,
        }
    }

    #[must_use]
    pub fn input_needs_continuation(&self) -> bool {
        self.prompt_content()
            .map(|c| c.ends_with(':'))
            .unwrap_or(false)
    }

    /// Enter command prompt mode
    pub fn enter_command_mode(&mut self) {
        self.input_mode = InputMode::Prompt(PromptContext::Command {
            buffer: CursorBuffer::empty(),
        });
    }

    /// Enter filter prompt mode
    pub fn enter_filter_mode(&mut self) {
        let buffer = match &self.view {
            ViewMode::Filter(state) => state.query_buffer.clone(),
            ViewMode::Daily(_) => CursorBuffer::empty(),
        };
        self.input_mode = InputMode::Prompt(PromptContext::Filter { buffer });
    }

    /// Exit prompt mode and return to normal
    pub fn exit_prompt(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    /// Open the project interface
    pub fn open_project_interface(&mut self) {
        self.input_mode =
            InputMode::Interface(InterfaceContext::Project(ProjectInterfaceState::new()));
    }

    /// Move selection up in project interface
    pub fn project_interface_move_up(&mut self) {
        let InputMode::Interface(InterfaceContext::Project(ref mut state)) = self.input_mode else {
            return;
        };
        state.selected = state.selected.saturating_sub(1);
    }

    /// Move selection down in project interface
    pub fn project_interface_move_down(&mut self) {
        let InputMode::Interface(InterfaceContext::Project(ref mut state)) = self.input_mode else {
            return;
        };
        if state.selected + 1 < state.filtered_indices.len() {
            state.selected += 1;
        }
    }

    /// Select the current project in the interface
    pub fn project_interface_select(&mut self) -> std::io::Result<()> {
        let project_info = {
            let InputMode::Interface(InterfaceContext::Project(ref state)) = self.input_mode else {
                return Ok(());
            };
            state.selected_project().map(|p| (p.id.clone(), p.available))
        };

        match project_info {
            Some((id, true)) => {
                self.input_mode = InputMode::Normal;
                self.switch_to_registered_project(&id)?;
            }
            Some((id, false)) => {
                self.set_status(format!("Project '{}' is unavailable", id));
            }
            None => {
                self.input_mode = InputMode::Normal;
            }
        }
        Ok(())
    }

    /// Close the current interface and return to normal
    pub fn close_interface(&mut self) {
        self.input_mode = InputMode::Normal;
    }
}
