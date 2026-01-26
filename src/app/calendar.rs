use std::collections::HashMap;
use std::io;

use chrono::{Datelike, Days, Local, Months, NaiveDate};

use crate::storage::{self, DayInfo};

use super::{App, SidebarType};

#[derive(Clone, Debug)]
pub struct CalendarState {
    pub selected: NaiveDate,
    pub display_month: NaiveDate,
    pub day_cache: HashMap<NaiveDate, DayInfo>,
}

impl CalendarState {
    #[must_use]
    pub fn new(date: NaiveDate) -> Self {
        Self {
            selected: date,
            display_month: first_of_month(date.year(), date.month()),
            day_cache: HashMap::new(),
        }
    }
}

impl App {
    #[must_use]
    pub fn calendar_state(&self) -> &CalendarState {
        &self.calendar_state
    }

    #[must_use]
    pub fn active_sidebar(&self) -> Option<SidebarType> {
        self.active_sidebar
    }

    pub fn toggle_sidebar(&mut self, sidebar: SidebarType) {
        self.active_sidebar = match self.active_sidebar {
            Some(current) if current == sidebar => None,
            _ => Some(sidebar),
        };
    }

    pub fn toggle_calendar_sidebar(&mut self) {
        self.toggle_sidebar(SidebarType::Calendar);
    }

    pub fn toggle_agenda(&mut self) {
        self.toggle_sidebar(SidebarType::Agenda);
    }

    pub fn sync_calendar_state(&mut self, date: NaiveDate) {
        let display_month = first_of_month(date.year(), date.month());
        let month_changed = self.calendar_state.display_month != display_month;

        self.calendar_state.selected = date;
        self.calendar_state.display_month = display_month;

        if month_changed {
            self.refresh_calendar_cache();
        }
    }

    pub fn refresh_calendar_cache(&mut self) {
        let display_month = self.calendar_state.display_month;
        self.calendar_state.day_cache = self.load_month_cache(display_month);
    }

    pub fn calendar_move(&mut self, dx: i32, dy: i32) -> io::Result<()> {
        let days_offset = dx + (dy * 7);
        let new_selected = if days_offset > 0 {
            self.calendar_state
                .selected
                .checked_add_days(Days::new(days_offset as u64))
        } else {
            self.calendar_state
                .selected
                .checked_sub_days(Days::new((-days_offset) as u64))
        };

        if let Some(new_date) = new_selected {
            self.goto_day(new_date)?;
        }
        Ok(())
    }

    pub fn calendar_prev_month(&mut self) -> io::Result<()> {
        if let Some(prev) = self
            .calendar_state
            .display_month
            .checked_sub_months(Months::new(1))
        {
            let new_selected = clamp_day_to_month(self.calendar_state.selected, prev);
            self.goto_day(new_selected)?;
        }
        Ok(())
    }

    pub fn calendar_next_month(&mut self) -> io::Result<()> {
        if let Some(next) = self
            .calendar_state
            .display_month
            .checked_add_months(Months::new(1))
        {
            let new_selected = clamp_day_to_month(self.calendar_state.selected, next);
            self.goto_day(new_selected)?;
        }
        Ok(())
    }

    pub fn calendar_goto_today(&mut self) -> io::Result<()> {
        self.goto_day(Local::now().date_naive())
    }

    fn load_month_cache(&self, month: NaiveDate) -> HashMap<NaiveDate, DayInfo> {
        let (start, end) = month_date_range(month.year(), month.month());
        let mut cache =
            storage::scan_days_in_range(start, end, self.active_path()).unwrap_or_default();

        // Mark dates with calendar events
        for date in start.iter_days().take_while(|d| *d <= end) {
            if !self.calendar_store.events_for_date(date).is_empty() {
                cache.entry(date).or_default().has_calendar_events = true;
            }
        }

        // Mark dates with recurring entries
        if let Ok(recurring_dates) =
            storage::scan_recurring_in_range(start, end, self.active_path())
        {
            for date in recurring_dates {
                cache.entry(date).or_default().has_recurring = true;
            }
        }

        cache
    }
}

fn month_date_range(year: i32, month: u32) -> (NaiveDate, NaiveDate) {
    let start = first_of_month(year, month);
    let end = first_of_next_month(year, month).pred_opt().unwrap_or(start);
    (start, end)
}

fn first_of_month(year: i32, month: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, 1).expect("valid year/month from chrono date")
}

fn first_of_next_month(year: i32, month: u32) -> NaiveDate {
    if month == 12 {
        NaiveDate::from_ymd_opt(year.saturating_add(1), 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .expect("valid year/month from chrono date")
}

fn days_in_month(year: i32, month: u32) -> u32 {
    first_of_next_month(year, month)
        .pred_opt()
        .map(|d| d.day())
        .unwrap_or(28)
}

pub(super) fn clamp_day_to_month(date: NaiveDate, target_month: NaiveDate) -> NaiveDate {
    let day = date
        .day()
        .min(days_in_month(target_month.year(), target_month.month()));
    NaiveDate::from_ymd_opt(target_month.year(), target_month.month(), day).unwrap_or(target_month)
}
