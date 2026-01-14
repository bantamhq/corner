use chrono::{DateTime, Local, NaiveDate};
use ratatui::style::Color;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub calendar_id: String,
    pub calendar_name: String,
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
    pub is_all_day: bool,
    pub multi_day_info: Option<(u8, u8)>,
    pub is_cancelled: bool,
    pub is_declined: bool,
    pub color: Color,
}

#[derive(Debug, Clone, Default)]
pub enum CalendarFetchStatus {
    #[default]
    NotStarted,
    Fetching,
    Ready,
    Error(String),
}

#[derive(Debug, Default)]
pub struct CalendarStore {
    events_by_date: HashMap<NaiveDate, Vec<CalendarEvent>>,
    pub fetch_status: CalendarFetchStatus,
    pub visible_calendar_count: usize,
}

impl CalendarStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn events_for_date(&self, date: NaiveDate) -> &[CalendarEvent] {
        self.events_by_date
            .get(&date)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    #[must_use]
    pub fn has_events_on_date(&self, date: NaiveDate) -> bool {
        self.events_by_date
            .get(&date)
            .is_some_and(|events| !events.is_empty())
    }

    pub fn clear(&mut self) {
        self.events_by_date.clear();
        self.fetch_status = CalendarFetchStatus::NotStarted;
        self.visible_calendar_count = 0;
    }

    pub fn update(&mut self, events: Vec<CalendarEvent>, visible_count: usize) {
        self.events_by_date.clear();

        for event in events {
            let date = event.start.date_naive();
            self.events_by_date.entry(date).or_default().push(event);
        }

        for events in self.events_by_date.values_mut() {
            events.sort_by(|a, b| match (a.is_all_day, b.is_all_day) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.start.cmp(&b.start),
            });
        }

        self.visible_calendar_count = visible_count;
        self.fetch_status = CalendarFetchStatus::Ready;
    }

    pub fn set_fetching(&mut self) {
        self.fetch_status = CalendarFetchStatus::Fetching;
    }

    pub fn set_error(&mut self, message: String) {
        self.fetch_status = CalendarFetchStatus::Error(message);
    }
}
