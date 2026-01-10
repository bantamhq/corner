use std::io;

use chrono::{Datelike, Days, Local, Months, NaiveDate};

use crate::storage;

use super::{App, DateInterfaceState, InputMode, InterfaceContext, LastInteraction, ViewMode};

impl App {
    pub fn open_date_interface(&mut self) {
        let initial_date = match &self.view {
            ViewMode::Daily(_) => self.current_date,
            ViewMode::Filter(_) => self.last_daily_date,
        };

        let mut state = DateInterfaceState::new(initial_date);
        state.day_cache = self.load_month_cache(state.display_month);
        self.input_mode = InputMode::Interface(InterfaceContext::Date(state));
    }

    pub fn confirm_date_interface(&mut self) -> io::Result<()> {
        let InputMode::Interface(InterfaceContext::Date(state)) =
            std::mem::replace(&mut self.input_mode, InputMode::Normal)
        else {
            return Ok(());
        };

        self.goto_day(state.selected)
    }

    pub fn date_interface_goto_today(&mut self) {
        let InputMode::Interface(InterfaceContext::Date(ref mut state)) = self.input_mode else {
            return;
        };

        state.last_interaction = LastInteraction::Calendar;

        let today = Local::now().date_naive();
        let old_month = (state.display_month.year(), state.display_month.month());
        state.selected = today;

        let new_month = (today.year(), today.month());
        if old_month != new_month {
            state.display_month = first_of_month(today.year(), today.month());
            self.refresh_date_interface_cache();
        }
    }

    pub fn date_interface_move(&mut self, dx: i32, dy: i32) {
        let InputMode::Interface(InterfaceContext::Date(ref mut state)) = self.input_mode else {
            return;
        };

        state.last_interaction = LastInteraction::Calendar;

        let days_offset = dx + (dy * 7);
        let new_selected = if days_offset > 0 {
            state
                .selected
                .checked_add_days(Days::new(days_offset as u64))
        } else {
            state
                .selected
                .checked_sub_days(Days::new((-days_offset) as u64))
        };

        if let Some(new_date) = new_selected {
            let old_month = (state.display_month.year(), state.display_month.month());
            state.selected = new_date;

            let new_month = (new_date.year(), new_date.month());
            if old_month != new_month {
                state.display_month = first_of_month(new_date.year(), new_date.month());
                self.refresh_date_interface_cache();
            }
        }
    }

    pub fn date_interface_prev_month(&mut self) {
        let InputMode::Interface(InterfaceContext::Date(ref mut state)) = self.input_mode else {
            return;
        };
        state.last_interaction = LastInteraction::Calendar;
        if let Some(prev) = state.display_month.checked_sub_months(Months::new(1)) {
            state.display_month = prev;
            state.selected = clamp_day_to_month(state.selected, prev);
        }
        self.refresh_date_interface_cache();
    }

    pub fn date_interface_next_month(&mut self) {
        let InputMode::Interface(InterfaceContext::Date(ref mut state)) = self.input_mode else {
            return;
        };
        state.last_interaction = LastInteraction::Calendar;
        if let Some(next) = state.display_month.checked_add_months(Months::new(1)) {
            state.display_month = next;
            state.selected = clamp_day_to_month(state.selected, next);
        }
        self.refresh_date_interface_cache();
    }

    pub fn date_interface_prev_year(&mut self) {
        let InputMode::Interface(InterfaceContext::Date(ref mut state)) = self.input_mode else {
            return;
        };
        state.last_interaction = LastInteraction::Calendar;
        let new_year = state.display_month.year() - 1;
        if let Some(new_month) = NaiveDate::from_ymd_opt(new_year, state.display_month.month(), 1) {
            state.display_month = new_month;
            state.selected = clamp_day_to_month(state.selected, new_month);
        }
        self.refresh_date_interface_cache();
    }

    pub fn date_interface_next_year(&mut self) {
        let InputMode::Interface(InterfaceContext::Date(ref mut state)) = self.input_mode else {
            return;
        };
        state.last_interaction = LastInteraction::Calendar;
        let new_year = state.display_month.year() + 1;
        if let Some(new_month) = NaiveDate::from_ymd_opt(new_year, state.display_month.month(), 1) {
            state.display_month = new_month;
            state.selected = clamp_day_to_month(state.selected, new_month);
        }
        self.refresh_date_interface_cache();
    }

    pub fn date_interface_submit_input(&mut self) -> io::Result<()> {
        let input = {
            let InputMode::Interface(InterfaceContext::Date(ref state)) = self.input_mode else {
                return Ok(());
            };
            state.query.content().trim().to_string()
        };

        if input.is_empty() {
            return Ok(());
        }

        let today = Local::now().date_naive();
        if let Some(date) = storage::parse_date(&input, storage::ParseContext::Interface, today) {
            self.input_mode = InputMode::Normal;
            self.goto_day(date)?;
        } else {
            self.set_status(format!("Invalid date: {input}"));
        }

        Ok(())
    }

    pub fn date_interface_input_char(&mut self, c: char) {
        let InputMode::Interface(InterfaceContext::Date(ref mut state)) = self.input_mode else {
            return;
        };
        state.query.insert_char(c);
        state.last_interaction = LastInteraction::Typed;
    }

    pub fn date_interface_input_backspace(&mut self) {
        let InputMode::Interface(InterfaceContext::Date(ref mut state)) = self.input_mode else {
            return;
        };
        state.query.delete_char_before();
        if state.query.is_empty() {
            state.last_interaction = LastInteraction::Calendar;
        }
    }

    pub fn date_interface_input_delete(&mut self) {
        let InputMode::Interface(InterfaceContext::Date(ref mut state)) = self.input_mode else {
            return;
        };
        state.query.delete_char_after();
    }

    pub fn date_interface_input_is_empty(&self) -> bool {
        let InputMode::Interface(InterfaceContext::Date(ref state)) = self.input_mode else {
            return true;
        };
        state.query.is_empty()
    }

    fn load_month_cache(
        &self,
        month: NaiveDate,
    ) -> std::collections::HashMap<NaiveDate, storage::DayInfo> {
        let (start, end) = month_date_range(month.year(), month.month());
        storage::scan_days_in_range(start, end, self.active_path()).unwrap_or_default()
    }

    fn refresh_date_interface_cache(&mut self) {
        let month = {
            let InputMode::Interface(InterfaceContext::Date(ref state)) = self.input_mode else {
                return;
            };
            state.display_month
        };

        let cache = self.load_month_cache(month);

        if let InputMode::Interface(InterfaceContext::Date(ref mut state)) = self.input_mode {
            state.day_cache = cache;
        }
    }
}

fn month_date_range(year: i32, month: u32) -> (NaiveDate, NaiveDate) {
    let start = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let end = first_of_next_month(year, month).pred_opt().unwrap();
    (start, end)
}

fn first_of_month(year: i32, month: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, 1).unwrap()
}

fn first_of_next_month(year: i32, month: u32) -> NaiveDate {
    if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    first_of_next_month(year, month)
        .pred_opt()
        .map(|d| d.day())
        .unwrap_or(28)
}

/// Keeps the same day-of-month but moves to target_month, clamping if needed.
fn clamp_day_to_month(date: NaiveDate, target_month: NaiveDate) -> NaiveDate {
    let day = date
        .day()
        .min(days_in_month(target_month.year(), target_month.month()));
    NaiveDate::from_ymd_opt(target_month.year(), target_month.month(), day).unwrap_or(target_month)
}
