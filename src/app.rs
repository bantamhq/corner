use std::io;

use chrono::{Datelike, Days, Local, NaiveDate};

use crate::cursor::CursorBuffer;
use crate::storage::{self, Entry, EntryType, FilterItem, Line};

#[derive(PartialEq, Clone)]
pub enum Mode {
    Daily,
    Edit,
    Command,
    Order,
    FilterInput,
    Filter,
}

pub struct App {
    pub current_date: NaiveDate,
    pub lines: Vec<Line>,
    pub entry_indices: Vec<usize>,
    pub selected: usize,
    pub edit_buffer: Option<CursorBuffer>,
    pub mode: Mode,
    pub command_buffer: String,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub show_help: bool,
    pub help_scroll: usize,
    pub help_visible_height: usize,
    pub original_lines: Option<Vec<Line>>,
    pub scroll_offset: usize,
    pub last_deleted: Option<(usize, Entry)>,
    pub filter_query: String,
    pub filter_buffer: String,
    pub filter_items: Vec<FilterItem>,
    pub filter_selected: usize,
    pub filter_scroll_offset: usize,
    pub filter_edit_target: Option<(NaiveDate, usize)>,
}

impl App {
    pub fn new() -> io::Result<Self> {
        let current_date = Local::now().date_naive();
        let lines = storage::load_day_lines(current_date)?;
        let entry_indices = Self::compute_entry_indices(&lines);

        Ok(Self {
            current_date,
            lines,
            selected: entry_indices.len().saturating_sub(1),
            entry_indices,
            edit_buffer: None,
            mode: Mode::Daily,
            command_buffer: String::new(),
            should_quit: false,
            status_message: None,
            show_help: false,
            help_scroll: 0,
            help_visible_height: 0,
            original_lines: None,
            scroll_offset: 0,
            last_deleted: None,
            filter_query: String::new(),
            filter_buffer: String::new(),
            filter_items: Vec::new(),
            filter_selected: 0,
            filter_scroll_offset: 0,
            filter_edit_target: None,
        })
    }

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

    pub fn get_selected_entry(&self) -> Option<&Entry> {
        if self.entry_indices.is_empty() {
            return None;
        }
        let line_idx = self.entry_indices.get(self.selected)?;
        if let Line::Entry(entry) = &self.lines[*line_idx] {
            Some(entry)
        } else {
            None
        }
    }

    pub fn get_selected_entry_mut(&mut self) -> Option<&mut Entry> {
        if self.entry_indices.is_empty() {
            return None;
        }
        let line_idx = *self.entry_indices.get(self.selected)?;
        if let Line::Entry(entry) = &mut self.lines[line_idx] {
            Some(entry)
        } else {
            None
        }
    }

    pub fn save(&mut self) {
        if let Err(e) = storage::save_day_lines(self.current_date, &self.lines) {
            self.status_message = Some(format!("Failed to save: {e}"));
        }
    }

