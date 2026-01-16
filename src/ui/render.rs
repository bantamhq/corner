use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line as RatatuiLine, Span};
use ratatui::widgets::{Borders, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, InputMode, SidebarType, ViewMode};

use super::agenda_widget::{AgendaVariant, build_agenda_widget};
use super::autocomplete::render_autocomplete_dropdown;
use super::calendar::{CalendarModel, render_calendar};
use super::container::{ContainerConfig, render_container_in_area, render_list};
use super::context::RenderContext;
use super::header::render_header_bar;
use super::layout::layout_nodes;
use super::overlay::{OverlayLayout, render_overlays};
use super::prep::prepare_render;
use super::scroll::set_edit_cursor;
use super::theme;
use super::view_model::{PanelContent, build_view_model};

pub fn render_app(f: &mut Frame<'_>, app: &mut App) {
    if app.active_sidebar().is_some() {
        app.ensure_agenda_cache();
    }

    let base_context = RenderContext::new(f.area());
    let sidebar_width = match app.active_sidebar() {
        Some(SidebarType::Calendar) => CalendarModel::panel_width(),
        Some(SidebarType::Agenda) => {
            let max_width = base_context
                .main_area
                .width
                .saturating_sub(theme::AGENDA_MIN_GUTTER);
            let min_width = CalendarModel::panel_width();
            if let Some(ref cache) = app.agenda_cache {
                let agenda = build_agenda_widget(cache, max_width as usize, AgendaVariant::Full);
                (agenda.required_width() as u16 + theme::AGENDA_BORDER_WIDTH as u16)
                    .max(min_width)
                    .min(max_width)
            } else {
                min_width
            }
        }
        None => 0,
    };
    let context = base_context.with_sidebar(sidebar_width);

    let prep = prepare_render(app, &context);
    let view_model = build_view_model(app, &context, prep);

    render_header_bar(f, context.header_area, view_model.header);
    render_view_heading(f, &context, app);

    let mut list_content_area = None;
    let mut primary_panel_area = None;

    for (panel_id, rect) in layout_nodes(context.content_area, &view_model.layout) {
        if let Some(panel) = view_model.panels.get(panel_id) {
            let focused = view_model.focused_panel == Some(panel_id);
            let container_layout = render_container_in_area(f, rect, &panel.config, focused);
            if view_model.primary_list_panel == Some(panel_id) {
                list_content_area = Some(container_layout.content_area);
                primary_panel_area = Some(container_layout.main_area);
            }
            match &panel.content {
                PanelContent::EntryList(list) => {
                    render_list(f, list, &container_layout, &app.surface);
                }
                PanelContent::Empty => {}
            }
        }
    }

    if let Some(main_area) = primary_panel_area {
        render_status_indicator(f, app, main_area);
    }

    if let Some(sidebar_area) = context.sidebar_area {
        match app.active_sidebar() {
            Some(SidebarType::Calendar) => render_calendar_sidebar(f, app, sidebar_area),
            Some(SidebarType::Agenda) => render_agenda_sidebar(f, app, sidebar_area),
            None => {}
        }
    }

    if let (Some(cursor), Some(content_area)) = (view_model.cursor.edit.as_ref(), list_content_area)
    {
        set_edit_cursor(f, cursor, app.scroll_offset(), content_area);
        render_autocomplete_dropdown(f, app, cursor, content_area);
    }

    // Render filter prompt autocomplete (positioned below heading)
    if matches!(app.input_mode, InputMode::FilterPrompt) {
        render_filter_prompt_autocomplete(f, app, &context);
    }

    let journal_name = app.journal_display_name();
    let journal_slot = app.active_journal();
    render_footer_bar(
        f,
        context.footer_area,
        &app.view,
        &app.input_mode,
        &journal_name,
        journal_slot,
        &app.surface,
    );

    render_overlays(
        f,
        view_model.overlays,
        OverlayLayout {
            screen_area: context.size,
            surface: &app.surface,
        },
    );
}

