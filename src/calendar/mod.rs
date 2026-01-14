mod fetch;
mod parse;
mod store;

pub use fetch::fetch_calendar;
pub use parse::{IcsParseResult, ParseContext, parse_ics};
pub use store::{CalendarEvent, CalendarFetchStatus, CalendarStore};

use chrono::{Duration, Local, NaiveDate};

use crate::config::{CalendarConfig, CalendarVisibilityConfig, CalendarVisibilityMode, Config};
use crate::storage::{JournalSlot, ProjectInfo};

pub struct CalendarFetchResult {
    pub events: Vec<CalendarEvent>,
    pub visible_count: usize,
    pub errors: Vec<(String, String)>,
}

#[must_use]
pub fn get_visible_calendar_ids(
    config: &Config,
    slot: &JournalSlot,
    project: Option<&ProjectInfo>,
) -> Vec<String> {
    if matches!(slot, JournalSlot::Hub) {
        return config.enabled_calendar_ids();
    }

    if let Some(proj) = project
        && let Some(ref cal_ids) = proj.calendars
    {
        return cal_ids
            .iter()
            .filter(|id| config.get_calendar(id).is_some_and(|c| c.enabled))
            .cloned()
            .collect();
    }

    match config.calendar_visibility.default_mode {
        CalendarVisibilityMode::All => config.enabled_calendar_ids(),
        CalendarVisibilityMode::None => Vec::new(),
    }
}

pub async fn fetch_all_calendars(config: &Config, visible_ids: &[String]) -> CalendarFetchResult {
    let today = Local::now().date_naive();
    let range_start = today - Duration::days(180);
    let range_end = today + Duration::days(365);
    let visibility = &config.calendar_visibility;

    let mut all_events = Vec::new();
    let mut errors = Vec::new();

    for cal_id in visible_ids {
        let Some(cal_config) = config.get_calendar(cal_id) else {
            continue;
        };

        let color = config.calendar_color(cal_id);
        match fetch_and_parse_calendar(
            cal_id,
            cal_config,
            range_start,
            range_end,
            visibility,
            color,
        )
        .await
        {
            Ok(events) => all_events.extend(events),
            Err(e) => errors.push((cal_id.clone(), e)),
        }
    }

    CalendarFetchResult {
        events: all_events,
        visible_count: visible_ids.len(),
        errors,
    }
}

async fn fetch_and_parse_calendar(
    cal_id: &str,
    config: &CalendarConfig,
    range_start: NaiveDate,
    range_end: NaiveDate,
    visibility: &CalendarVisibilityConfig,
    color: ratatui::style::Color,
) -> Result<Vec<CalendarEvent>, String> {
    let ics_content = fetch_calendar(&config.url).await?;
    let ctx = ParseContext {
        calendar_id: cal_id,
        calendar_name: cal_id,
        range_start,
        range_end,
        display_cancelled: visibility.display_cancelled,
        display_declined: visibility.display_declined,
        color,
    };
    let result = parse_ics(&ics_content, &ctx)?;
    Ok(result.events)
}

pub fn update_store(store: &mut CalendarStore, result: CalendarFetchResult) {
    store.update(result.events, result.visible_count);

    if !result.errors.is_empty() {
        let error_msg = result
            .errors
            .iter()
            .map(|(id, e)| format!("{id}: {e}"))
            .collect::<Vec<_>>()
            .join("; ");
        store.set_error(error_msg);
    }
}