    pub fn goto_day(&mut self, date: NaiveDate) -> io::Result<()> {
        if date == self.current_date {
            return Ok(());
        }

        self.save();
        self.current_date = date;
        self.lines = storage::load_day_lines(date)?;
        self.entry_indices = Self::compute_entry_indices(&self.lines);
        self.selected = self.entry_indices.len().saturating_sub(1);
        self.edit_buffer = None;
        self.mode = Mode::Daily;
        self.scroll_offset = 0;

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

    pub fn parse_goto_date(input: &str) -> Option<NaiveDate> {
        if let Ok(date) = NaiveDate::parse_from_str(input, "%Y/%m/%d") {
            return Some(date);
        }
        let current_year = Local::now().year();
        NaiveDate::parse_from_str(&format!("{current_year}/{input}"), "%Y/%m/%d").ok()
    }

    pub fn add_entry(&mut self, entry: Entry, at_bottom: bool) {
        let insert_pos = if at_bottom || self.entry_indices.is_empty() {
            self.lines.len()
        } else {
            self.entry_indices[self.selected] + 1
        };

        self.lines.insert(insert_pos, Line::Entry(entry));
        self.entry_indices = Self::compute_entry_indices(&self.lines);

        self.selected = self
            .entry_indices
            .iter()
            .position(|&idx| idx == insert_pos)
            .unwrap_or(self.entry_indices.len().saturating_sub(1));

        self.edit_buffer = Some(CursorBuffer::empty());
        self.mode = Mode::Edit;
    }

    pub fn new_task(&mut self, at_bottom: bool) {
        self.add_entry(Entry::new_task(""), at_bottom);
    }

    pub fn commit_and_add_new(&mut self) {
        let Some(buffer) = self.edit_buffer.take() else {
            return;
        };
        let content = buffer.into_content();

        if content.trim().is_empty() {
            let was_at_end = self.selected == self.entry_indices.len() - 1;
            self.delete_selected();
            if !was_at_end && self.selected > 0 {
                self.selected -= 1;
            }
            self.mode = Mode::Daily;
            return;
        }

        let entry_type = self
            .get_selected_entry()
            .map(|e| e.entry_type.clone())
            .unwrap_or(EntryType::Task { completed: false });

        if let Some(entry) = self.get_selected_entry_mut() {
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
        self.add_entry(new_entry, false);
    }

    pub fn edit_selected(&mut self) {
        let content = self.get_selected_entry().map(|e| e.content.clone());
        if let Some(content) = content {
            self.edit_buffer = Some(CursorBuffer::new(content));
            self.mode = Mode::Edit;
        }
    }

    pub fn exit_edit(&mut self) {
        if let Some((date, line_index)) = self.filter_edit_target.take() {
            if let Some(buffer) = self.edit_buffer.take() {
                let content = buffer.into_content();
                if !content.trim().is_empty() {
                    match Self::update_entry_in_storage(date, line_index, content) {
                        Ok(false) => {
                            self.status_message = Some(format!(
                                "Failed to update: no entry at index {line_index} for {date}"
                            ));
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Failed to save: {e}"));
                        }
                        Ok(true) => {
                            // Keep current day in sync if edited entry was from current day
                            if date == self.current_date {
                                let _ = self.reload_current_day();
                            }
                        }
                    }
                }
            }
            let _ = self.refresh_filter();
            self.mode = Mode::Filter;
        } else {
            if let Some(buffer) = self.edit_buffer.take() {
                let content = buffer.into_content();
                if content.trim().is_empty() {
                    self.delete_selected();
                    self.scroll_offset = 0;
                } else if let Some(entry) = self.get_selected_entry_mut() {
                    entry.content = content;
                    self.save();
                }
            }
            self.mode = Mode::Daily;
        }
    }

    pub fn cancel_edit(&mut self) {
        self.edit_buffer = None;
        if self.filter_edit_target.take().is_some() {
            self.mode = Mode::Filter;
        } else {
            if let Some(entry) = self.get_selected_entry()
                && entry.content.is_empty()
            {
                self.delete_selected();
                self.scroll_offset = 0;
            }
            self.mode = Mode::Daily;
        }
    }

    fn update_entry_in_storage(date: NaiveDate, line_index: usize, content: String) -> io::Result<bool> {
        let mut lines = storage::load_day_lines(date)?;
        let updated = if let Some(Line::Entry(entry)) = lines.get_mut(line_index) {
            entry.content = content;
            true
        } else {
            false
        };
        if updated {
            storage::save_day_lines(date, &lines)?;
        }
        Ok(updated)
    }

    pub fn delete_selected(&mut self) {
        if self.entry_indices.is_empty() {
            return;
        }

        let line_idx = self.entry_indices[self.selected];
        if let Line::Entry(entry) = &self.lines[line_idx] {
            self.last_deleted = Some((line_idx, entry.clone()));
        }
        self.lines.remove(line_idx);
        self.entry_indices = Self::compute_entry_indices(&self.lines);

        if !self.entry_indices.is_empty() && self.selected >= self.entry_indices.len() {
            self.selected = self.entry_indices.len() - 1;
        }
    }

    pub fn undo(&mut self) {
        if let Some((line_idx, entry)) = self.last_deleted.take() {
            let insert_idx = line_idx.min(self.lines.len());
            self.lines.insert(insert_idx, Line::Entry(entry));
            self.entry_indices = Self::compute_entry_indices(&self.lines);
            if let Some(pos) = self.entry_indices.iter().position(|&i| i == insert_idx) {
                self.selected = pos;
            }
            self.save();
        }
    }

    pub fn toggle_task(&mut self) {
        if let Some(entry) = self.get_selected_entry_mut() {
            entry.toggle_complete();
            self.save();
        }
    }

    pub fn cycle_entry_type(&mut self) {
        if let Some(entry) = self.get_selected_entry_mut() {
            entry.entry_type = match entry.entry_type {
                EntryType::Task { .. } => EntryType::Note,
                EntryType::Note => EntryType::Event,
                EntryType::Event => EntryType::Task { completed: false },
            };
        }
    }

    pub fn gather_completed_tasks(&mut self) {
        let task_indices: Vec<usize> = self
            .lines
            .iter()
            .enumerate()
            .filter_map(|(i, line)| match line {
                Line::Entry(e) if matches!(e.entry_type, EntryType::Task { .. }) => Some(i),
                _ => None,
            })
            .collect();

        if task_indices.is_empty() {
            return;
        }

        let mut tasks: Vec<Entry> = task_indices
            .iter()
            .filter_map(|&i| {
                if let Line::Entry(e) = &self.lines[i] {
                    Some(e.clone())
                } else {
                    None
                }
            })
            .collect();

        tasks.sort_by_key(|e| !matches!(e.entry_type, EntryType::Task { completed: true }));

        for (slot, &line_idx) in task_indices.iter().enumerate() {
            self.lines[line_idx] = Line::Entry(tasks[slot].clone());
        }

        self.entry_indices = Self::compute_entry_indices(&self.lines);
        self.save();
    }

    pub fn enter_order_mode(&mut self) {
        if !self.entry_indices.is_empty() {
            self.original_lines = Some(self.lines.clone());
            self.mode = Mode::Order;
        }
    }

    pub fn exit_order_mode(&mut self, save: bool) {
        if save {
            self.save();
        } else if let Some(original) = self.original_lines.take() {
            self.lines = original;
            self.entry_indices = Self::compute_entry_indices(&self.lines);
        }
        self.original_lines = None;
        self.mode = Mode::Daily;
    }

    pub fn order_move_up(&mut self) {
        if self.selected == 0 {
            return;
        }
        let curr_line_idx = self.entry_indices[self.selected];
        let prev_line_idx = self.entry_indices[self.selected - 1];
        self.lines.swap(curr_line_idx, prev_line_idx);
        self.entry_indices = Self::compute_entry_indices(&self.lines);
        self.selected -= 1;
    }

    pub fn order_move_down(&mut self) {
        if self.selected >= self.entry_indices.len() - 1 {
            return;
        }
        let curr_line_idx = self.entry_indices[self.selected];
        let next_line_idx = self.entry_indices[self.selected + 1];
        self.lines.swap(curr_line_idx, next_line_idx);
        self.entry_indices = Self::compute_entry_indices(&self.lines);
        self.selected += 1;
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.entry_indices.is_empty() && self.selected < self.entry_indices.len() - 1 {
            self.selected += 1;
        }
    }

    pub fn jump_to_first(&mut self) {
        self.selected = 0;
    }

    pub fn jump_to_last(&mut self) {
        if !self.entry_indices.is_empty() {
            self.selected = self.entry_indices.len() - 1;
        }
    }

    pub fn execute_command(&mut self) -> io::Result<()> {
        let cmd = self.command_buffer.trim();
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        let command = parts.first().copied().unwrap_or("");
        let arg = parts.get(1).copied().unwrap_or("").trim();

        match command {
            "q" | "quit" => {
                self.save();
                self.should_quit = true;
            }
            "goto" | "gt" => {
                if arg.is_empty() {
                    self.status_message =
                        Some("Usage: :goto YYYY/MM/DD or :goto MM/DD".to_string());
                } else if let Some(date) = Self::parse_goto_date(arg) {
                    self.goto_day(date)?;
                } else {
                    self.status_message = Some(format!("Invalid date: {arg}"));
                }
            }
            _ => {}
        }
        self.command_buffer.clear();
        self.mode = Mode::Daily;
        Ok(())
    }

    pub fn enter_filter_input(&mut self) {
        self.filter_buffer = self.filter_query.clone();
        self.mode = Mode::FilterInput;
    }

    pub fn execute_filter(&mut self) -> io::Result<()> {
        self.save();
        self.filter_query = self.filter_buffer.clone();
        let filter = storage::parse_filter_query(&self.filter_query);
        self.filter_items = storage::collect_filtered_entries(&filter)?;
        self.filter_selected = 0;
        self.filter_scroll_offset = 0;
        self.mode = Mode::Filter;
        Ok(())
    }

    pub fn cancel_filter_input(&mut self) {
        self.filter_buffer.clear();
        self.mode = if self.filter_query.is_empty() {
            Mode::Daily
        } else {
            Mode::Filter
        };
    }

    pub fn exit_filter(&mut self) {
        self.filter_query.clear();
        self.filter_items.clear();
        self.mode = Mode::Daily;
    }

    pub fn refresh_filter(&mut self) -> io::Result<()> {
        let filter = storage::parse_filter_query(&self.filter_query);
        self.filter_items = storage::collect_filtered_entries(&filter)?;
        self.filter_selected = self
            .filter_selected
            .min(self.filter_items.len().saturating_sub(1));
        self.filter_scroll_offset = 0;
        Ok(())
    }

    pub fn filter_move_up(&mut self) {
        if self.filter_selected > 0 {
            self.filter_selected -= 1;
        }
    }

    pub fn filter_move_down(&mut self) {
        if !self.filter_items.is_empty() && self.filter_selected < self.filter_items.len() - 1 {
            self.filter_selected += 1;
        }
    }

    pub fn filter_visual_line(&self) -> usize {
        if self.filter_items.is_empty() {
            return 0;
        }

        let mut date_headers = 0;
        let mut last_date = None;
        let mut is_first_of_date = false;

        for (idx, item) in self.filter_items.iter().enumerate() {
            if last_date != Some(item.source_date) {
                date_headers += 1;
                last_date = Some(item.source_date);
                if idx == self.filter_selected {
                    is_first_of_date = true;
                }
            }
            if idx == self.filter_selected {
                break;
            }
        }

        let visual_line = date_headers + self.filter_selected;

        if is_first_of_date && visual_line > 0 {
            visual_line - 1
        } else {
            visual_line
        }
    }

    pub fn filter_total_lines(&self) -> usize {
        if self.filter_items.is_empty() {
            return 1;
        }

        let unique_dates = self
            .filter_items
            .iter()
            .map(|item| item.source_date)
            .collect::<std::collections::HashSet<_>>()
            .len();

        unique_dates + self.filter_items.len()
    }

    pub fn filter_jump_to_day(&mut self) -> io::Result<()> {
        let Some(item) = self.filter_items.get(self.filter_selected) else {
            return Ok(());
        };

        let date = item.source_date;
        let line_index = item.line_index;

        self.filter_query.clear();
        self.filter_items.clear();

        if date != self.current_date {
            self.current_date = date;
            self.lines = storage::load_day_lines(date)?;
            self.entry_indices = Self::compute_entry_indices(&self.lines);
        }

        if let Some(pos) = self.entry_indices.iter().position(|&i| i == line_index) {
            self.selected = pos;
        }

        self.mode = Mode::Daily;
        self.scroll_offset = 0;
        Ok(())
    }

    pub fn filter_toggle(&mut self) -> io::Result<()> {
        let Some(item) = self.filter_items.get(self.filter_selected) else {
            return Ok(());
        };

        let date = item.source_date;
        let line_index = item.line_index;

        Self::toggle_entry_in_storage(date, line_index)?;

        let item = &mut self.filter_items[self.filter_selected];
        item.completed = !item.completed;
        if let EntryType::Task { completed } = &mut item.entry_type {
            *completed = item.completed;
        }

        if date == self.current_date {
            self.reload_current_day()?;
        }

        Ok(())
    }

    pub fn filter_edit(&mut self) {
        let Some(item) = self.filter_items.get(self.filter_selected) else {
            return;
        };

        self.filter_edit_target = Some((item.source_date, item.line_index));
        self.edit_buffer = Some(CursorBuffer::new(item.content.clone()));
        self.mode = Mode::Edit;
    }

    fn toggle_entry_in_storage(date: NaiveDate, line_index: usize) -> io::Result<()> {
        let mut lines = storage::load_day_lines(date)?;
        if let Some(Line::Entry(entry)) = lines.get_mut(line_index) {
            entry.toggle_complete();
        }
        storage::save_day_lines(date, &lines)
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
        let result = App::parse_goto_date("2025/12/30");
        assert!(result.is_some(), "2025/12/30 should parse");
        assert_eq!(
            result.unwrap(),
            NaiveDate::from_ymd_opt(2025, 12, 30).unwrap()
        );

        let result = App::parse_goto_date("12/30");
        assert!(result.is_some(), "12/30 should parse");

        assert!(App::parse_goto_date("12/30/2025").is_none());
        assert!(App::parse_goto_date("12/30/25").is_none());
    }

    #[test]
    fn test_command_parsing() {
        let cmd = "gt 12/30";
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        let command = parts.first().copied().unwrap_or("");
        let arg = parts.get(1).copied().unwrap_or("").trim();

        assert_eq!(command, "gt");
        assert_eq!(arg, "12/30");

        let cmd = "gt 12/30/2025";
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        let command = parts.first().copied().unwrap_or("");
        let arg = parts.get(1).copied().unwrap_or("").trim();

        assert_eq!(command, "gt");
        assert_eq!(arg, "12/30/2025");
        assert!(
            App::parse_goto_date(arg).is_none(),
            "12/30/2025 should NOT parse"
        );
    }
}
