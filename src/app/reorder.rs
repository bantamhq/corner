use crate::storage::Line;

use super::{App, InputMode, SelectedItem, ViewMode};

impl App {
    pub fn enter_reorder_mode(&mut self) {
        if matches!(self.get_selected_item(), SelectedItem::Projected { .. }) {
            self.set_error("Cannot reorder projected entries");
            return;
        }

        if let ViewMode::Daily(state) = &mut self.view
            && !self.entry_indices.is_empty()
        {
            state.original_lines = Some(self.lines.clone());
            self.input_mode = InputMode::Reorder;
        }
    }

    pub fn save_reorder_mode(&mut self) {
        if !matches!(self.view, ViewMode::Daily(_)) {
            return;
        }
        self.save();
        if let ViewMode::Daily(state) = &mut self.view {
            state.original_lines = None;
        }
        self.input_mode = InputMode::Normal;
    }

    pub fn cancel_reorder_mode(&mut self) {
        if !matches!(self.view, ViewMode::Daily(_)) {
            return;
        }
        if let ViewMode::Daily(state) = &mut self.view
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

    fn is_entry_visible_at(&self, line_idx: usize) -> bool {
        if !self.hide_completed {
            return true;
        }
        if let Line::Entry(raw_entry) = &self.lines[line_idx] {
            self.should_show_raw_entry(raw_entry)
        } else {
            true
        }
    }

    pub fn reorder_move_up(&mut self) {
        let SelectedItem::Daily {
            index: curr_entry_idx,
            ..
        } = self.get_selected_item()
        else {
            return;
        };

        let prev_entry_idx = self.entry_indices[..curr_entry_idx]
            .iter()
            .enumerate()
            .rev()
            .find(|&(_, &line_idx)| self.is_entry_visible_at(line_idx))
            .map(|(idx, _)| idx);

        let Some(prev_entry_idx) = prev_entry_idx else {
            return;
        };

        let curr_line_idx = self.entry_indices[curr_entry_idx];
        let prev_line_idx = self.entry_indices[prev_entry_idx];
        let entry = self.lines.remove(curr_line_idx);
        self.lines.insert(prev_line_idx, entry);
        self.entry_indices = Self::compute_entry_indices(&self.lines);

        if let ViewMode::Daily(state) = &mut self.view {
            state.selected -= 1;
        }
    }

    pub fn reorder_move_down(&mut self) {
        let SelectedItem::Daily {
            index: curr_entry_idx,
            ..
        } = self.get_selected_item()
        else {
            return;
        };

        let next_entry_idx = self.entry_indices[curr_entry_idx + 1..]
            .iter()
            .enumerate()
            .find(|&(_, &line_idx)| self.is_entry_visible_at(line_idx))
            .map(|(offset, _)| curr_entry_idx + 1 + offset);

        let Some(next_entry_idx) = next_entry_idx else {
            return;
        };

        let curr_line_idx = self.entry_indices[curr_entry_idx];
        let next_line_idx = self.entry_indices[next_entry_idx];
        let entry = self.lines.remove(curr_line_idx);
        let insert_pos = if next_line_idx > curr_line_idx {
            next_line_idx
        } else {
            next_line_idx + 1
        };
        self.lines.insert(insert_pos, entry);
        self.entry_indices = Self::compute_entry_indices(&self.lines);

        if let ViewMode::Daily(state) = &mut self.view {
            state.selected += 1;
        }
    }
}
