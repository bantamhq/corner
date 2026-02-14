use std::collections::HashMap;
use std::io;

use chrono::NaiveDate;

use crate::app::{App, DeleteTarget, EntryLocation, ViewMode};
use crate::storage::{self, Entry, EntryType, Line};

use super::types::{Action, ActionDescription, StatusVisibility};

fn pluralize(count: usize) -> &'static str {
    if count == 1 { "entry" } else { "entries" }
}

pub struct DeleteEntries {
    pub targets: Vec<DeleteTarget>,
}

impl DeleteEntries {
    #[must_use]
    pub fn new(targets: Vec<DeleteTarget>) -> Self {
        Self { targets }
    }

    #[must_use]
    pub fn single(target: DeleteTarget) -> Self {
        Self {
            targets: vec![target],
        }
    }
}

impl Action for DeleteEntries {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let mut deleted_entries = Vec::new();

        // Sort targets by line index descending for safe deletion
        self.targets.sort_by(|a, b| {
            let idx_a = match a {
                DeleteTarget::Daily { line_idx, .. } => *line_idx,
                DeleteTarget::Projected(entry) => entry.line_index,
                DeleteTarget::Filter { entry, .. } => entry.line_index,
            };
            let idx_b = match b {
                DeleteTarget::Daily { line_idx, .. } => *line_idx,
                DeleteTarget::Projected(entry) => entry.line_index,
                DeleteTarget::Filter { entry, .. } => entry.line_index,
            };
            idx_b.cmp(&idx_a)
        });

        for target in &self.targets {
            let entry_data = execute_delete_raw(app, target)?;
            deleted_entries.push(entry_data);
        }

        // Remove from state.entries for Filter targets in descending index order.
        // This must happen after storage operations to avoid index shifting issues.
        if let ViewMode::Filter(state) = &mut app.view {
            let mut filter_indices: Vec<usize> = self
                .targets
                .iter()
                .filter_map(|t| match t {
                    DeleteTarget::Filter { index, .. } => Some(*index),
                    _ => None,
                })
                .collect();
            filter_indices.sort_by(|a, b| b.cmp(a)); // descending

            for index in filter_indices {
                if index < state.entries.len() {
                    state.entries.remove(index);
                }
            }

            if !state.entries.is_empty() && state.selected >= state.entries.len() {
                state.selected = state.entries.len().saturating_sub(1);
            }
        }

        // Reverse so entries are in ascending order for restore
        deleted_entries.reverse();

        Ok(Box::new(RestoreEntries {
            entries: deleted_entries,
        }))
    }

    fn description(&self) -> ActionDescription {
        let count = self.targets.len();
        ActionDescription::always(
            format!("Deleted {}", pluralize(count)),
            format!("Restored {}", pluralize(count)),
        )
    }
}

pub struct RestoreEntries {
    entries: Vec<(NaiveDate, usize, Entry)>,
}

impl Action for RestoreEntries {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let mut delete_targets = Vec::new();

        // Sort by line index ascending for correct insertion order
        self.entries.sort_by_key(|(_, line_idx, _)| *line_idx);

