use chrono::{Datelike, Local, NaiveDate};
use ratatui::widgets::calendar::{CalendarEventStore, Monthly};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};
use time::{Date, Month};

use crate::app::DateInterfaceState;

use super::interface_popup::{PopupLayout, render_popup_frame, render_query_input};

const CALENDAR_WIDTH: u16 = 22;

/// Convert chrono NaiveDate to time::Date (required by ratatui calendar)
fn to_time_date(date: NaiveDate) -> Date {
    Date::from_calendar_date(
        date.year(),
        Month::try_from(date.month() as u8).unwrap(),
        date.day() as u8,
    )
    .unwrap()
}

pub fn render_date_interface(f: &mut Frame, state: &DateInterfaceState, area: Rect) {
    let layout = PopupLayout::with_query(area);

    if layout.is_too_small() {
        return;
    }

    let title = state.display_month.format("%B %Y").to_string();
    render_popup_frame(f, &layout, &title);
    render_query_input(f, &layout, &state.query, true);

    let today = Local::now().date_naive();
    let mut events = CalendarEventStore::default();

    // Style for days with content (not dimmed)
    // Priority: incomplete tasks (yellow) > journal events (magenta) > calendar events (blue) > other (gray)
    for (date, info) in &state.day_cache {
        if *date == state.selected || *date == today {
            continue;
        }
        if info.has_entries || info.has_calendar_events {
            let style = if info.has_incomplete_tasks {
                Style::new().fg(Color::Yellow).not_dim()
            } else if info.has_events {
                Style::new().fg(Color::Magenta).not_dim()
            } else if info.has_calendar_events {
                Style::new().fg(Color::Blue).not_dim()
            } else {
                Style::new().fg(Color::Gray).not_dim()
            };
            events.add(to_time_date(*date), style);
        }
    }

    if today.month() == state.display_month.month()
        && today.year() == state.display_month.year()
        && today != state.selected
    {
        events.add(to_time_date(today), Style::new().fg(Color::Cyan).not_dim());
    }

    let selected_info = state.day_cache.get(&state.selected);
    let selected_style = if state.selected == today {
        Style::new().fg(Color::Cyan).reversed().not_dim()
    } else if selected_info
        .map(|i| i.has_incomplete_tasks)
        .unwrap_or(false)
    {
        Style::new().fg(Color::Yellow).reversed().not_dim()
    } else if selected_info.map(|i| i.has_events).unwrap_or(false) {
        Style::new().fg(Color::Magenta).reversed().not_dim()
    } else if selected_info.map(|i| i.has_calendar_events).unwrap_or(false) {
        Style::new().fg(Color::Blue).reversed().not_dim()
    } else {
        Style::new().reversed().not_dim()
    };
    events.add(to_time_date(state.selected), selected_style);

    let calendar_area = Rect {
        x: layout.content_area.x,
        y: layout.content_area.y,
        width: CALENDAR_WIDTH.min(layout.content_area.width),
        height: layout.content_area.height.saturating_sub(1),
    };

    let calendar = Monthly::new(to_time_date(state.display_month), events)
        .show_weekdays_header(Style::new().fg(Color::Gray).dim().bold())
        .default_style(Style::new().fg(Color::Gray).dim());

    f.render_widget(calendar, calendar_area);

    let legend_area = Rect {
        x: layout.content_area.x,
        y: layout.query_area.y.saturating_sub(1),
        width: layout.content_area.width,
        height: 1,
    };

    let dim = Style::new().dim();
    let legend_line = Line::from(vec![
        Span::styled("● ", Style::new().fg(Color::Yellow)),
        Span::styled("tasks  ", dim),
        Span::styled("● ", Style::new().fg(Color::Magenta)),
        Span::styled("events  ", dim),
        Span::styled("● ", Style::new().fg(Color::Blue)),
        Span::styled("cal", dim),
    ]);

    f.render_widget(Paragraph::new(legend_line), legend_area);
}
