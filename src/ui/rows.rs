use chrono::Timelike;
use ratatui::{
    style::{Color, Style, Stylize},
    text::Span,
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, InputMode};
use crate::calendar::CalendarEvent;
use crate::storage::{Entry, EntryType, RawEntry, SourceType};

use super::model::RowModel;
use super::shared::{
    date_suffix_style, entry_style, format_date_suffix, style_content, truncate_with_tags,
    wrap_text,
};
use super::theme;

pub fn build_calendar_row(
    event: &CalendarEvent,
    width: usize,
    show_calendar_name: bool,
) -> RowModel {
    let prefix = "* ";
    let prefix_width = prefix.width();
    let indicator = theme::GLYPH_CALENDAR.to_string();

    let content = format_calendar_event(event, show_calendar_name);
    let available = width.saturating_sub(prefix_width);
    let display_text = truncate_with_tags(&content, available);

    let content_style = if event.is_cancelled || event.is_declined {
        Style::default().italic().crossed_out()
    } else {
        Style::default().italic()
    };
    let (_, rest_of_prefix) = split_prefix(prefix);

    RowModel::new(
        Some(Span::styled(indicator, Style::default().fg(event.color))),
        Some(Span::styled(rest_of_prefix, content_style)),
        style_content(&display_text, content_style),
        None,
    )
}

pub fn build_projected_row(
    app: &App,
    projected_entry: &Entry,
    is_selected: bool,
    visible_idx: usize,
    width: usize,
) -> RowModel {
    let (source_suffix, _) = format_date_suffix(projected_entry.source_date);
    build_entry_row(
        app,
        EntryRowSpec {
            entry_type: &projected_entry.entry_type,
            text: &projected_entry.content,
            width,
            is_selected,
            visible_idx,
            indicator: EntryIndicator::Projected(&projected_entry.source_type),
            suffix: EntrySuffix::Date(source_suffix),
        },
    )
}

pub fn build_daily_entry_row(
    app: &App,
    entry: &RawEntry,
    is_selected: bool,
    visible_idx: usize,
    width: usize,
) -> RowModel {
    build_entry_row(
        app,
        EntryRowSpec {
            entry_type: &entry.entry_type,
            text: &entry.content,
            width,
            is_selected,
            visible_idx,
            indicator: EntryIndicator::Daily,
            suffix: EntrySuffix::None,
        },
    )
}

pub fn build_filter_selected_row(app: &App, entry: &Entry, index: usize, width: usize) -> RowModel {
    let content_style = entry_style(&entry.entry_type);
    let text = entry.content.clone();
    let prefix = entry.entry_type.prefix();
    let prefix_width = prefix.width();
    let (date_suffix, date_suffix_width) = format_date_suffix(entry.source_date);

    let (_, rest_of_prefix) = split_prefix(prefix);
    let available = width.saturating_sub(prefix_width + date_suffix_width);
    let display_text = truncate_with_tags(&text, available);

    let resolver = IndicatorResolver::new(app);
    RowModel::new(
        Some(resolver.filter_cursor_indicator(index)),
        Some(Span::styled(rest_of_prefix, content_style)),
        style_content(&display_text, content_style),
        Some(Span::styled(date_suffix, date_suffix_style(content_style))),
    )
}

#[derive(Copy, Clone)]
enum EntryIndicator<'a> {
    Daily,
    Filter,
    Projected(&'a SourceType),
}

enum EntrySuffix {
    None,
    Date(String),
}

struct EntryRowSpec<'a> {
    entry_type: &'a EntryType,
    text: &'a str,
    width: usize,
    is_selected: bool,
    visible_idx: usize,
    indicator: EntryIndicator<'a>,
    suffix: EntrySuffix,
}

fn build_entry_row(app: &App, spec: EntryRowSpec<'_>) -> RowModel {
    let content_style = entry_style(spec.entry_type);
    let prefix = spec.entry_type.prefix();
    let prefix_width = prefix.width();

    let (suffix_text, suffix_width) = match spec.suffix {
        EntrySuffix::None => (None, 0),
        EntrySuffix::Date(text) => {
            let width = text.width();
            (Some(text), width)
        }
    };

    let available = spec.width.saturating_sub(prefix_width + suffix_width);
    let display_text = truncate_with_tags(spec.text, available);

    let (first_char, rest_of_prefix) = split_prefix(prefix);
    let resolver = IndicatorResolver::new(app);
    let indicator = match spec.indicator {
        EntryIndicator::Daily => resolver.entry_indicator(
            spec.is_selected,
            spec.visible_idx,
            resolver.cursor_color(),
            &first_char,
            content_style,
        ),
        EntryIndicator::Filter => {
            resolver.filter_list_indicator(&first_char, spec.visible_idx, content_style)
        }
        EntryIndicator::Projected(source_type) => match source_type {
            SourceType::Later => resolver.entry_indicator(
                spec.is_selected,
                spec.visible_idx,
                theme::PROJECTED_DATE,
                &first_char,
                content_style,
            ),
            _ => resolver.projected_indicator(spec.is_selected, source_type, content_style),
        },
    };

    let suffix_span = suffix_text.map(|text| Span::styled(text, date_suffix_style(content_style)));

    RowModel::new(
        Some(indicator),
        Some(Span::styled(rest_of_prefix, content_style)),
        style_content(&display_text, content_style),
        suffix_span,
    )
}

pub fn build_filter_row(app: &App, entry: &Entry, index: usize, width: usize) -> RowModel {
    let (date_suffix, _) = format_date_suffix(entry.source_date);
    build_entry_row(
        app,
        EntryRowSpec {
            entry_type: &entry.entry_type,
            text: &entry.content,
            width,
            is_selected: false,
            visible_idx: index,
            indicator: EntryIndicator::Filter,
            suffix: EntrySuffix::Date(date_suffix),
        },
    )
}