        if app.combined_view {
            for (_date, _line_idx, entry) in &self.entries {
                let path = &entry.source_journal;
                if let Ok(mut lines) = storage::load_day_lines(entry.source_date, path) {
                    let insert_idx = entry.line_index.min(lines.len());
                    lines.insert(insert_idx, Line::Entry(entry.to_raw()));
                    let _ = storage::save_day_lines(entry.source_date, path, &lines);
                    delete_targets.push(DeleteTarget::Daily {
                        line_idx: insert_idx,
                        entry: entry.clone(),
                    });
                }
            }
            let _ = app.load_combined_data();
            clamp_daily_selection(app);
        } else {
            match &app.view {
            ViewMode::Daily(_) => {
                let (current_day_entries, other_day_entries): (Vec<_>, Vec<_>) = self
                    .entries
                    .iter()
                    .cloned()
                    .partition(|(date, _, _)| *date == app.current_date);

                if !current_day_entries.is_empty() {
                    let mut any_completed = false;
                    let mut last_insert_idx = 0;

                    for (i, (_date, line_idx, entry)) in current_day_entries.iter().enumerate() {
                        let insert_idx = (line_idx + i).min(app.lines.len());
                        if matches!(entry.entry_type, EntryType::Task { completed: true }) {
                            any_completed = true;
                        }

                        delete_targets.push(DeleteTarget::Daily {
                            line_idx: insert_idx,
                            entry: entry.clone(),
                        });

                        app.lines.insert(insert_idx, Line::Entry(entry.to_raw()));
                        last_insert_idx = insert_idx;
                    }

                    app.entry_indices = App::compute_entry_indices(&app.lines);

                    if app.hide_completed && any_completed {
                        app.hide_completed = false;
                    }

                    let visible_idx = app
                        .entry_indices
                        .iter()
                        .position(|&i| i == last_insert_idx)
                        .map(|actual_idx| app.actual_to_visible_index(actual_idx));

                    if let ViewMode::Daily(state) = &mut app.view
                        && let Some(idx) = visible_idx
                    {
                        state.selected = idx;
                    }
                    app.save();
                }

                if !other_day_entries.is_empty() {
                    let path = app.active_path().to_path_buf();

                    // Group by date for efficient file operations
                    let mut entries_by_date: HashMap<NaiveDate, Vec<(usize, Entry)>> =
                        HashMap::new();
                    for (date, line_idx, entry) in &other_day_entries {
                        entries_by_date
                            .entry(*date)
                            .or_default()
                            .push((*line_idx, entry.clone()));
                    }

                    for (date, date_entries) in entries_by_date {
                        if let Ok(mut lines) = storage::load_day_lines(date, &path) {
                            for (i, (line_idx, entry)) in date_entries.into_iter().enumerate() {
                                let insert_idx = (line_idx + i).min(lines.len());
                                lines.insert(insert_idx, Line::Entry(entry.to_raw()));

                                delete_targets.push(DeleteTarget::Projected(entry));
                            }
                            let _ = storage::save_day_lines(date, &path, &lines);
                        }
                    }

                    app.refresh_projected_entries();
                }
            }
            ViewMode::Filter(_) => {
                let path = app.active_path().to_path_buf();

                let mut entries_by_date: HashMap<NaiveDate, Vec<(usize, Entry)>> = HashMap::new();
                for (date, line_idx, entry) in &self.entries {
                    entries_by_date
                        .entry(*date)
                        .or_default()
                        .push((*line_idx, entry.clone()));
                }

                for (date, date_entries) in entries_by_date {
                    if let Ok(mut lines) = storage::load_day_lines(date, &path) {
                        for (i, (line_idx, entry)) in date_entries.into_iter().enumerate() {
                            let insert_idx = (line_idx + i).min(lines.len());

                            let restored_entry = Entry {
                                entry_type: entry.entry_type.clone(),
                                content: entry.content.clone(),
                                source_date: date,
                                line_index: insert_idx,
                                source_type: entry.source_type.clone(),
                                source_journal: entry.source_journal.clone(),
                            };
                            lines.insert(insert_idx, Line::Entry(entry.to_raw()));

                            if let ViewMode::Filter(state) = &mut app.view {
                                let filter_index = state.entries.len();
                                state.entries.push(restored_entry.clone());
                                state.selected = filter_index;

                                delete_targets.push(DeleteTarget::Filter {
                                    index: filter_index,
                                    entry: restored_entry,
                                });
                            }
                        }
                        let _ = storage::save_day_lines(date, &path, &lines);

                        if date == app.current_date {
                            let _ = app.reload_current_day();
                        }
                    }
                }
            }
        }
        }

        Ok(Box::new(DeleteEntries {
            targets: delete_targets,
        }))
    }

    fn description(&self) -> ActionDescription {
        let count = self.entries.len();
        ActionDescription::always(
            format!("Restored {}", pluralize(count)),
            format!("Deleted {}", pluralize(count)),
        )
    }
}

