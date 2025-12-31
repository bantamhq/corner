use std::io;

use chrono::{Datelike, Days, Local, NaiveDate};

use crate::cursor::CursorBuffer;
use crate::storage::{self, Entry, EntryType, Line, TaskItem};

#[derive(PartialEq, Clone)]
pub enum Mode {
    Daily,
    Edit,
    Command,
    Tasks,
}

pub struct App {
    pub current_date: NaiveDate,
    /// Preserves all content including non-entry text (blank lines, notes, etc.)
    /// so the journal file can contain arbitrary markdown without data loss.
    pub lines: Vec<Line>,
    /// Maps selection index to position in `lines` for navigable entries only.
    pub entry_indices: Vec<usize>,
    pub selected: usize,
    pub edit_buffer: Option<CursorBuffer>,
    pub mode: Mode,
    pub command_buffer: String,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub task_items: Vec<TaskItem>,
    pub task_selected: usize,
}

impl App {
    pub fn new() -> io::Result<Self> {
        let current_date = Local::now().date_naive();
        let lines = storage::load_day_lines(current_date)?;
        let entry_indices = Self::compute_entry_indices(&lines);

        Ok(Self {
            current_date,
            lines,
            entry_indices,
            selected: 0,
            edit_buffer: None,
            mode: Mode::Daily,
            command_buffer: String::new(),
            should_quit: false,
            status_message: None,
            task_items: Vec::new(),
            task_selected: 0,
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
        self.selected = 0;
        self.edit_buffer = None;
        self.mode = Mode::Daily;

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

        if let Some(entry) = self.get_selected_entry_mut() {
            entry.content = content;
        }
        self.save();

        self.add_entry(Entry::new_task(""), false);
    }

    pub fn edit_selected(&mut self) {
        let content = self.get_selected_entry().map(|e| e.content.clone());
        if let Some(content) = content {
            self.edit_buffer = Some(CursorBuffer::new(content));
            self.mode = Mode::Edit;
        }
    }

    pub fn exit_edit(&mut self) {
        if let Some(buffer) = self.edit_buffer.take() {
            let content = buffer.into_content();
            if content.trim().is_empty() {
                self.delete_selected();
            } else if let Some(entry) = self.get_selected_entry_mut() {
                entry.content = content;
                self.save();
            }
        }
        self.mode = Mode::Daily;
    }

    pub fn delete_selected(&mut self) {
        if self.entry_indices.is_empty() {
            return;
        }

        let line_idx = self.entry_indices[self.selected];
        self.lines.remove(line_idx);
        self.entry_indices = Self::compute_entry_indices(&self.lines);

        if !self.entry_indices.is_empty() && self.selected >= self.entry_indices.len() {
            self.selected = self.entry_indices.len() - 1;
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

    pub fn enter_tasks_mode(&mut self) -> io::Result<()> {
        self.save();
        self.task_items = storage::collect_incomplete_tasks()?;
        self.task_selected = 0;
        self.mode = Mode::Tasks;
        Ok(())
    }

    pub fn exit_tasks_mode(&mut self) {
        self.mode = Mode::Daily;
    }

    pub fn task_move_up(&mut self) {
        if self.task_selected > 0 {
            self.task_selected -= 1;
        }
    }

    pub fn task_move_down(&mut self) {
        if !self.task_items.is_empty() && self.task_selected < self.task_items.len() - 1 {
            self.task_selected += 1;
        }
    }

    pub fn task_jump_to_day(&mut self) -> io::Result<()> {
        if let Some(item) = self.task_items.get(self.task_selected) {
            let date = item.date;
            self.goto_day(date)?;
            self.mode = Mode::Daily;
        }
        Ok(())
    }

    pub fn task_toggle(&mut self) -> io::Result<()> {
        let Some(item) = self.task_items.get(self.task_selected) else {
            return Ok(());
        };

        let date = item.date;
        let line_index = item.line_index;

        Self::toggle_task_in_storage(date, line_index)?;
        self.task_items[self.task_selected].completed =
            !self.task_items[self.task_selected].completed;

        if date == self.current_date {
            self.reload_current_day()?;
        }

        Ok(())
    }

    fn toggle_task_in_storage(date: NaiveDate, line_index: usize) -> io::Result<()> {
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
