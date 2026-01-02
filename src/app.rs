use std::io;

use chrono::{Datelike, Days, Local, NaiveDate};

use crate::config::{resolve_path, Config};
use crate::cursor::CursorBuffer;
use crate::storage::{self, Entry, EntryType, FilterEntry, LaterEntry, Line};

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
        // Default selection: if there are later entries, start at first later entry (index 0)
        // Otherwise start at last regular entry
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
    pub query_buffer: String,
    pub entries: Vec<FilterEntry>,
    pub selected: usize,
    pub scroll_offset: usize,
}

/// Which view is currently active and its state
#[derive(Clone)]
pub enum ViewMode {
    Daily(DailyState),
    Filter(FilterState),
}

/// Context for what is being edited
#[derive(Clone, PartialEq)]
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

/// What keyboard handler to use
#[derive(Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Edit(EditContext),
    Command,
    Order,
    QueryInput,
}

/// Where to insert a new entry
pub enum InsertPosition {
    Bottom,
    Below,
    Above,
}

pub struct App {
    // Core data (always loaded)
    pub current_date: NaiveDate,
    pub lines: Vec<Line>,
    pub entry_indices: Vec<usize>,

    // View & Input state
    pub view: ViewMode,
    pub input_mode: InputMode,

    // Edit buffer
    pub edit_buffer: Option<CursorBuffer>,

    // Command mode
    pub command_buffer: String,

    // UI state
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub show_help: bool,
    pub help_scroll: usize,
    pub help_visible_height: usize,

    // Undo
    pub last_deleted: Option<(NaiveDate, usize, Entry)>,

    // Filter history
    pub last_filter_query: Option<String>,

    // Config
    pub config: Config,
}