fn render_footer_bar(
    f: &mut Frame<'_>,
    area: Rect,
    view: &ViewMode,
    input_mode: &InputMode,
    journal_name: &str,
    journal_slot: crate::storage::JournalSlot,
    surface: &super::surface::Surface,
) {
    use ratatui::layout::{Constraint, Direction, Layout};

    let bg = surface.gray1;

    let (mode_label, mode_color) = match input_mode {
        InputMode::Edit(_) => ("Edit", theme::EDIT_PRIMARY),
        InputMode::Selection(_) => ("Select", theme::EDIT_PRIMARY),
        InputMode::Reorder => ("Reorder", theme::EDIT_PRIMARY),
        _ => match view {
            ViewMode::Daily(_) => ("Daily", theme::DAILY_PRIMARY),
            ViewMode::Filter(_) => ("Filter", theme::FILTER_PRIMARY),
        },
    };
    let mode_text = format!(" {} ", mode_label);
    let mode_width = mode_text.len() as u16;

    let journal_color = match journal_slot {
        crate::storage::JournalSlot::Hub => theme::HUB_PRIMARY,
        crate::storage::JournalSlot::Project => theme::PROJECT_PRIMARY,
    };
    let journal_text = format!(" {} ", journal_name);
    let journal_width = journal_text.len() as u16;

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(1),             // left padding (default bg)
            Constraint::Length(mode_width),    // mode indicator
            Constraint::Min(0),                // flexible middle (gray bg)
            Constraint::Length(journal_width), // journal indicator
            Constraint::Length(1),             // right padding (default bg)
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(Span::styled(
            mode_text,
            Style::default().fg(theme::TEXT_ON_ACCENT).bg(mode_color),
        )),
        layout[1],
    );

    f.render_widget(
        Paragraph::new(Span::styled(
            " ".repeat(layout[2].width as usize),
            Style::default().bg(bg),
        )),
        layout[2],
    );

    f.render_widget(
        Paragraph::new(Span::styled(
            journal_text,
            Style::default().fg(theme::TEXT_ON_ACCENT).bg(journal_color),
        )),
        layout[3],
    );
}

fn render_view_heading(f: &mut Frame<'_>, context: &RenderContext, app: &App) {
    let heading_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(context.heading_area);

    let heading_row = heading_layout[0];
    let rule_row = heading_layout[1];

    let is_filter_prompt = matches!(app.input_mode, InputMode::FilterPrompt);

    let (label, color) = match &app.view {
        ViewMode::Daily(_) => {
            let date_label =
                super::shared::format_date_smart(app.current_date, &app.config.header_date_format);
            let color = theme::context_primary(app.active_journal());
            (date_label, color)
        }
        ViewMode::Filter(state) => {
            let query_text = if is_filter_prompt {
                state.query_buffer.content().to_string()
            } else {
                state.query.clone()
            };
            let filter_label = format!("Filter: {}", query_text);
            let color = theme::context_primary(app.active_journal());
            (filter_label, color)
        }
    };

    let label_width = label.width();
    let line_spans = vec![
        Span::raw(" ".repeat(theme::HEADING_PADDING)),
        Span::styled(
            label,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ];
    let heading_line = RatatuiLine::from(line_spans);
    f.render_widget(Paragraph::new(heading_line), heading_row);

    // Set cursor position when in filter prompt mode
    if is_filter_prompt && let ViewMode::Filter(state) = &app.view {
        let prefix = "Filter: ";
        let cursor_x = heading_row.x
            + theme::HEADING_PADDING as u16
            + prefix.width() as u16
            + state.query_buffer.cursor_display_pos() as u16;
        f.set_cursor_position((cursor_x, heading_row.y));
    }

    let rule_width = rule_row.width as usize;
    let highlight_start = theme::HEADING_PADDING;
    let highlight_len = label_width.min(rule_width.saturating_sub(highlight_start));
    let after_len = rule_width.saturating_sub(highlight_start + highlight_len);

    let mut rule_spans = Vec::new();
    if highlight_start > 0 {
        rule_spans.push(Span::styled(
            "─".repeat(highlight_start),
            Style::default().fg(theme::TEXT_MUTED),
        ));
    }
    if highlight_len > 0 {
        rule_spans.push(Span::styled(
            "─".repeat(highlight_len),
            Style::default().fg(color),
        ));
    }
    if after_len > 0 {
        rule_spans.push(Span::styled(
            "─".repeat(after_len),
            Style::default().fg(theme::TEXT_MUTED),
        ));
    }

    let rule_line = RatatuiLine::from(rule_spans);
    f.render_widget(Paragraph::new(rule_line), rule_row);
}

fn render_calendar_sidebar(f: &mut Frame<'_>, app: &App, sidebar_area: Rect) {
    let calendar_state = app.calendar_state();

    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(theme::CALENDAR_PANEL_HEIGHT),
            Constraint::Min(theme::UPCOMING_MIN_HEIGHT),
        ])
        .split(sidebar_area);

    let calendar_area = split[0];
    let upcoming_area = split[1];

    let calendar_config = ContainerConfig {
        title: Some(RatatuiLine::from(
            calendar_state.display_month.format(" %B %Y ").to_string(),
        )),
        border_color: theme::BORDER_DEFAULT,
        focused_border_color: None,
        padded: false,
        borders: Borders::ALL,
        rounded: true,
        bottom_buffer: 0,
    };

    let calendar_layout = render_container_in_area(f, calendar_area, &calendar_config, false);
    let calendar_model = CalendarModel {
        selected: calendar_state.selected,
        display_month: calendar_state.display_month,
        day_cache: &calendar_state.day_cache,
        journal_slot: app.active_journal(),
    };
    render_calendar(f, &calendar_model, calendar_layout.content_area);

    let upcoming_config = ContainerConfig {
        title: Some(RatatuiLine::from(" Upcoming ")),
        border_color: theme::BORDER_DEFAULT,
        focused_border_color: None,
        padded: false,
        borders: Borders::ALL,
        rounded: true,
        bottom_buffer: 0,
    };

    let upcoming_layout = render_container_in_area(f, upcoming_area, &upcoming_config, false);
    if let Some(ref cache) = app.agenda_cache {
        let agenda = build_agenda_widget(
            cache,
            upcoming_layout.content_area.width as usize,
            AgendaVariant::Mini,
        );
        let lines = agenda.render_lines();
        let content = Paragraph::new(lines);
        f.render_widget(content, upcoming_layout.content_area);
    }
}

