pub mod actions;
mod command;
mod date_interface;
mod edit_mode;
mod entry_ops;
mod filter_ops;
pub mod hints;
mod interface_ops;
mod journal;
mod navigation;
mod reorder;
mod selection_ops;
mod tag_interface;
mod tag_ops;

pub use entry_ops::{DeleteTarget, EntryLocation, TagRemovalTarget, ToggleTarget, YankTarget};
pub use hints::{HintContext, HintMode};

use std::collections::{BTreeSet, HashMap};
use std::io;
use std::path::Path;

use chrono::{Datelike, Local, NaiveDate};

use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::calendar::{
    CalendarFetchResult, CalendarStore, fetch_all_calendars, get_visible_calendar_ids, update_store,
};
use crate::config::Config;
use crate::cursor::CursorBuffer;
use crate::dispatch::Keymap;
use crate::storage::{
    self, DayInfo, Entry, EntryType, JournalContext, JournalSlot, Line, ProjectRegistry, RawEntry,
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
    /// Cursor position of last selection operation - on next move, anchor updates to this
    last_operation_at: Option<usize>,
}

impl SelectionState {
    #[must_use]
    pub fn new(anchor: usize) -> Self {
        let mut selected_indices = BTreeSet::new();
        selected_indices.insert(anchor);
        Self {
            anchor,
            selected_indices,
            last_operation_at: None,
        }
    }