impl App {
    pub fn new(config: Config) -> io::Result<Self> {
        let current_date = Local::now().date_naive();
        let lines = storage::load_day_lines(current_date)?;
        let entry_indices = Self::compute_entry_indices(&lines);
        let later_entries = storage::collect_later_entries_for_date(current_date)?;

        Ok(Self {
            current_date,
            lines,
            entry_indices: entry_indices.clone(),
            view: ViewMode::Daily(DailyState::new(entry_indices.len(), later_entries)),
            input_mode: InputMode::Normal,
            edit_buffer: None,
            command_buffer: String::new(),
            should_quit: false,
            status_message: None,
            show_help: false,
            help_scroll: 0,
            help_visible_height: 0,
            last_deleted: None,
            last_filter_query: None,
            config,
        })
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

    // === Daily Entry Accessors ===

    fn get_daily_entry(&self, entry_index: usize) -> Option<&Entry> {
        let line_idx = self.entry_indices.get(entry_index)?;
        if let Line::Entry(entry) = &self.lines[*line_idx] {
            Some(entry)
        } else {
            None
        }
    }

    fn get_daily_entry_mut(&mut self, entry_index: usize) -> Option<&mut Entry> {
        let line_idx = *self.entry_indices.get(entry_index)?;
        if let Line::Entry(entry) = &mut self.lines[line_idx] {
            Some(entry)
        } else {
            None
        }
    }

    // === Later Entry Helpers ===

    /// Total selectable entries in daily view (later entries + regular entries)
    #[must_use]
    pub fn daily_entry_count(&self) -> usize {
        let ViewMode::Daily(state) = &self.view else {
            return 0;
        };
        state.later_entries.len() + self.entry_indices.len()
    }

    /// Saves current day's lines to storage, displaying any error as a status message.
    pub fn save(&mut self) {
        if let Err(e) = storage::save_day_lines(self.current_date, &self.lines) {
            self.status_message = Some(format!("Failed to save: {e}"));
        }
    }

    // === Navigation (unified) ===

    pub fn move_up(&mut self) {
        match &mut self.view {
            ViewMode::Daily(state) => {
                state.selected = state.selected.saturating_sub(1);
            }
            ViewMode::Filter(state) => {
                state.selected = state.selected.saturating_sub(1);
            }
        }
    }

    pub fn move_down(&mut self) {
        match &mut self.view {
            ViewMode::Daily(state) => {
                let total = state.later_entries.len() + self.entry_indices.len();
                if total > 0 && state.selected < total - 1 {
                    state.selected += 1;
                }
            }
            ViewMode::Filter(state) => {
                if !state.entries.is_empty() && state.selected < state.entries.len() - 1 {
                    state.selected += 1;
                }
            }
        }
    }

    pub fn jump_to_first(&mut self) {
        match &mut self.view {
            ViewMode::Daily(state) => state.selected = 0,
            ViewMode::Filter(state) => state.selected = 0,
        }
    }

    pub fn jump_to_last(&mut self) {
        match &mut self.view {
            ViewMode::Daily(state) => {
                let total = state.later_entries.len() + self.entry_indices.len();
                if total > 0 {
                    state.selected = total - 1;
                }
            }
            ViewMode::Filter(state) => {
                if !state.entries.is_empty() {
                    state.selected = state.entries.len() - 1;
                }
            }
        }
    }

    // === Unified Entry Operations ===

    /// Delete the currently selected entry (view-aware)
    pub fn delete_current_entry(&mut self) -> io::Result<()> {
        match &mut self.view {
            ViewMode::Daily(state) => {
                // Check if a later entry is selected
                if let Some(later_entry) = state.later_entries.get(state.selected).cloned() {
                    // Delete from source day
                    storage::delete_entry(later_entry.source_date, later_entry.line_index)?;

                    // Store for undo
                    let entry = Entry {
                        entry_type: later_entry.entry_type,
                        content: later_entry.content,
                    };
                    self.last_deleted =
                        Some((later_entry.source_date, later_entry.line_index, entry));

                    // Refresh later entries
                    state.later_entries =
                        storage::collect_later_entries_for_date(self.current_date)?;

                    // Adjust selection
                    let total = state.later_entries.len() + self.entry_indices.len();
                    if total > 0 && state.selected >= total {
                        state.selected = total - 1;
                    }
                    return Ok(());
                }

                // Regular entry deletion
                let entry_index = state.selected - state.later_entries.len();
                if entry_index >= self.entry_indices.len() {
                    return Ok(());
                }
                let line_idx = self.entry_indices[entry_index];
                if let Line::Entry(entry) = &self.lines[line_idx] {
                    self.last_deleted = Some((self.current_date, line_idx, entry.clone()));
                }
                self.lines.remove(line_idx);
                self.entry_indices = Self::compute_entry_indices(&self.lines);
                let total = state.later_entries.len() + self.entry_indices.len();
                if total > 0 && state.selected >= total {
                    state.selected = total - 1;
                }
                self.save();
            }
            ViewMode::Filter(state) => {
                let Some(filter_entry) = state.entries.get(state.selected) else {
                    return Ok(());
                };
                let date = filter_entry.source_date;
                let line_index = filter_entry.line_index;
                let entry = Entry {
                    entry_type: filter_entry.entry_type.clone(),
                    content: filter_entry.content.clone(),
                };
                self.last_deleted = Some((date, line_index, entry));

                storage::delete_entry(date, line_index)?;
                state.entries.remove(state.selected);

                // Adjust line indices for remaining entries from the same date
                for filter_entry in &mut state.entries {
                    if filter_entry.source_date == date && filter_entry.line_index > line_index {
                        filter_entry.line_index -= 1;
                    }
                }

                if !state.entries.is_empty() && state.selected >= state.entries.len() {
                    state.selected = state.entries.len() - 1;
                }

                if date == self.current_date {
                    self.reload_current_day()?;
                }
            }
        }
        Ok(())
    }

    /// Toggle task completion (view-aware)
    pub fn toggle_current_entry(&mut self) -> io::Result<()> {
        match &mut self.view {
            ViewMode::Daily(state) => {
                // Check if a later entry is selected
                if let Some(later_entry) = state.later_entries.get(state.selected).cloned() {
                    if !matches!(later_entry.entry_type, EntryType::Task { .. }) {
                        return Ok(());
                    }
                    storage::toggle_entry_complete(
                        later_entry.source_date,
                        later_entry.line_index,
                    )?;
                    // Refresh later entries
                    state.later_entries =
                        storage::collect_later_entries_for_date(self.current_date)?;
                    return Ok(());
                }

                // Regular entry toggle
                let entry_index = state.selected - state.later_entries.len();
                let line_idx = match self.entry_indices.get(entry_index) {
                    Some(&idx) => idx,
                    None => return Ok(()),
                };
                if let Line::Entry(entry) = &mut self.lines[line_idx] {
                    entry.toggle_complete();
                    self.save();
                }
            }
            ViewMode::Filter(state) => {
                let Some(filter_entry) = state.entries.get(state.selected) else {
                    return Ok(());
                };
                if !matches!(filter_entry.entry_type, EntryType::Task { .. }) {
                    return Ok(());
                }
                let date = filter_entry.source_date;
                let line_index = filter_entry.line_index;

                storage::toggle_entry_complete(date, line_index)?;

                let filter_entry = &mut state.entries[state.selected];
                filter_entry.completed = !filter_entry.completed;
                if let EntryType::Task { completed } = &mut filter_entry.entry_type {
                    *completed = filter_entry.completed;
                }

                if date == self.current_date {
                    self.reload_current_day()?;
                }
            }
        }
        Ok(())
    }

    /// Start editing the current entry (view-aware)
    pub fn edit_current_entry(&mut self) {
        match &self.view {
            ViewMode::Daily(state) => {
                // Check if a later entry is selected
                if let Some(later_entry) = state.later_entries.get(state.selected) {
                    let ctx = EditContext::LaterEdit {
                        source_date: later_entry.source_date,
                        line_index: later_entry.line_index,
                        later_index: state.selected,
                    };
                    self.edit_buffer = Some(CursorBuffer::new(later_entry.content.clone()));
                    self.input_mode = InputMode::Edit(ctx);
                    return;
                }

                // Regular entry editing
                let entry_index = state.selected - state.later_entries.len();
                let content = self.get_daily_entry(entry_index).map(|e| e.content.clone());
                if let Some(content) = content {
                    self.edit_buffer = Some(CursorBuffer::new(content));
                    self.input_mode = InputMode::Edit(EditContext::Daily { entry_index });
                }
            }
            ViewMode::Filter(state) => {
                let Some(filter_entry) = state.entries.get(state.selected) else {
                    return;
                };
                let ctx = EditContext::FilterEdit {
                    date: filter_entry.source_date,
                    line_index: filter_entry.line_index,
                    filter_index: state.selected,
                };
                self.edit_buffer = Some(CursorBuffer::new(filter_entry.content.clone()));
                self.input_mode = InputMode::Edit(ctx);
            }
        }
    }

    pub fn yank_current_entry(&mut self) {
        let content = match &self.view {
            ViewMode::Daily(state) => {
                if let Some(later_entry) = state.later_entries.get(state.selected) {
                    later_entry.content.clone()
                } else {
                    let entry_index = state.selected - state.later_entries.len();
                    match self.get_daily_entry(entry_index) {
                        Some(entry) => entry.content.clone(),
                        None => return,
                    }
                }
            }
            ViewMode::Filter(state) => match state.entries.get(state.selected) {
                Some(filter_entry) => filter_entry.content.clone(),
                None => return,
            },
        };

        match Self::copy_to_clipboard(&content) {
            Ok(()) => self.status_message = Some("Yanked".to_string()),
            Err(e) => self.status_message = Some(format!("Failed to yank: {e}")),
        }
    }

    fn copy_to_clipboard(text: &str) -> Result<(), arboard::Error> {
        let mut clipboard = arboard::Clipboard::new()?;
        clipboard.set_text(text)?;
        Ok(())
    }

    // === Edit Mode Operations ===

    /// Cycle entry type while editing (BackTab)
    pub fn cycle_edit_entry_type(&mut self) {
        match &mut self.input_mode {
            InputMode::Edit(EditContext::Daily { entry_index }) => {
                let line_idx = match self.entry_indices.get(*entry_index) {
                    Some(&idx) => idx,
                    None => return,
                };
                if let Line::Entry(entry) = &mut self.lines[line_idx] {
                    entry.entry_type = match entry.entry_type {
                        EntryType::Task { .. } => EntryType::Note,
                        EntryType::Note => EntryType::Event,
                        EntryType::Event => EntryType::Task { completed: false },
                    };
                }
            }
            InputMode::Edit(EditContext::FilterEdit {
                date,
                line_index,
                filter_index,
            }) => {
                let date = *date;
                let line_index = *line_index;
                let filter_index = *filter_index;

                if let Ok(Some(new_type)) = storage::cycle_entry_type(date, line_index)
                    && let ViewMode::Filter(state) = &mut self.view
                    && let Some(filter_entry) = state.entries.get_mut(filter_index)
                {
                    filter_entry.entry_type = new_type;
                    filter_entry.completed =
                        matches!(filter_entry.entry_type, EntryType::Task { completed: true });
                    if date == self.current_date {
                        let _ = self.reload_current_day();
                    }
                }
            }
            InputMode::Edit(EditContext::FilterQuickAdd { entry_type, .. }) => {
                *entry_type = match entry_type {
                    EntryType::Task { .. } => EntryType::Note,
                    EntryType::Note => EntryType::Event,
                    EntryType::Event => EntryType::Task { completed: false },
                };
            }
            InputMode::Edit(EditContext::LaterEdit {
                source_date,
                line_index,
                later_index,
            }) => {
                let source_date = *source_date;
                let line_index = *line_index;
                let later_index = *later_index;

                if let Ok(Some(new_type)) = storage::cycle_entry_type(source_date, line_index)
                    && let ViewMode::Daily(state) = &mut self.view
                    && let Some(later_entry) = state.later_entries.get_mut(later_index)
                {
                    later_entry.entry_type = new_type;
                    later_entry.completed =
                        matches!(later_entry.entry_type, EntryType::Task { completed: true });
                }
            }
            _ => {}
        }
    }

    /// Save and exit edit mode (Enter)
    pub fn exit_edit(&mut self) {
        let Some(buffer) = self.edit_buffer.take() else {
            self.input_mode = InputMode::Normal;
            return;
        };
        let content = buffer.into_content();
        let content = storage::expand_favorite_tags(&content, &self.config.favorite_tags);
        let content = storage::normalize_natural_dates(&content, Local::now().date_naive());

        match std::mem::replace(&mut self.input_mode, InputMode::Normal) {
            InputMode::Edit(EditContext::Daily { entry_index }) => {
                if content.trim().is_empty() {
                    self.delete_at_index_daily(entry_index);
                    if let ViewMode::Daily(state) = &mut self.view {
                        state.scroll_offset = 0;
                    }
                } else if let Some(entry) = self.get_daily_entry_mut(entry_index) {
                    entry.content = content;
                    self.save();
                }
            }
            InputMode::Edit(EditContext::FilterEdit {
                date, line_index, ..
            }) => {
                if content.trim().is_empty() {
                    let _ = storage::delete_entry(date, line_index);
                } else {
                    match storage::update_entry_content(date, line_index, content) {
                        Ok(false) => {
                            self.status_message = Some(format!(
                                "Failed to update: no entry at index {line_index} for {date}"
                            ));
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Failed to save: {e}"));
                        }
                        Ok(true) => {}
                    }
                }
                if date == self.current_date {
                    let _ = self.reload_current_day();
                }
                let _ = self.refresh_filter();
            }
            InputMode::Edit(EditContext::FilterQuickAdd { date, entry_type }) => {
                if !content.trim().is_empty()
                    && let Ok(mut lines) = storage::load_day_lines(date)
                {
                    let entry = Entry {
                        entry_type,
                        content,
                    };
                    lines.push(Line::Entry(entry));
                    let _ = storage::save_day_lines(date, &lines);
                    if date == self.current_date {
                        let _ = self.reload_current_day();
                    }
                }
                let _ = self.refresh_filter();
                if let ViewMode::Filter(state) = &mut self.view {
                    state.selected = state.entries.len().saturating_sub(1);
                }
            }
            InputMode::Edit(EditContext::LaterEdit {
                source_date,
                line_index,
                ..
            }) => {
                if content.trim().is_empty() {
                    let _ = storage::delete_entry(source_date, line_index);
                } else {
                    match storage::update_entry_content(source_date, line_index, content) {
                        Ok(false) => {
                            self.status_message = Some(format!(
                                "Failed to update: no entry at index {line_index} for {source_date}"
                            ));
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Failed to save: {e}"));
                        }
                        Ok(true) => {}
                    }
                }
                // Refresh later entries
                if let ViewMode::Daily(state) = &mut self.view {
                    state.later_entries =
                        storage::collect_later_entries_for_date(self.current_date)
                            .unwrap_or_default();
                }
            }
            _ => {}
        }
    }

    /// Cancel edit mode without saving (Esc)
    pub fn cancel_edit(&mut self) {
        self.edit_buffer = None;

        match std::mem::replace(&mut self.input_mode, InputMode::Normal) {
            InputMode::Edit(EditContext::Daily { entry_index }) => {
                if let Some(entry) = self.get_daily_entry(entry_index)
                    && entry.content.is_empty()
                {
                    self.delete_at_index_daily(entry_index);
                    if let ViewMode::Daily(state) = &mut self.view {
                        state.scroll_offset = 0;
                    }
                }
            }
            InputMode::Edit(EditContext::FilterEdit { .. })
            | InputMode::Edit(EditContext::FilterQuickAdd { .. }) => {
                // Just return to filter view, no cleanup needed
            }
            InputMode::Edit(EditContext::LaterEdit { .. }) => {
                // Just return to daily view, no cleanup needed
            }
            _ => {}
        }
    }

    /// Save and add new entry (Tab)
    pub fn commit_and_add_new(&mut self) {
        let Some(buffer) = self.edit_buffer.take() else {
            return;
        };
        let content = buffer.into_content();

        match std::mem::replace(&mut self.input_mode, InputMode::Normal) {
            InputMode::Edit(EditContext::FilterQuickAdd { date, entry_type }) => {
                if !content.trim().is_empty()
                    && let Ok(mut lines) = storage::load_day_lines(date)
                {
                    let line_index = lines.len();
                    let entry = Entry {
                        entry_type: entry_type.clone(),
                        content: content.clone(),
                    };
                    lines.push(Line::Entry(entry));
                    let _ = storage::save_day_lines(date, &lines);
                    if date == self.current_date {
                        let _ = self.reload_current_day();
                    }

                    // Add to filter entries without refreshing (defer filter until exit)
                    if let ViewMode::Filter(state) = &mut self.view {
                        state.entries.push(FilterEntry {
                            source_date: date,
                            line_index,
                            entry_type: entry_type.clone(),
                            content,
                            completed: matches!(entry_type, EntryType::Task { completed: true }),
                        });
                        state.selected = state.entries.len().saturating_sub(1);
                    }
                }
                self.edit_buffer = Some(CursorBuffer::empty());
                self.input_mode = InputMode::Edit(EditContext::FilterQuickAdd {
                    date,
                    entry_type: match entry_type {
                        EntryType::Task { .. } => EntryType::Task { completed: false },
                        other => other,
                    },
                });
            }
            InputMode::Edit(EditContext::Daily { entry_index }) => {
                if content.trim().is_empty() {
                    let was_at_end = entry_index == self.entry_indices.len().saturating_sub(1);
                    self.delete_at_index_daily(entry_index);
                    if let ViewMode::Daily(state) = &mut self.view
                        && !was_at_end
                        && state.selected > 0
                    {
                        state.selected -= 1;
                    }
                    return;
                }

                let entry_type = self
                    .get_daily_entry(entry_index)
                    .map(|e| e.entry_type.clone())
                    .unwrap_or(EntryType::Task { completed: false });

                if let Some(entry) = self.get_daily_entry_mut(entry_index) {
                    entry.content = content;
                }
                self.save();

                let new_entry = Entry {
                    entry_type: match entry_type {
                        EntryType::Task { .. } => EntryType::Task { completed: false },
                        other => other,
                    },
                    content: String::new(),
                };
                self.add_entry_internal(new_entry, InsertPosition::Below);
            }
            _ => {}
        }
    }

    // === Daily-Only Operations ===

    fn delete_at_index_daily(&mut self, entry_index: usize) {
        if entry_index >= self.entry_indices.len() {
            return;
        }
        let line_idx = self.entry_indices[entry_index];
        if let Line::Entry(entry) = &self.lines[line_idx] {
            self.last_deleted = Some((self.current_date, line_idx, entry.clone()));
        }
        self.lines.remove(line_idx);
        self.entry_indices = Self::compute_entry_indices(&self.lines);

        if let ViewMode::Daily(state) = &mut self.view
            && !self.entry_indices.is_empty()
            && state.selected >= self.entry_indices.len()
        {
            state.selected = self.entry_indices.len() - 1;
        }
    }

    fn add_entry_internal(&mut self, entry: Entry, position: InsertPosition) {
        let ViewMode::Daily(state) = &mut self.view else {
            return;
        };

        let later_count = state.later_entries.len();

        let insert_pos = if matches!(position, InsertPosition::Bottom) || self.entry_indices.is_empty() {
            self.lines.len()
        } else {
            // Convert visual selection to entry index (skip later entries)
            let entry_index = state.selected.saturating_sub(later_count);
            if entry_index < self.entry_indices.len() {
                match position {
                    InsertPosition::Below => self.entry_indices[entry_index] + 1,
                    InsertPosition::Above => self.entry_indices[entry_index],
                    InsertPosition::Bottom => unreachable!(),
                }
            } else {
                self.lines.len()
            }
        };

        self.lines.insert(insert_pos, Line::Entry(entry));
        self.entry_indices = Self::compute_entry_indices(&self.lines);

        let entry_index = self
            .entry_indices
            .iter()
            .position(|&idx| idx == insert_pos)
            .unwrap_or(self.entry_indices.len().saturating_sub(1));

        // Visual selection includes later entries offset
        state.selected = later_count + entry_index;

        self.edit_buffer = Some(CursorBuffer::empty());
        self.input_mode = InputMode::Edit(EditContext::Daily { entry_index });
    }

    pub fn new_task(&mut self, position: InsertPosition) {
        self.add_entry_internal(Entry::new_task(""), position);
    }

    pub fn undo(&mut self) {
        let Some((date, line_idx, entry)) = self.last_deleted.take() else {
            return;
        };

        match &mut self.view {
            ViewMode::Daily(state) => {
                // Only undo if we're on the same day
                if date != self.current_date {
                    self.status_message = Some(format!(
                        "Undo: entry was from {}, go to that day first",
                        date.format("%m/%d")
                    ));
                    self.last_deleted = Some((date, line_idx, entry));
                    return;
                }
                let insert_idx = line_idx.min(self.lines.len());
                self.lines.insert(insert_idx, Line::Entry(entry));
                self.entry_indices = Self::compute_entry_indices(&self.lines);
                if let Some(pos) = self.entry_indices.iter().position(|&i| i == insert_idx) {
                    state.selected = pos;
                }
                self.save();
            }
            ViewMode::Filter(state) => {
                if let Ok(mut lines) = storage::load_day_lines(date) {
                    let insert_idx = line_idx.min(lines.len());
                    lines.insert(insert_idx, Line::Entry(entry.clone()));
                    let _ = storage::save_day_lines(date, &lines);

                    let filter_entry = FilterEntry {
                        source_date: date,
                        line_index: insert_idx,
                        entry_type: entry.entry_type.clone(),
                        content: entry.content,
                        completed: matches!(entry.entry_type, EntryType::Task { completed: true }),
                    };
                    state.entries.push(filter_entry);
                    state.selected = state.entries.len() - 1;

                    if date == self.current_date {
                        let _ = self.reload_current_day();
                    }
                }
            }
        }
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

    pub fn enter_order_mode(&mut self) {
        let ViewMode::Daily(state) = &mut self.view else {
            return;
        };

        // Block order mode if a later entry is selected
        if state.selected < state.later_entries.len() {
            self.status_message = Some("Cannot reorder later entries".to_string());
            return;
        }

        if !self.entry_indices.is_empty() {
            state.original_lines = Some(self.lines.clone());
            self.input_mode = InputMode::Order;
        }
    }

    pub fn exit_order_mode(&mut self, save: bool) {
        if !matches!(self.view, ViewMode::Daily(_)) {
            return;
        }
        if save {
            self.save();
        } else if let ViewMode::Daily(state) = &mut self.view
            && let Some(original) = state.original_lines.take()
        {
            self.lines = original;
            self.entry_indices = Self::compute_entry_indices(&self.lines);
        }
        if let ViewMode::Daily(state) = &mut self.view {
            state.original_lines = None;
        }
        self.input_mode = InputMode::Normal;
    }

    pub fn order_move_up(&mut self) {
        let ViewMode::Daily(state) = &mut self.view else {
            return;
        };

        // Convert to entry index (skip later entries)
        let later_count = state.later_entries.len();
        if state.selected < later_count {
            return; // Shouldn't happen, but protect against it
        }
        let entry_index = state.selected - later_count;

        // Can't move above first entry (but can be at first entry position)
        if entry_index == 0 {
            return;
        }

        let curr_line_idx = self.entry_indices[entry_index];
        let prev_line_idx = self.entry_indices[entry_index - 1];
        self.lines.swap(curr_line_idx, prev_line_idx);
        self.entry_indices = Self::compute_entry_indices(&self.lines);
        state.selected -= 1;
    }

    pub fn order_move_down(&mut self) {
        let ViewMode::Daily(state) = &mut self.view else {
            return;
        };

        // Convert to entry index (skip later entries)
        let later_count = state.later_entries.len();
        if state.selected < later_count {
            return; // Shouldn't happen, but protect against it
        }
        let entry_index = state.selected - later_count;

        if entry_index >= self.entry_indices.len() - 1 {
            return;
        }

        let curr_line_idx = self.entry_indices[entry_index];
        let next_line_idx = self.entry_indices[entry_index + 1];
        self.lines.swap(curr_line_idx, next_line_idx);
        self.entry_indices = Self::compute_entry_indices(&self.lines);
        state.selected += 1;
    }

    // === Day Navigation (Daily only) ===

    /// Load a day's data into self, returning later entries for view construction.
    fn load_day(&mut self, date: NaiveDate) -> io::Result<Vec<LaterEntry>> {
        self.current_date = date;
        self.lines = storage::load_day_lines(date)?;
        self.entry_indices = Self::compute_entry_indices(&self.lines);
        storage::collect_later_entries_for_date(date)
    }

    pub fn goto_day(&mut self, date: NaiveDate) -> io::Result<()> {
        if date == self.current_date {
            return Ok(());
        }

        self.save();
        let later_entries = self.load_day(date)?;
        self.edit_buffer = None;
        self.view = ViewMode::Daily(DailyState::new(self.entry_indices.len(), later_entries));
        self.input_mode = InputMode::Normal;

        Ok(())
    }

    pub fn prev_day(&mut self) -> io::Result<()> {
        if let Some(prev) = self.current_date.checked_sub_days(Days::new(1)) {
            self.goto_day(prev)?;
        }
        Ok(())
    }

    pub fn next_day(&mut self) -> io::Result<()> {
        if let Some(next) = self.current_date.checked_add_days(Days::new(1)) {
            self.goto_day(next)?;
        }
        Ok(())
    }

    pub fn goto_today(&mut self) -> io::Result<()> {
        self.goto_day(Local::now().date_naive())
    }

    #[must_use]
    pub fn parse_goto_date(input: &str) -> Option<NaiveDate> {
        // YYYY/MM/DD (only if first part is exactly 4 digits)
        if let Some(first_slash) = input.find('/')
            && first_slash == 4
            && input[..4].chars().all(|c| c.is_ascii_digit())
            && let Ok(date) = NaiveDate::parse_from_str(input, "%Y/%m/%d")
        {
            return Some(date);
        }

        let parts: Vec<&str> = input.split('/').collect();

        // MM/DD/YYYY or MM/DD/YY
        if parts.len() == 3
            && let (Ok(month), Ok(day), Ok(year)) = (
                parts[0].parse::<u32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<i32>(),
            )
        {
            let full_year = if year < 100 { 2000 + year } else { year };
            return NaiveDate::from_ymd_opt(full_year, month, day);
        }

        // MM/DD (always future - if date passed this year, use next year)
        if parts.len() == 2
            && let (Ok(month), Ok(day)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>())
        {
            let today = Local::now().date_naive();
            if let Some(date) = NaiveDate::from_ymd_opt(today.year(), month, day) {
                if date >= today {
                    return Some(date);
                }
                return NaiveDate::from_ymd_opt(today.year() + 1, month, day);
            }
        }

        None
    }

    pub fn open_journal(&mut self, path: &str) -> io::Result<()> {
        self.save();

        let path = resolve_path(path);
        storage::set_journal_path(path.clone());
        let later_entries = self.load_day(Local::now().date_naive())?;
        self.view = ViewMode::Daily(DailyState::new(self.entry_indices.len(), later_entries));
        self.status_message = Some(format!("Opened: {}", path.display()));
        Ok(())
    }

    pub fn reload_config(&mut self) -> io::Result<()> {
        self.config = Config::load()?;
        self.status_message = Some("Config reloaded".to_string());
        Ok(())
    }

    // === Command Mode ===

    pub fn execute_command(&mut self) -> io::Result<()> {
        let cmd = std::mem::take(&mut self.command_buffer);
        let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
        let command = parts.first().copied().unwrap_or("");
        let arg = parts.get(1).copied().unwrap_or("").trim();

        match command {
            "q" | "quit" => {
                self.save();
                self.should_quit = true;
            }
            "goto" | "g" => {
                if arg.is_empty() {
                    self.status_message =
                        Some("Usage: :goto YYYY/MM/DD or :goto MM/DD".to_string());
                } else if let Some(date) = Self::parse_goto_date(arg) {
                    self.goto_day(date)?;
                } else {
                    self.status_message = Some(format!("Invalid date: {arg}"));
                }
            }
            "o" | "open" => {
                if arg.is_empty() {
                    self.status_message = Some("Usage: :open /path/to/file.md".to_string());
                } else {
                    self.open_journal(arg)?;
                }
            }
            "config-reload" => {
                self.reload_config()?;
            }
            _ => {}
        }
        self.input_mode = InputMode::Normal;
        Ok(())
    }

    // === Filter Operations ===

    pub fn enter_filter_input(&mut self) {
        match &mut self.view {
            ViewMode::Filter(state) => {
                // Pre-fill with current query for modification
                state.query_buffer = state.query.clone();
            }
            ViewMode::Daily(_) => {
                // Use command_buffer temporarily, ensure it's clean
                self.command_buffer.clear();
            }
        }

        self.input_mode = InputMode::QueryInput;
    }

    pub fn execute_filter(&mut self) -> io::Result<()> {
        self.save();

        let query = match &self.view {
            ViewMode::Filter(state) => state.query_buffer.clone(),
            ViewMode::Daily(_) => std::mem::take(&mut self.command_buffer),
        };

        let (query, unknown_filters) = storage::expand_saved_filters(&query, &self.config.filters);
        let mut filter = storage::parse_filter_query(&query);
        filter.invalid_tokens.extend(unknown_filters);

        if !filter.invalid_tokens.is_empty() {
            self.status_message = Some(format!(
                "Unknown filter: {}",
                filter.invalid_tokens.join(", ")
            ));
        }

        let entries = storage::collect_filtered_entries(&filter)?;
        let selected = entries.len().saturating_sub(1);

        self.view = ViewMode::Filter(FilterState {
            query,
            query_buffer: String::new(),
            entries,
            selected,
            scroll_offset: 0,
        });
        self.input_mode = InputMode::Normal;
        Ok(())
    }

    pub fn quick_filter(&mut self, query: &str) -> io::Result<()> {
        self.save();
        let (query, unknown_filters) = storage::expand_saved_filters(query, &self.config.filters);
        let mut filter = storage::parse_filter_query(&query);
        filter.invalid_tokens.extend(unknown_filters);

        if !filter.invalid_tokens.is_empty() {
            self.status_message = Some(format!(
                "Unknown filter: {}",
                filter.invalid_tokens.join(", ")
            ));
        }

        let entries = storage::collect_filtered_entries(&filter)?;
        let selected = entries.len().saturating_sub(1);

        self.view = ViewMode::Filter(FilterState {
            query,
            query_buffer: String::new(),
            entries,
            selected,
            scroll_offset: 0,
        });
        self.input_mode = InputMode::Normal;
        Ok(())
    }

    pub fn cancel_filter_input(&mut self) {
        match &mut self.view {
            ViewMode::Filter(state) => {
                state.query_buffer.clear();
            }
            ViewMode::Daily(_) => {
                self.command_buffer.clear();
            }
        }
        self.input_mode = InputMode::Normal;
    }

    pub fn exit_filter(&mut self) {
        if let ViewMode::Filter(state) = &self.view {
            self.last_filter_query = Some(state.query.clone());
        }
        let later_entries =
            storage::collect_later_entries_for_date(self.current_date).unwrap_or_default();
        self.view = ViewMode::Daily(DailyState::new(self.entry_indices.len(), later_entries));
        self.input_mode = InputMode::Normal;
    }

    pub fn return_to_filter(&mut self) -> io::Result<()> {
        let query = self
            .last_filter_query
            .clone()
            .unwrap_or_else(|| self.config.default_filter.clone());
        self.quick_filter(&query)
    }

    pub fn refresh_filter(&mut self) -> io::Result<()> {
        let ViewMode::Filter(state) = &mut self.view else {
            return Ok(());
        };

        let filter = storage::parse_filter_query(&state.query);

        if !filter.invalid_tokens.is_empty() {
            self.status_message = Some(format!(
                "Unknown filter: {}",
                filter.invalid_tokens.join(", ")
            ));
        }

        state.entries = storage::collect_filtered_entries(&filter)?;
        state.selected = state.selected.min(state.entries.len().saturating_sub(1));
        state.scroll_offset = 0;
        Ok(())
    }

    /// Navigate to a specific day and select the entry at the given line index
    fn goto_entry_source(&mut self, date: NaiveDate, line_index: usize) -> io::Result<()> {
        if let ViewMode::Filter(state) = &self.view {
            self.last_filter_query = Some(state.query.clone());
        }
        if date != self.current_date {
            self.save();
        }

        let later_entries = self.load_day(date)?;
        let later_count = later_entries.len();

        // Find entry position and add later_count offset
        let entry_pos = self
            .entry_indices
            .iter()
            .position(|&i| i == line_index)
            .unwrap_or(0);
        let selected = later_count + entry_pos;

        self.view = ViewMode::Daily(DailyState {
            selected,
            scroll_offset: 0,
            original_lines: None,
            later_entries,
        });
        self.input_mode = InputMode::Normal;
        self.edit_buffer = None;

        Ok(())
    }

    /// View the source day of the currently selected entry (unified across views)
    /// - Filter view: jumps to the source day of the selected filter entry
    /// - Daily view with later entry selected: jumps to the source day
    /// - Daily view with regular entry: no-op (already on source day)
    pub fn view_entry_source(&mut self) -> io::Result<()> {
        match &self.view {
            ViewMode::Filter(state) => {
                let Some(filter_entry) = state.entries.get(state.selected) else {
                    return Ok(());
                };
                self.goto_entry_source(filter_entry.source_date, filter_entry.line_index)
            }
            ViewMode::Daily(state) => {
                // Only works for later entries (regular entries are already on their source day)
                let Some(later_entry) = state.later_entries.get(state.selected) else {
                    return Ok(());
                };
                self.goto_entry_source(later_entry.source_date, later_entry.line_index)
            }
        }
    }

    pub fn filter_quick_add(&mut self) {
        let today = Local::now().date_naive();
        self.edit_buffer = Some(CursorBuffer::empty());
        self.input_mode = InputMode::Edit(EditContext::FilterQuickAdd {
            date: today,
            entry_type: EntryType::Task { completed: false },
        });
    }

    // === Filter View Helpers ===

    #[must_use]
    pub fn filter_visual_line(&self) -> usize {
        let ViewMode::Filter(state) = &self.view else {
            return 0;
        };
        // +1 for the "Filter: {query}" header line
        state.selected + 1
    }

    #[must_use]
    pub fn filter_total_lines(&self) -> usize {
        let ViewMode::Filter(state) = &self.view else {
            return 1;
        };
        // +1 for the "Filter: {query}" header line
        state.entries.len() + 1
    }

    fn reload_current_day(&mut self) -> io::Result<()> {
        self.lines = storage::load_day_lines(self.current_date)?;
        self.entry_indices = Self::compute_entry_indices(&self.lines);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_goto_date() {
        // YYYY/MM/DD
        let result = App::parse_goto_date("2025/12/30");
        assert_eq!(result, NaiveDate::from_ymd_opt(2025, 12, 30));

        // MM/DD (assumes current year)
        let result = App::parse_goto_date("12/30");
        assert!(result.is_some(), "12/30 should parse");

        // MM/DD/YYYY
        let result = App::parse_goto_date("12/30/2025");
        assert_eq!(result, NaiveDate::from_ymd_opt(2025, 12, 30));

        // MM/DD/YY (assumes 2000s)
        let result = App::parse_goto_date("12/30/25");
        assert_eq!(result, NaiveDate::from_ymd_opt(2025, 12, 30));

        let result = App::parse_goto_date("1/1/26");
        assert_eq!(result, NaiveDate::from_ymd_opt(2026, 1, 1));

        // Reject ambiguous short "year" that would parse as year 1
        assert!(
            App::parse_goto_date("1/1/26").unwrap().year() >= 2000,
            "1/1/26 should not parse as year 1"
        );
    }

    #[test]
    fn test_command_parsing() {
        let cmd = "goto 12/30";
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        assert_eq!(parts[0], "goto");
        assert_eq!(parts[1], "12/30");

        let cmd = "g 12/30/2025";
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        assert_eq!(parts[0], "g");
        assert_eq!(parts[1], "12/30/2025");
    }
}
