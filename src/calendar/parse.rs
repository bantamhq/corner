use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use ical::parser::ical::component::IcalEvent;
use ical::parser::ical::IcalParser;
use ical::property::Property;
use rrule::{RRuleSet, Tz as RRuleTz};
use std::collections::{HashMap, HashSet};
use std::io::BufReader;

use super::CalendarEvent;

const MAX_RECURRENCE_OCCURRENCES: u16 = 500;

type IcalParams = Option<Vec<(String, Vec<String>)>>;

pub struct IcsParseResult {
    pub events: Vec<CalendarEvent>,
}

pub fn parse_ics(
    content: &str,
    calendar_id: &str,
    calendar_name: &str,
    range_start: NaiveDate,
    range_end: NaiveDate,
    display_cancelled: bool,
    display_declined: bool,
) -> Result<IcsParseResult, String> {
    let reader = BufReader::new(content.as_bytes());
    let parser = IcalParser::new(reader);

    // First pass: collect all RECURRENCE-ID exceptions per UID
    // These are instances that override a specific occurrence of a recurring event
    let mut recurrence_exceptions: HashMap<String, HashSet<NaiveDate>> = HashMap::new();
    let mut all_events: Vec<IcalEvent> = Vec::new();

    for calendar in parser {
        let calendar = calendar.map_err(|e| format!("Failed to parse ICS: {e:?}"))?;

        for event in calendar.events {
            if let Some(uid) = get_property(&event, "UID")
                && let Some(recurrence_id) = get_property(&event, "RECURRENCE-ID")
            {
                // Extract date from RECURRENCE-ID (can be date or datetime)
                let date_str = &recurrence_id[..8.min(recurrence_id.len())];
                if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y%m%d") {
                    recurrence_exceptions.entry(uid).or_default().insert(date);
                }
            }
            all_events.push(event);
        }
    }

    // Second pass: parse events, skipping recurrence dates that have exceptions
    let mut events = Vec::new();

    for event in &all_events {
        if let Some(parsed) = parse_event(
            event,
            calendar_id,
            calendar_name,
            range_start,
            range_end,
            &recurrence_exceptions,
            display_cancelled,
            display_declined,
        )? {
            events.extend(parsed);
        }
    }

    Ok(IcsParseResult { events })
}

fn parse_event(
    event: &IcalEvent,
    calendar_id: &str,
    calendar_name: &str,
    range_start: NaiveDate,
    range_end: NaiveDate,
    recurrence_exceptions: &HashMap<String, HashSet<NaiveDate>>,
    display_cancelled: bool,
    display_declined: bool,
) -> Result<Option<Vec<CalendarEvent>>, String> {
    let uid = get_property(event, "UID").unwrap_or_default();
    let summary = get_property(event, "SUMMARY").unwrap_or_default();

    if summary.is_empty() {
        return Ok(None);
    }

    let is_cancelled =
        get_property(event, "STATUS").is_some_and(|s| s.eq_ignore_ascii_case("CANCELLED"));
    if is_cancelled && !display_cancelled {
        return Ok(None);
    }

    let is_declined = has_declined_attendee(event);
    if is_declined && !display_declined {
        return Ok(None);
    }

    let Some(dtstart_prop) = find_property(event, "DTSTART") else {
        return Ok(None);
    };
    let dtstart_value = dtstart_prop.value.as_deref().unwrap_or_default();
    let is_all_day = is_date_value(&dtstart_prop.params) || dtstart_value.len() == 8;
    let start = parse_datetime(dtstart_value, get_tzid(&dtstart_prop.params), is_all_day)?;

    let end = if let Some(dtend_prop) = find_property(event, "DTEND") {
        let dtend_value = dtend_prop.value.as_deref().unwrap_or_default();
        parse_datetime(dtend_value, get_tzid(&dtend_prop.params), is_all_day)?
    } else if is_all_day {
        start + Duration::days(1)
    } else {
        start + Duration::hours(1)
    };

    let rrule_str = get_property(event, "RRULE");
    let mut exdates = parse_exdates(event);

    // Add recurrence exceptions (RECURRENCE-ID instances) to exclusion set
    // This prevents duplicates when an instance has been modified
    if let Some(exceptions) = recurrence_exceptions.get(&uid) {
        exdates.extend(exceptions);
    }

    let occurrences = if let Some(rrule) = rrule_str {
        expand_rrule(&start, &rrule, &exdates, range_start, range_end)?
    } else {
        let start_date = start.date_naive();
        if start_date >= range_start && start_date <= range_end {
            vec![start]
        } else {
            vec![]
        }
    };

    let event_duration = end - start;
    let is_multi_day = is_all_day && event_duration > Duration::days(1);
    let total_days = if is_multi_day {
        (event_duration.num_days() as u8).max(1)
    } else {
        1
    };

    let mut result = Vec::new();

    for occ_start in occurrences {
        let occ_end = occ_start + event_duration;

        if is_multi_day {
            for day_num in 0..total_days {
                let day_start = occ_start + Duration::days(i64::from(day_num));
                let day_date = day_start.date_naive();

                if day_date >= range_start && day_date <= range_end {
                    result.push(CalendarEvent {
                        id: format!("{uid}_{day_date}"),
                        title: summary.clone(),
                        calendar_id: calendar_id.to_string(),
                        calendar_name: calendar_name.to_string(),
                        start: day_start,
                        end: day_start + Duration::days(1),
                        is_all_day: true,
                        multi_day_info: Some((day_num + 1, total_days)),
                        is_cancelled,
                        is_declined,
                    });
                }
            }
        } else {
            let start_date = occ_start.date_naive();
            if start_date >= range_start && start_date <= range_end {
                result.push(CalendarEvent {
                    id: uid.clone(),
                    title: summary.clone(),
                    calendar_id: calendar_id.to_string(),
                    calendar_name: calendar_name.to_string(),
                    start: occ_start,
                    end: occ_end,
                    is_all_day,
                    multi_day_info: None,
                    is_cancelled,
                    is_declined,
                });
            }
        }
    }

    Ok(Some(result))
}