/// Execute a single delete without modifying undo state
fn execute_delete_raw(
    app: &mut App,
    target: &DeleteTarget,
) -> io::Result<(NaiveDate, usize, Entry)> {
    let path = resolve_delete_path(app, target);

    match target {
        DeleteTarget::Projected(entry) => {
            storage::delete_entry(entry.source_date, &path, entry.line_index)?;

            if app.combined_view {
                let _ = app.load_combined_data();
            } else {
                app.refresh_projected_entries();
            }
            clamp_daily_selection(app);

            Ok((entry.source_date, entry.line_index, entry.clone()))
        }
        DeleteTarget::Daily { line_idx, entry } => {
            if app.combined_view {
                storage::delete_entry(app.current_date, &path, *line_idx)?;
                let _ = app.load_combined_data();
                clamp_daily_selection(app);
                return Ok((app.current_date, *line_idx, entry.clone()));
            }

            let result = (app.current_date, *line_idx, entry.clone());
            app.lines.remove(*line_idx);
            app.entry_indices = App::compute_entry_indices(&app.lines);
            clamp_daily_selection(app);
            app.save();
            // Refresh projected entries in case we deleted a ↺ entry that was hiding a recurring
            app.refresh_projected_entries();
            Ok(result)
        }
        DeleteTarget::Filter { entry, .. } => {
            storage::delete_entry(entry.source_date, &path, entry.line_index)?;

            // Adjust line_index for remaining entries from the same date.
            // state.entries removal happens in DeleteEntries::execute to handle
            // index ordering correctly when deleting multiple entries.
            if let ViewMode::Filter(state) = &mut app.view {
                for filter_entry in &mut state.entries {
                    if filter_entry.source_date == entry.source_date
                        && filter_entry.line_index > entry.line_index
                    {
                        filter_entry.line_index -= 1;
                    }
                }
            }

            if entry.source_date == app.current_date {
                app.reload_current_day()?;
            }

            Ok((entry.source_date, entry.line_index, entry.clone()))
        }
    }
}

fn resolve_delete_path(app: &App, target: &DeleteTarget) -> std::path::PathBuf {
    if !app.combined_view {
        return app.active_path().to_path_buf();
    }
    match target {
        DeleteTarget::Projected(entry)
        | DeleteTarget::Filter { entry, .. }
        | DeleteTarget::Daily { entry, .. } => entry.source_journal.clone(),
    }
}

fn clamp_daily_selection(app: &mut App) {
    let visible = app.visible_entry_count();
    if let ViewMode::Daily(state) = &mut app.view
        && visible > 0
        && state.selected >= visible
    {
        state.selected = visible - 1;
    }
}

#[derive(Clone)]
pub struct CreateTarget {
    pub date: NaiveDate,
    pub line_index: usize,
    pub entry: Entry,
    pub is_filter_quick_add: bool,
}

pub struct CreateEntry {
    target: CreateTarget,
}

impl CreateEntry {
    #[must_use]
    pub fn new(target: CreateTarget) -> Self {
        Self { target }
    }
}