pub fn build_edit_rows_with_prefix_width(
    prefix: &str,
    prefix_width: usize,
    content_style: Style,
    text: &str,
    text_width: usize,
    suffix: Option<Span<'static>>,
) -> Vec<RowModel> {
    let wrap_width = text_width.saturating_sub(1).max(1);
    let wrapped = wrap_text(text, wrap_width);

    if wrapped.is_empty() {
        return vec![RowModel::new(
            None,
            Some(Span::styled(prefix.to_string(), content_style)),
            Vec::new(),
            suffix,
        )];
    }

    wrapped
        .iter()
        .enumerate()
        .map(|(i, line_text)| {
            let prefix_text = if i == 0 {
                prefix.to_string()
            } else {
                " ".repeat(prefix_width)
            };
            RowModel::new(
                None,
                Some(Span::styled(prefix_text, content_style)),
                style_content(line_text, content_style),
                if i == 0 { suffix.clone() } else { None },
            )
        })
        .collect()
}

pub fn build_message_row(message: &str, style: Style) -> RowModel {
    RowModel::from_spans(vec![Span::styled(message.to_string(), style)])
}

fn split_prefix(prefix: &str) -> (String, String) {
    let mut chars = prefix.chars();
    let first_char = chars.next().unwrap_or('-');
    let rest: String = chars.collect();
    (first_char.to_string(), rest)
}

struct IndicatorResolver<'a> {
    app: &'a App,
}

impl<'a> IndicatorResolver<'a> {
    fn new(app: &'a App) -> Self {
        Self { app }
    }

    fn selection_active(&self, index: usize) -> bool {
        if let InputMode::Selection(ref state) = self.app.input_mode {
            state.is_selected(index)
        } else {
            false
        }
    }

    fn cursor_color(&self) -> Color {
        theme::context_primary(self.app.active_journal())
    }

    fn filter_cursor_indicator(&self, index: usize) -> Span<'static> {
        let in_selection_mode = matches!(self.app.input_mode, InputMode::Selection(_));
        let glyph = if self.selection_active(index) {
            theme::GLYPH_SELECTED
        } else {
            theme::GLYPH_CURSOR
        };
        let color = if in_selection_mode || self.selection_active(index) {
            theme::EDIT_PRIMARY
        } else {
            self.cursor_color()
        };
        Span::styled(glyph, Style::default().fg(color))
    }

    fn filter_list_indicator(
        &self,
        first_char: &str,
        index: usize,
        content_style: Style,
    ) -> Span<'static> {
        if self.selection_active(index) {
            Span::styled(
                theme::GLYPH_UNSELECTED,
                Style::default().fg(theme::EDIT_PRIMARY),
            )
        } else {
            Span::styled(first_char.to_string(), content_style)
        }
    }

    fn projected_indicator(
        &self,
        is_cursor: bool,
        kind: &SourceType,
        content_style: Style,
    ) -> Span<'static> {
        let indicator = match kind {
            SourceType::Later => theme::GLYPH_PROJECTED_LATER,
            SourceType::Recurring => theme::GLYPH_PROJECTED_RECURRING,
            SourceType::Local => unreachable!("projected entries are never Local"),
            SourceType::Calendar { .. } => theme::GLYPH_PROJECTED_CALENDAR,
        };

        if is_cursor {
            Span::styled(indicator, Style::default().fg(theme::PROJECTED_DATE))
        } else {
            Span::styled(indicator.to_string(), content_style)
        }
    }

    fn entry_indicator(
        &self,
        is_cursor: bool,
        visible_idx: usize,
        cursor_color: Color,
        default_first_char: &str,
        default_style: Style,
    ) -> Span<'static> {
        let is_selected_in_selection = self.selection_active(visible_idx);

        if is_cursor {
            if matches!(self.app.input_mode, InputMode::Reorder) {
                Span::styled(
                    theme::GLYPH_REORDER,
                    Style::default().fg(theme::EDIT_PRIMARY),
                )
            } else if matches!(self.app.input_mode, InputMode::Selection(_)) {
                let glyph = if is_selected_in_selection {
                    theme::GLYPH_SELECTED
                } else {
                    theme::GLYPH_CURSOR
                };
                Span::styled(glyph, Style::default().fg(theme::EDIT_PRIMARY))
            } else {
                Span::styled(theme::GLYPH_CURSOR, Style::default().fg(cursor_color))
            }
        } else if is_selected_in_selection {
            Span::styled(
                theme::GLYPH_UNSELECTED,
                Style::default().fg(theme::EDIT_PRIMARY),
            )
        } else {
            Span::styled(default_first_char.to_string(), default_style)
        }
    }
}

fn format_calendar_event(event: &CalendarEvent, show_calendar_name: bool) -> String {
    let mut parts = vec![event.title.clone()];

    if let Some((day, total)) = event.multi_day_info {
        parts.push(format!("{day}/{total}"));
    }

    if !event.is_all_day {
        let start_hour = event.start.hour();
        let end_hour = event.end.hour();
        let same_period = (start_hour < 12) == (end_hour < 12);

        let start_fmt = if same_period { "%-I:%M" } else { "%-I:%M%P" };
        let time_str = format!(
            "{}-{}",
            event.start.format(start_fmt),
            event.end.format("%-I:%M%P")
        );
        parts.push(time_str);
    }

    let main_text = parts.join(" - ");
    if show_calendar_name {
        format!("{main_text} ({})", event.calendar_name)
    } else {
        main_text
    }
}