fn get_property(event: &IcalEvent, name: &str) -> Option<String> {
    event
        .properties
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.value.clone())
}

fn get_param<'a>(params: &'a IcalParams, name: &str) -> Option<&'a str> {
    params
        .as_ref()?
        .iter()
        .find(|(k, _)| k == name)
        .and_then(|(_, v)| v.first())
        .map(String::as_str)
}

fn is_date_value(params: &IcalParams) -> bool {
    get_param(params, "VALUE") == Some("DATE")
}

fn get_tzid(params: &IcalParams) -> Option<&str> {
    get_param(params, "TZID")
}

fn find_property<'a>(event: &'a IcalEvent, name: &str) -> Option<&'a Property> {
    event.properties.iter().find(|p| p.name == name)
}

fn parse_datetime(
    value: &str,
    tzid: Option<&str>,
    is_all_day: bool,
) -> Result<DateTime<Local>, String> {
    if is_all_day || value.len() == 8 {
        let date = NaiveDate::parse_from_str(value, "%Y%m%d")
            .map_err(|e| format!("Failed to parse date '{value}': {e}"))?;
        let naive_dt = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Local
            .from_local_datetime(&naive_dt)
            .single()
            .unwrap_or_else(Local::now));
    }

    if let Some(stripped) = value.strip_suffix('Z') {
        let naive = NaiveDateTime::parse_from_str(stripped, "%Y%m%dT%H%M%S")
            .map_err(|e| format!("Failed to parse UTC datetime '{value}': {e}"))?;
        return Ok(Utc.from_utc_datetime(&naive).with_timezone(&Local));
    }

    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")
        .map_err(|e| format!("Failed to parse datetime '{value}': {e}"))?;

    if let Some(tz_name) = tzid
        && let Ok(tz) = tz_name.parse::<Tz>()
    {
        let tz_dt = tz
            .from_local_datetime(&naive)
            .single()
            .ok_or_else(|| format!("Ambiguous datetime for timezone {tz_name}"))?;
        return Ok(tz_dt.with_timezone(&Local));
    }

    Ok(Local
        .from_local_datetime(&naive)
        .single()
        .unwrap_or_else(Local::now))
}

fn has_declined_attendee(event: &IcalEvent) -> bool {
    for prop in &event.properties {
        if prop.name == "ATTENDEE"
            && let Some(ref params) = prop.params
        {
            for (key, values) in params {
                if key == "PARTSTAT" && values.iter().any(|v| v.eq_ignore_ascii_case("DECLINED")) {
                    return true;
                }
            }
        }
    }
    false
}

fn parse_exdates(event: &IcalEvent) -> HashSet<NaiveDate> {
    let mut exdates = HashSet::new();

    for prop in &event.properties {
        if prop.name == "EXDATE"
            && let Some(value) = &prop.value
        {
            for date_str in value.split(',') {
                let date_str = date_str.trim();
                if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y%m%d") {
                    exdates.insert(date);
                } else if date_str.len() >= 8
                    && let Ok(date) = NaiveDate::parse_from_str(&date_str[..8], "%Y%m%d")
                {
                    exdates.insert(date);
                }
            }
        }
    }

    exdates
}

fn expand_rrule(
    dtstart: &DateTime<Local>,
    rrule_str: &str,
    exdates: &HashSet<NaiveDate>,
    range_start: NaiveDate,
    range_end: NaiveDate,
) -> Result<Vec<DateTime<Local>>, String> {
    let dtstart_str = dtstart.format("%Y%m%dT%H%M%S").to_string();
    let full_rrule = format!("DTSTART:{dtstart_str}\nRRULE:{rrule_str}");

    let rrule_set: RRuleSet = full_rrule
        .parse()
        .map_err(|e| format!("Failed to parse RRULE '{rrule_str}': {e:?}"))?;

    let range_start_dt = range_start
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(RRuleTz::Local(Local))
        .unwrap();
    let range_end_dt = range_end
        .and_hms_opt(23, 59, 59)
        .unwrap()
        .and_local_timezone(RRuleTz::Local(Local))
        .unwrap();

    let occurrences = rrule_set
        .after(range_start_dt)
        .before(range_end_dt)
        .all(MAX_RECURRENCE_OCCURRENCES)
        .dates
        .into_iter()
        .filter(|dt| !exdates.contains(&dt.date_naive()))
        .map(|dt| dt.with_timezone(&Local))
        .collect();

    Ok(occurrences)
}