impl Action for CreateEntry {
    fn execute(&mut self, _app: &mut App) -> io::Result<Box<dyn Action>> {
        // Entry was already created when this action was made,
        // just return the reverse action
        Ok(Box::new(UncreateEntry::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        ActionDescription {
            past: "Created entry".to_string(),
            past_reversed: "Removed entry".to_string(),
            visibility: StatusVisibility::OnUndo,
        }
    }
}

struct UncreateEntry {
    target: CreateTarget,
}

impl UncreateEntry {
    fn new(target: CreateTarget) -> Self {
        Self { target }
    }
}

impl Action for UncreateEntry {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let path = app.active_path().to_path_buf();

        // Delete the entry
        storage::delete_entry(self.target.date, &path, self.target.line_index)?;

        // Update app state
        if self.target.date == app.current_date {
            app.reload_current_day()?;
            app.clamp_selection_to_visible();
        }

        if self.target.is_filter_quick_add
            && let ViewMode::Filter(state) = &mut app.view
        {
            // Remove from filter entries and adjust indices
            state.entries.retain(|e| {
                !(e.source_date == self.target.date && e.line_index == self.target.line_index)
            });
            for entry in &mut state.entries {
                if entry.source_date == self.target.date
                    && entry.line_index > self.target.line_index
                {
                    entry.line_index -= 1;
                }
            }
            if !state.entries.is_empty() && state.selected >= state.entries.len() {
                state.selected = state.entries.len() - 1;
            }
        }

        // Return action to redo the create
        Ok(Box::new(RecreateEntry::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        ActionDescription {
            past: "Removed entry".to_string(),
            past_reversed: "Created entry".to_string(),
            visibility: StatusVisibility::OnUndo,
        }
    }
}

struct RecreateEntry {
    target: CreateTarget,
}

impl RecreateEntry {
    fn new(target: CreateTarget) -> Self {
        Self { target }
    }
}

impl Action for RecreateEntry {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let path = app.active_path().to_path_buf();

        // Recreate the entry at the original position
        let mut lines = storage::load_day_lines(self.target.date, &path)?;

        // Insert at original position (or end if position is beyond current length)
        let insert_pos = self.target.line_index.min(lines.len());
        lines.insert(insert_pos, Line::Entry(self.target.entry.to_raw()));

        storage::save_day_lines(self.target.date, &path, &lines)?;

        // Update app state
        if self.target.date == app.current_date {
            app.reload_current_day()?;
        }

        if self.target.is_filter_quick_add {
            // Refresh filter to pick up the recreated entry
            let _ = app.refresh_filter();
        }

        // Return action to undo again
        Ok(Box::new(UncreateEntry::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        ActionDescription {
            past: "Created entry".to_string(),
            past_reversed: "Removed entry".to_string(),
            visibility: StatusVisibility::OnUndo,
        }
    }
}

#[derive(Clone)]
pub struct EditTarget {
    pub location: EntryLocation,
    pub original_content: String,
    pub new_content: String,
    pub entry_type: EntryType,
}

pub struct EditEntry {
    target: EditTarget,
}

impl EditEntry {
    #[must_use]
    pub fn new(target: EditTarget) -> Self {
        Self { target }
    }
}

impl Action for EditEntry {
    fn execute(&mut self, _app: &mut App) -> io::Result<Box<dyn Action>> {
        // Content was already saved when this action was created,
        // so just return the reverse action
        Ok(Box::new(RestoreEdit::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        ActionDescription {
            past: "Edited entry".to_string(),
            past_reversed: "Reverted edit".to_string(),
            visibility: StatusVisibility::OnUndo,
        }
    }
}

struct RestoreEdit {
    target: EditTarget,
}

impl RestoreEdit {
    fn new(target: EditTarget) -> Self {
        Self { target }
    }
}

impl Action for RestoreEdit {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        // Restore original content
        set_entry_content_raw(app, &self.target)?;

        // Return action to redo the edit
        Ok(Box::new(RedoEdit::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        ActionDescription {
            past: "Reverted edit".to_string(),
            past_reversed: "Edited entry".to_string(),
            visibility: StatusVisibility::OnUndo,
        }
    }
}

struct RedoEdit {
    target: EditTarget,
}

impl RedoEdit {
    fn new(target: EditTarget) -> Self {
        Self { target }
    }
}

impl Action for RedoEdit {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        // Restore new content
        let mut redo_target = self.target.clone();
        std::mem::swap(
            &mut redo_target.original_content,
            &mut redo_target.new_content,
        );
        set_entry_content_raw(app, &redo_target)?;

        // Return action to undo again
        Ok(Box::new(RestoreEdit::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        ActionDescription {
            past: "Edited entry".to_string(),
            past_reversed: "Reverted edit".to_string(),
            visibility: StatusVisibility::OnUndo,
        }
    }
}

fn set_entry_content_raw(app: &mut App, target: &EditTarget) -> io::Result<()> {
    let path = app.resolve_entry_path(&target.location);
    let content = &target.original_content;

    match &target.location {
        EntryLocation::Projected(entry) => {
            storage::mutate_entry(entry.source_date, &path, entry.line_index, |raw_entry| {
                raw_entry.content = content.clone();
            })?;
            app.refresh_projected_entries();
        }
        EntryLocation::Daily {
            line_idx,
            source_path,
        } => {
            if source_path.is_some() || app.combined_view {
                storage::update_entry_content(app.current_date, &path, *line_idx, content.clone())?;
                let _ = app.load_combined_data();
            } else if let Line::Entry(raw_entry) = &mut app.lines[*line_idx] {
                raw_entry.content = content.clone();
                app.save();
            }
        }
        EntryLocation::Filter { index, entry } => {
            storage::mutate_entry(entry.source_date, &path, entry.line_index, |raw_entry| {
                raw_entry.content = content.clone();
            })?;

            if let ViewMode::Filter(state) = &mut app.view
                && let Some(filter_entry) = state.entries.get_mut(*index)
            {
                filter_entry.content = content.clone();
            }

            if entry.source_date == app.current_date {
                app.reload_current_day()?;
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct CycleTarget {
    pub location: EntryLocation,
    pub original_type: EntryType,
}

pub struct CycleEntryType {
    targets: Vec<CycleTarget>,
}

impl CycleEntryType {
    #[must_use]
    pub fn new(targets: Vec<CycleTarget>) -> Self {
        Self { targets }
    }

    #[must_use]
    pub fn single(location: EntryLocation, original_type: EntryType) -> Self {
        Self::new(vec![CycleTarget {
            location,
            original_type,
        }])
    }
}

impl Action for CycleEntryType {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        for target in &self.targets {
            execute_cycle_raw(app, &target.location)?;
        }

        Ok(Box::new(RestoreEntryType::new(self.targets.clone())))
    }

    fn description(&self) -> ActionDescription {
        let count = self.targets.len();
        if count == 1 {
            ActionDescription {
                past: "Cycled entry type".to_string(),
                past_reversed: "Restored entry type".to_string(),
                visibility: StatusVisibility::Silent,
            }
        } else {
            ActionDescription {
                past: format!("Cycled type on {} entries", count),
                past_reversed: format!("Restored type on {} entries", count),
                visibility: StatusVisibility::Silent,
            }
        }
    }
}

struct RestoreEntryType {
    original_targets: Vec<CycleTarget>,
}

impl RestoreEntryType {
    fn new(original_targets: Vec<CycleTarget>) -> Self {
        Self { original_targets }
    }
}

impl Action for RestoreEntryType {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        // Restore each entry to its original type
        for target in &self.original_targets {
            set_entry_type_raw(app, &target.location, &target.original_type)?;
        }

        // Return an action that will re-cycle (redo)
        Ok(Box::new(CycleEntryType::new(self.original_targets.clone())))
    }

    fn description(&self) -> ActionDescription {
        let count = self.original_targets.len();
        if count == 1 {
            ActionDescription {
                past: "Restored entry type".to_string(),
                past_reversed: "Cycled entry type".to_string(),
                visibility: StatusVisibility::Silent,
            }
        } else {
            ActionDescription {
                past: format!("Restored type on {} entries", count),
                past_reversed: format!("Cycled type on {} entries", count),
                visibility: StatusVisibility::Silent,
            }
        }
    }
}

fn execute_cycle_raw(app: &mut App, location: &EntryLocation) -> io::Result<Option<EntryType>> {
    let path = app.resolve_entry_path(location);

    match location {
        EntryLocation::Projected(entry) => {
            let new_type = storage::cycle_entry_type(entry.source_date, &path, entry.line_index)?;
            if let Some(ref new_type) = new_type
                && let ViewMode::Daily(state) = &mut app.view
                && let Some(projected_entry) = state.projected_entries.iter_mut().find(|e| {
                    e.source_date == entry.source_date && e.line_index == entry.line_index
                })
            {
                projected_entry.entry_type = new_type.clone();
            }
            Ok(new_type)
        }
        EntryLocation::Daily {
            line_idx,
            source_path,
        } => {
            if source_path.is_some() || app.combined_view {
                // Combined mode: persist via storage, then reload
                let new_type =
                    storage::cycle_entry_type(app.current_date, &path, *line_idx)?;
                let _ = app.load_combined_data();
                Ok(new_type)
            } else if let Line::Entry(raw_entry) = &mut app.lines[*line_idx] {
                let new_type = raw_entry.entry_type.cycle();
                raw_entry.entry_type = new_type.clone();
                app.save();
                Ok(Some(new_type))
            } else {
                Ok(None)
            }
        }
        EntryLocation::Filter { index, entry } => {
            let new_type = storage::cycle_entry_type(entry.source_date, &path, entry.line_index)?;
            if let Some(ref new_type) = new_type {
                if let ViewMode::Filter(state) = &mut app.view
                    && let Some(filter_entry) = state.entries.get_mut(*index)
                {
                    filter_entry.entry_type = new_type.clone();
                }

                if entry.source_date == app.current_date {
                    app.reload_current_day()?;
                }
            }
            Ok(new_type)
        }
    }
}

fn set_entry_type_raw(
    app: &mut App,
    location: &EntryLocation,
    entry_type: &EntryType,
) -> io::Result<()> {
    let path = app.resolve_entry_path(location);

    match location {
        EntryLocation::Projected(entry) => {
            storage::mutate_entry(entry.source_date, &path, entry.line_index, |raw_entry| {
                raw_entry.entry_type = entry_type.clone();
            })?;
            if let ViewMode::Daily(state) = &mut app.view
                && let Some(projected_entry) = state.projected_entries.iter_mut().find(|e| {
                    e.source_date == entry.source_date && e.line_index == entry.line_index
                })
            {
                projected_entry.entry_type = entry_type.clone();
            }
        }
        EntryLocation::Daily {
            line_idx,
            source_path,
        } => {
            if source_path.is_some() || app.combined_view {
                let et = entry_type.clone();
                storage::mutate_entry(app.current_date, &path, *line_idx, move |raw_entry| {
                    raw_entry.entry_type = et;
                })?;
                let _ = app.load_combined_data();
            } else if let Line::Entry(raw_entry) = &mut app.lines[*line_idx] {
                raw_entry.entry_type = entry_type.clone();
                app.save();
            }
        }
        EntryLocation::Filter { index, entry } => {
            storage::mutate_entry(entry.source_date, &path, entry.line_index, |raw_entry| {
                raw_entry.entry_type = entry_type.clone();
            })?;
            if let ViewMode::Filter(state) = &mut app.view
                && let Some(filter_entry) = state.entries.get_mut(*index)
            {
                filter_entry.entry_type = entry_type.clone();
            }

            if entry.source_date == app.current_date {
                app.reload_current_day()?;
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct PasteTarget {
    pub date: NaiveDate,
    pub start_line_index: usize,
    pub entries: Vec<Entry>,
}

pub struct PasteEntries {
    target: PasteTarget,
}

impl PasteEntries {
    #[must_use]
    pub fn new(target: PasteTarget) -> Self {
        Self { target }
    }
}

impl Action for PasteEntries {
    fn execute(&mut self, _app: &mut App) -> io::Result<Box<dyn Action>> {
        // Entries were already pasted when this action was made,
        // just return the reverse action
        Ok(Box::new(UnpasteEntries::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        let count = self.target.entries.len();
        ActionDescription {
            past: format!("Pasted {} {}", count, pluralize(count)),
            past_reversed: format!("Removed {} pasted {}", count, pluralize(count)),
            visibility: StatusVisibility::Always,
        }
    }
}

struct UnpasteEntries {
    target: PasteTarget,
}

impl UnpasteEntries {
    fn new(target: PasteTarget) -> Self {
        Self { target }
    }
}

impl Action for UnpasteEntries {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let path = app.active_path().to_path_buf();

        // Delete entries in reverse order to maintain indices
        for i in (0..self.target.entries.len()).rev() {
            let line_index = self.target.start_line_index + i;
            storage::delete_entry(self.target.date, &path, line_index)?;
        }

        if self.target.date == app.current_date {
            app.reload_current_day()?;
            app.clamp_selection_to_visible();
        }

        if let ViewMode::Filter(_) = &app.view {
            let _ = app.refresh_filter();
        }

        Ok(Box::new(RepasteEntries::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        let count = self.target.entries.len();
        ActionDescription {
            past: format!("Removed {} pasted {}", count, pluralize(count)),
            past_reversed: format!("Pasted {} {}", count, pluralize(count)),
            visibility: StatusVisibility::Always,
        }
    }
}

struct RepasteEntries {
    target: PasteTarget,
}

impl RepasteEntries {
    fn new(target: PasteTarget) -> Self {
        Self { target }
    }
}

impl Action for RepasteEntries {
    fn execute(&mut self, app: &mut App) -> io::Result<Box<dyn Action>> {
        let path = app.active_path().to_path_buf();

        let mut lines = storage::load_day_lines(self.target.date, &path)?;

        for (i, entry) in self.target.entries.iter().enumerate() {
            let insert_pos = (self.target.start_line_index + i).min(lines.len());
            lines.insert(insert_pos, Line::Entry(entry.to_raw()));
        }

        storage::save_day_lines(self.target.date, &path, &lines)?;

        if self.target.date == app.current_date {
            app.reload_current_day()?;
        }

        if let ViewMode::Filter(_) = &app.view {
            let _ = app.refresh_filter();
        }

        Ok(Box::new(UnpasteEntries::new(self.target.clone())))
    }

    fn description(&self) -> ActionDescription {
        let count = self.target.entries.len();
        ActionDescription {
            past: format!("Pasted {} {}", count, pluralize(count)),
            past_reversed: format!("Removed {} pasted {}", count, pluralize(count)),
            visibility: StatusVisibility::Always,
        }
    }
}