    /// Toggle range from anchor to cursor position.
    /// If all in range are selected, deselect all; otherwise select all.
    /// Anchor does NOT move - call on_cursor_move before movement to update anchor.
    pub fn extend_to(&mut self, cursor: usize) {
        let (start, end) = if cursor < self.anchor {
            (cursor, self.anchor)
        } else {
            (self.anchor, cursor)
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
        self.last_operation_at = Some(cursor);
    }

    /// Toggle a single index in selection and update anchor immediately
    pub fn toggle(&mut self, index: usize) {
        if self.selected_indices.contains(&index) {
            self.selected_indices.remove(&index);
        } else {
            self.selected_indices.insert(index);
        }
        self.anchor = index;
        self.last_operation_at = Some(index);
    }

    /// Call before cursor movement - updates anchor to last operation position
    /// only if that position is still selected
    pub fn on_cursor_move(&mut self) {
        if let Some(pos) = self.last_operation_at.take()
            && self.selected_indices.contains(&pos)
        {
            self.anchor = pos;
        }
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

/// Tracks the last interaction type in the date interface for smart submit
#[derive(Clone, Debug, Default, PartialEq)]
pub enum LastInteraction {
    /// User typed into the query field
    Typed,
    /// User navigated the calendar (default)
    #[default]
    Calendar,
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
    /// Last interaction type for smart submit behavior
    pub last_interaction: LastInteraction,
}

impl DateInterfaceState {
    #[must_use]
    pub fn new(date: NaiveDate) -> Self {
        Self {
            selected: date,
            display_month: NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap(),
            day_cache: HashMap::new(),
            query: CursorBuffer::empty(),
            last_interaction: LastInteraction::Calendar,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProjectInterfaceState {
    pub selected: usize,
    pub scroll_offset: usize,
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
        let mut projects: Vec<_> = registry
            .projects
            .into_iter()
            .filter(|p| !p.hide_from_registry)
            .collect();
        projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Self {
            selected: 0,
            scroll_offset: 0,
            projects,
        }
    }

    #[must_use]
    pub fn selected_project(&self) -> Option<&storage::ProjectInfo> {
        self.projects.get(self.selected)
    }

    /// Remove the selected project from the registry and return its ID.
    pub fn remove_selected(&mut self) -> Option<String> {
        let project = self.selected_project()?;
        let id = project.id.clone();

        if std::env::var("CALIBER_SKIP_REGISTRY").is_err() {
            let mut registry = storage::ProjectRegistry::load();
            if registry.remove(&id) {
                let _ = registry.save();
            }
        }

        self.projects.remove(self.selected);
        if self.selected >= self.projects.len() && self.selected > 0 {
            self.selected -= 1;
        }

        Some(id)
    }

    /// Hide the selected project by setting hide_from_registry in its config.
    pub fn hide_selected(&mut self) -> Option<String> {
        let project = self.selected_project()?;
        let id = project.id.clone();
        let path = project.path.clone();

        if storage::set_hide_from_registry(&path, true).is_ok() {
            self.projects.remove(self.selected);
            if self.selected >= self.projects.len() && self.selected > 0 {
                self.selected -= 1;
            }
            return Some(id);
        }
        None
    }
}

/// Information about a tag in the journal
#[derive(Clone, Debug)]
pub struct TagInfo {
    pub name: String,
    pub count: usize,
}

/// State for the tag interface popup
#[derive(Clone, Debug)]
pub struct TagInterfaceState {
    pub selected: usize,
    pub scroll_offset: usize,
    pub tags: Vec<TagInfo>,
}

impl TagInterfaceState {
    #[must_use]
    pub fn new(tags: Vec<TagInfo>) -> Self {
        Self {
            selected: 0,
            scroll_offset: 0,
            tags,
        }
    }

    #[must_use]
    pub fn selected_tag(&self) -> Option<&TagInfo> {
        self.tags.get(self.selected)
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
    DeleteTag(String),
}

/// Context for prompt input modes (owns its buffer)
#[derive(Clone, Debug)]
pub enum PromptContext {
    Command {
        buffer: CursorBuffer,
    },
    Filter {
        buffer: CursorBuffer,
    },
    RenameTag {
        old_tag: String,
        buffer: CursorBuffer,
    },
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
    pub cached_journal_tags: Vec<TagInfo>,
    pub executor: actions::ActionExecutor,
    pub keymap: Keymap,
    original_edit_content: Option<String>,
    pub calendar_store: CalendarStore,
    runtime_handle: Option<Handle>,
    calendar_rx: Option<mpsc::Receiver<CalendarFetchResult>>,
    calendar_tx: Option<mpsc::Sender<CalendarFetchResult>>,
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
        Self::new_with_context(config, date, context, None)
    }

    /// Creates a new App with explicit context (for testing and main)
    pub fn new_with_context(
        config: Config,
        date: NaiveDate,
        journal_context: JournalContext,
        runtime_handle: Option<Handle>,
    ) -> io::Result<Self> {
        let path = journal_context.active_path().to_path_buf();
        let lines = storage::load_day_lines(date, &path)?;
        let entry_indices = Self::compute_entry_indices(&lines);
        let projected_entries = storage::collect_projected_entries_for_date(date, &path)?;
        let projected_entries = navigation::filter_done_today_recurring(projected_entries, &lines);
        let in_git_repo = storage::find_git_root().is_some();
        let cached_journal_tags = Vec::new();
        let hide_completed = config.hide_completed;

        let keymap = Keymap::new(&config.keys).unwrap_or_else(|e| {
            eprintln!("Invalid key config: {e}");
            Keymap::default()
        });

        let (calendar_tx, calendar_rx) = if runtime_handle.is_some() {
            let (tx, rx) = mpsc::channel(1);
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

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
            calendar_store: CalendarStore::new(),
            runtime_handle,
            calendar_rx,
            calendar_tx,
        };

        if hide_completed {
            app.clamp_selection_to_visible();
        }

        app.trigger_calendar_fetch();

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

    pub fn trigger_calendar_fetch(&mut self) {
        let Some(ref handle) = self.runtime_handle else {
            return;
        };

        if !self.config.has_calendars() {
            return;
        }

        let project = self.get_current_project_info();
        let visible_ids = get_visible_calendar_ids(
            &self.config,
            &self.active_journal(),
            project.as_ref(),
        );

        if visible_ids.is_empty() {
            self.calendar_store.clear();
            return;
        }

        self.calendar_store.set_fetching();

        let config = self.config.clone();
        let Some(tx) = self.calendar_tx.clone() else {
            return;
        };

        handle.spawn(async move {
            let result = fetch_all_calendars(&config, &visible_ids).await;
            let _ = tx.send(result).await;
        });
    }

    pub fn poll_calendar_results(&mut self) {
        let Some(ref mut rx) = self.calendar_rx else {
            return;
        };
        if let Ok(result) = rx.try_recv() {
            update_store(&mut self.calendar_store, result);
        }
    }

    /// Returns the number of calendar events for the current date.
    /// Used for scroll offset calculations since calendar events are rendered
    /// at the top but are not selectable.
    #[must_use]
    pub fn calendar_event_count(&self) -> usize {
        self.calendar_store.events_for_date(self.current_date).len()
    }

    /// Get ProjectInfo for the current project (if in project journal).
    fn get_current_project_info(&self) -> Option<storage::ProjectInfo> {
        if !matches!(self.active_journal(), JournalSlot::Project) {
            return None;
        }
        let path = self.journal_context.project_path()?;
        let registry = ProjectRegistry::load();
        registry.find_by_path(path).cloned()
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
                self.set_status(msg);
            }
            Ok(None) => {}
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
                (buffer.content().to_string(), HintMode::Command)
            }
            InputMode::Prompt(PromptContext::Filter { buffer }) => {
                (buffer.content().to_string(), HintMode::Filter)
            }
            InputMode::Edit(_) => {
                if let Some(ref buffer) = self.edit_buffer {
                    (buffer.content().to_string(), HintMode::Entry)
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

        self.refresh_tag_cache();

        let saved_filters: Vec<String> = if mode == HintMode::Filter {
            self.config.filters.keys().cloned().collect()
        } else {
            Vec::new()
        };

        let tag_names: Vec<String> = self
            .cached_journal_tags
            .iter()
            .map(|t| t.name.clone())
            .collect();

        self.hint_state = HintContext::compute(&input, mode, &tag_names, &saved_filters);
    }

    /// Clear any active hints.
    pub fn clear_hints(&mut self) {
        self.hint_state = HintContext::Inactive;
    }

    pub fn refresh_tag_cache(&mut self) {
        self.cached_journal_tags = self.collect_all_tags().unwrap_or_default();
    }

    fn collect_all_tags(&self) -> io::Result<Vec<TagInfo>> {
        let journal = storage::load_journal(self.active_path())?;
        let mut tag_counts: HashMap<String, usize> = HashMap::new();

        for cap in storage::TAG_REGEX.captures_iter(&journal) {
            let tag = cap[1].to_lowercase();
            *tag_counts.entry(tag).or_insert(0) += 1;
        }

        let mut tags: Vec<TagInfo> = tag_counts
            .into_iter()
            .map(|(name, count)| TagInfo { name, count })
            .collect();

        tags.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(tags)
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
            InputMode::Prompt(PromptContext::RenameTag { buffer, .. }) => Some(buffer.content()),
            _ => None,
        }
    }

    /// Check if the current prompt buffer is empty
    #[must_use]
    pub fn prompt_is_empty(&self) -> bool {
        match &self.input_mode {
            InputMode::Prompt(PromptContext::Command { buffer }) => buffer.is_empty(),
            InputMode::Prompt(PromptContext::Filter { buffer }) => buffer.is_empty(),
            InputMode::Prompt(PromptContext::RenameTag { buffer, .. }) => buffer.is_empty(),
            _ => true,
        }
    }

    /// Get a mutable reference to the current prompt buffer (if in prompt mode)
    pub fn prompt_buffer_mut(&mut self) -> Option<&mut CursorBuffer> {
        match &mut self.input_mode {
            InputMode::Prompt(PromptContext::Command { buffer }) => Some(buffer),
            InputMode::Prompt(PromptContext::Filter { buffer }) => Some(buffer),
            InputMode::Prompt(PromptContext::RenameTag { buffer, .. }) => Some(buffer),
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

    pub fn cancel_prompt(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub fn open_project_interface(&mut self) {
        self.input_mode =
            InputMode::Interface(InterfaceContext::Project(ProjectInterfaceState::new()));
    }

    pub fn project_interface_select(&mut self) -> std::io::Result<()> {
        let project_info = {
            let InputMode::Interface(InterfaceContext::Project(ref state)) = self.input_mode else {
                return Ok(());
            };
            state
                .selected_project()
                .map(|p| (p.id.clone(), p.available))
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

    pub fn cancel_interface(&mut self) {
        self.input_mode = InputMode::Normal;
    }
}
