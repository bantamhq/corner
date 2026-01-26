use std::collections::HashMap;

use chrono::{Datelike, Local, NaiveDate};
use ratatui::widgets::calendar::{CalendarEventStore, Monthly};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Style, Stylize},
};
use time::{Date, Month};

use crate::storage::DayInfo;

use super::theme;

pub const CALENDAR_WIDTH: u16 = 22;
pub const CALENDAR_HEIGHT: u16 = 7;
const CALENDAR_RIGHT_PADDING: u16 = 0;
const CALENDAR_BORDER_PADDING: u16 = 2;

pub struct CalendarModel<'a> {
    pub selected: NaiveDate,
    pub display_month: NaiveDate,
    pub day_cache: &'a HashMap<NaiveDate, DayInfo>,
}

impl CalendarModel<'_> {
    #[must_use]
    pub fn panel_width() -> u16 {
        CALENDAR_WIDTH + CALENDAR_RIGHT_PADDING + CALENDAR_BORDER_PADDING
    }
}

/// Convert chrono NaiveDate to time::Date (required by ratatui calendar)
fn to_time_date(date: NaiveDate) -> Date {
    Date::from_calendar_date(
        date.year(),
        Month::try_from(date.month() as u8).unwrap(),
        date.day() as u8,
    )
    .unwrap()
}

pub fn render_calendar(f: &mut Frame<'_>, model: &CalendarModel<'_>, area: Rect) {
    let today = Local::now().date_naive();
    let mut events = CalendarEventStore::default();

    // Style non-selected, non-today days based on content
    for (date, info) in model.day_cache {
        if *date == model.selected || *date == today {
            continue;
        }
        if info.has_incomplete_tasks {
            // Incomplete tasks: yellow to draw attention
            events.add(
                to_time_date(*date),
                Style::default().fg(theme::CALENDAR_INCOMPLETE).not_dim(),
            );
        } else if info.has_entries {
            // Manual entries: white
            events.add(
                to_time_date(*date),
                Style::default().fg(theme::CALENDAR_TEXT).not_dim(),
            );
        } else if info.has_calendar_events || info.has_recurring {
            // Only automated entries (calendar/recurring): blue
            events.add(
                to_time_date(*date),
                Style::default().fg(theme::CALENDAR_AUTOMATED).not_dim(),
            );
        }
        // Empty days use default style (white dimmed)
    }

    // Today - always cyan regardless of journal context
    if today.month() == model.display_month.month()
        && today.year() == model.display_month.year()
        && today != model.selected
    {
        events.add(
            to_time_date(today),
            Style::default().fg(theme::CALENDAR_TODAY).not_dim(),
        );
    }

    // Selected day styling
    let selected_info = model.day_cache.get(&model.selected);
    let selected_style = if model.selected == today {
        Style::default().fg(theme::CALENDAR_TODAY).reversed().not_dim()
    } else if selected_info.is_some_and(|i| i.has_incomplete_tasks) {
        Style::default()
            .fg(theme::CALENDAR_INCOMPLETE)
            .reversed()
            .not_dim()
    } else if selected_info.is_some_and(|i| i.has_entries) {
        Style::default()
            .fg(theme::CALENDAR_TEXT)
            .reversed()
            .not_dim()
    } else if selected_info.is_some_and(|i| i.has_calendar_events || i.has_recurring) {
        Style::default()
            .fg(theme::CALENDAR_AUTOMATED)
            .reversed()
            .not_dim()
    } else {
        Style::default().reversed().not_dim()
    };
    events.add(to_time_date(model.selected), selected_style);

    let calendar_area = Rect {
        x: area.x,
        y: area.y,
        width: CALENDAR_WIDTH.min(area.width),
        height: CALENDAR_HEIGHT.min(area.height),
    };

    let calendar = Monthly::new(to_time_date(model.display_month), events)
        .show_weekdays_header(Style::default().fg(theme::CALENDAR_TEXT).dim().bold())
        .default_style(Style::default().fg(theme::CALENDAR_TEXT).dim());

    f.render_widget(calendar, calendar_area);
}