fn render_agenda_sidebar(f: &mut Frame<'_>, app: &App, sidebar_area: Rect) {
    let config = ContainerConfig {
        title: Some(RatatuiLine::from(" Agenda ")),
        border_color: theme::BORDER_DEFAULT,
        focused_border_color: None,
        padded: false,
        borders: Borders::ALL,
        rounded: true,
        bottom_buffer: 0,
    };

    let layout = render_container_in_area(f, sidebar_area, &config, false);
    if let Some(ref cache) = app.agenda_cache {
        let agenda = build_agenda_widget(
            cache,
            layout.content_area.width as usize,
            AgendaVariant::Full,
        );
        let lines = agenda.render_lines();
        let content = Paragraph::new(lines);
        f.render_widget(content, layout.content_area);
    }
}

fn render_status_indicator(f: &mut Frame<'_>, app: &App, main_area: Rect) {
    let Some(ref status) = app.status_message else {
        return;
    };

    let bg_color = theme::panel_bg(&app.surface);
    let border_color = if status.is_error {
        theme::STATUS_ERROR
    } else {
        theme::context_primary(app.active_journal())
    };

    let bg_style = Style::default().bg(bg_color);
    let border_span = Span::styled("▌", Style::default().fg(border_color).bg(bg_color));
    let text_span = Span::styled(
        status.text.clone(),
        Style::default().fg(theme::STATUS_TEXT).bg(bg_color),
    );
    let padding_span = Span::styled(" ", bg_style);
    let line = RatatuiLine::from(vec![border_span, text_span, padding_span]);

    let status_area = Rect {
        x: main_area.x + 1,
        y: main_area.y + main_area.height.saturating_sub(2),
        width: main_area.width.saturating_sub(2),
        height: 1,
    };

    f.render_widget(Paragraph::new(line), status_area);
}

fn render_filter_prompt_autocomplete(f: &mut Frame<'_>, app: &App, context: &RenderContext) {
    use super::autocomplete::{
        MAX_SUGGESTIONS, build_dropdown_lines, render_dropdown_box, token_display_len,
    };

    if !app.hint_state.is_active() {
        return;
    }

    let items = app.hint_state.display_items("");
    if items.is_empty() {
        return;
    }

    let ViewMode::Filter(state) = &app.view else {
        return;
    };

    let prefix = "Filter: ";
    let cursor_x = context.heading_area.x
        + theme::HEADING_PADDING as u16
        + prefix.width() as u16
        + state.query_buffer.cursor_display_pos() as u16;

    let token_len = token_display_len(&app.hint_state) as u16;
    let start_x = cursor_x.saturating_sub(token_len + 1);
    let start_y = context.heading_area.y + 2;

    let window_len = items.len().min(MAX_SUGGESTIONS);
    let width = 20u16;
    let height = (window_len as u16) + 2;

    let area = Rect {
        x: start_x,
        y: start_y,
        width,
        height,
    };

    let text_width = width.saturating_sub(2) as usize;
    let lines = build_dropdown_lines(
        &items,
        app.hint_state.selected_index(),
        app.hint_state.scroll_offset(),
        app.hint_state.color(),
        text_width,
    );
    render_dropdown_box(f, area, lines);
}
