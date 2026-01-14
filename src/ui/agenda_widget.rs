use std::path::Path;

use chrono::{Local, NaiveDate};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::calendar::CalendarStore;
use crate::storage::{self, EntryType, SourceType};

use super::shared::truncate_text;
use super::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AgendaVariant {
    Mini, // compact, no times, no spacing - shown as "Upcoming" in calendar sidebar
    Full, // spacious, with times, with spacing - shown as "Agenda" sidebar
}

#[derive(Clone)]
pub struct AgendaDayModel {
    pub date: NaiveDate,
    pub entries: Vec<AgendaEntryModel>,
}

#[derive(Clone)]
pub struct AgendaEntryModel {
    pub prefix: char,
    pub text: String,
    pub text_with_time: Option<String>,
    pub style: Style,
}

#[derive(Clone)]
pub struct AgendaCache {
    pub days: Vec<AgendaDayModel>,
    pub max_width: usize,
    pub max_width_with_times: usize,
}

pub struct AgendaWidgetModel<'a> {
    pub days: &'a [AgendaDayModel],
    pub width: usize,
    pub variant: AgendaVariant,
}

impl AgendaWidgetModel<'_> {
    #[must_use]
    pub fn required_width(&self) -> usize {
        self.width
    }

    pub fn render_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let content_width = self.width.saturating_sub(theme::AGENDA_BORDER_WIDTH);

        for (i, day) in self.days.iter().enumerate() {
            if i > 0 && self.variant == AgendaVariant::Full {
                lines.push(Line::from(""));
            }
            let date_str = day.date.format("%m/%d/%y").to_string();
            lines.push(Line::from(Span::styled(
                format!(" {date_str}"),
                Style::default().add_modifier(Modifier::DIM),
            )));

            for entry in &day.entries {
                let prefix = format!(" {} ", entry.prefix);
                let prefix_width = prefix.width();
                let max_text = content_width.saturating_sub(prefix_width);
                let text = match self.variant {
                    AgendaVariant::Full => entry.text_with_time.as_deref().unwrap_or(&entry.text),
                    AgendaVariant::Mini => &entry.text,
                };
                let text = truncate_text(text, max_text);
                lines.push(Line::from(vec![
                    Span::styled(prefix, entry.style),
                    Span::raw(text),
                ]));
            }
        }

        lines
    }
}

pub fn build_agenda_widget<'a>(
    cache: &'a AgendaCache,
    width: usize,
    variant: AgendaVariant,
) -> AgendaWidgetModel<'a> {
    let max_width = match variant {
        AgendaVariant::Full => cache.max_width_with_times,
        AgendaVariant::Mini => cache.max_width,
    };
    AgendaWidgetModel {
        days: &cache.days,
        width: width.min(max_width),
        variant,
    }
}

pub fn collect_agenda_cache(calendar_store: &CalendarStore, path: &Path) -> AgendaCache {
    let today = Local::now().date_naive();
    let mut days = Vec::new();
    let mut max_width = theme::AGENDA_DATE_WIDTH;
    let mut max_width_with_times = theme::AGENDA_DATE_WIDTH;
    let mut total_entries = 0usize;

    for day_offset in 0..theme::AGENDA_MAX_DAYS_SEARCH {
        let date = today + chrono::Duration::days(day_offset);
        let mut entries = Vec::new();

        for event in calendar_store.events_for_date(date) {
            let text = event.title.clone();
            let text_with_time = if event.is_all_day {
                None
            } else {
                Some(format!(
                    "{} {}",
                    event.start.format("%-I:%M%P"),
                    event.title
                ))
            };
            let entry_width = text.width() + theme::AGENDA_ENTRY_PADDING;
            max_width = max_width.max(entry_width);
            let time_width = text_with_time
                .as_ref()
                .map_or(entry_width, |t| t.width() + theme::AGENDA_ENTRY_PADDING);
            max_width_with_times = max_width_with_times.max(time_width);
            entries.push(AgendaEntryModel {
                prefix: theme::GLYPH_CALENDAR,
                text,
                text_with_time,
                style: Style::default().fg(event.color),
            });
        }

        if let Ok(projected) = storage::collect_projected_entries_for_date(date, path) {
            for entry in projected.iter() {
                let is_recurring = entry.source_type == SourceType::Recurring;
                let is_event = entry.entry_type == EntryType::Event;
                if !is_recurring && !is_event {
                    continue;
                }
                let (prefix, style) =
                    projected_entry_prefix_and_style(&entry.source_type, &entry.entry_type);
                let text = truncate_to_first_tag(&entry.content);
                let entry_width = text.width() + theme::AGENDA_ENTRY_PADDING;
                max_width = max_width.max(entry_width);
                max_width_with_times = max_width_with_times.max(entry_width);
                entries.push(AgendaEntryModel {
                    prefix,
                    text,
                    text_with_time: None,
                    style,
                });
            }
        }

        if let Ok(day_lines) = storage::load_day_lines(date, path) {
            for line in &day_lines {
                if let storage::Line::Entry(raw) = line {
                    if raw.entry_type != EntryType::Event {
                        continue;
                    }
                    let text = truncate_to_first_tag(&raw.content);
                    let entry_width = text.width() + theme::AGENDA_ENTRY_PADDING;
                    max_width = max_width.max(entry_width);
                    max_width_with_times = max_width_with_times.max(entry_width);
                    entries.push(AgendaEntryModel {
                        prefix: theme::GLYPH_AGENDA_EVENT,
                        text,
                        text_with_time: None,
                        style: Style::default().add_modifier(Modifier::ITALIC),
                    });
                }
            }
        }

        if !entries.is_empty() {
            total_entries += entries.len();
            days.push(AgendaDayModel { date, entries });
        }

        if total_entries >= theme::AGENDA_MIN_ENTRIES {
            break;
        }
    }

    AgendaCache {
        days,
        max_width,
        max_width_with_times,
    }
}

fn projected_entry_prefix_and_style(
    source_type: &SourceType,
    entry_type: &EntryType,
) -> (char, Style) {
    if *source_type == SourceType::Recurring {
        return (theme::GLYPH_AGENDA_RECURRING, Style::default());
    }
    if *entry_type == EntryType::Event {
        return (
            theme::GLYPH_AGENDA_EVENT,
            Style::default().add_modifier(Modifier::ITALIC),
        );
    }
    (theme::GLYPH_AGENDA_FALLBACK, Style::default())
}

fn truncate_to_first_tag(content: &str) -> String {
    if let Some(pos) = content.find('#') {
        content[..pos].trim().to_string()
    } else {
        content.to_string()
    }
}
