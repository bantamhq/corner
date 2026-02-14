use std::collections::BTreeMap;
use std::path::PathBuf;

use ratatui::{
    style::{Modifier, Style, Stylize},
    text::Span,
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, EditContext, InputMode, ViewMode};
use crate::storage::ProjectRegistry;

use super::helpers::edit_text;
use super::model::ListModel;
use super::rows;
use super::rows::build_edit_rows_with_prefix_width;
use super::shared::{date_suffix_style, entry_style, format_date_suffix};

pub fn build_filter_list(app: &App, width: usize) -> ListModel {
    if app.combined_view {
        return build_combined_filter_list(app, width);
    }

    let ViewMode::Filter(state) = &app.view else {
        return ListModel::from_rows(None, Vec::new(), app.scroll_offset());
    };

    let mut rows = Vec::new();

    let is_quick_adding = matches!(
        app.input_mode,
        InputMode::Edit(EditContext::FilterQuickAdd { .. })
    );
    let is_editing = matches!(
        app.input_mode,
        InputMode::Edit(EditContext::FilterEdit { .. })
    );

    for (idx, filter_entry) in state.entries.iter().enumerate() {
        let is_selected = idx == state.selected && !is_quick_adding;
        let is_editing_this = is_selected && is_editing;

        let content_style = entry_style(&filter_entry.entry_type);

        let text = edit_text(app, is_editing_this, &filter_entry.content);

        let prefix = filter_entry.entry_type.prefix();
        let prefix_width = prefix.width();

        if is_selected {
            if is_editing_this {
                let (date_suffix, date_suffix_width) = format_date_suffix(filter_entry.source_date);
                let text_width = width.saturating_sub(prefix_width + date_suffix_width);
                rows.extend(build_edit_rows_with_prefix_width(
                    prefix,
                    prefix_width,
                    content_style,
                    &text,
                    text_width,
                    Some(Span::styled(date_suffix, date_suffix_style(content_style))),
                ));
            } else {
                rows.push(rows::build_filter_selected_row(
                    app,
                    filter_entry,
                    idx,
                    width,
                ));
            }
        } else {
            rows.push(rows::build_filter_row(app, filter_entry, idx, width));
        }
    }

    if let InputMode::Edit(EditContext::FilterQuickAdd { entry_type, .. }) = &app.input_mode {
        let text = edit_text(app, true, "");
        let prefix = entry_type.prefix();
        let prefix_width = prefix.width();
        let text_width = width.saturating_sub(prefix_width);

        let content_style = entry_style(entry_type);
        rows.extend(build_edit_rows_with_prefix_width(
            prefix,
            prefix_width,
            content_style,
            &text,
            text_width,
            None,
        ));
    }

    if state.entries.is_empty() && !is_quick_adding {
        rows.push(rows::build_message_row(
            "(no matches)",
            Style::default().dim(),
        ));
    }

    ListModel::from_rows(None, rows, app.scroll_offset())
}

fn build_combined_filter_list(app: &App, width: usize) -> ListModel {
    use super::model::RowModel;

    let ViewMode::Filter(state) = &app.view else {
        return ListModel::from_rows(None, Vec::new(), app.scroll_offset());
    };

    let mut rows = Vec::new();

    // Group entries by source_journal, preserving order of first appearance
    let mut groups: BTreeMap<PathBuf, Vec<(usize, &crate::storage::Entry)>> = BTreeMap::new();
    for (idx, entry) in state.entries.iter().enumerate() {
        groups
            .entry(entry.source_journal.clone())
            .or_default()
            .push((idx, entry));
    }

    // Build a lookup from journal path to project name
    let registry = ProjectRegistry::load();
    let resolve_name = |path: &PathBuf| -> String {
        if path == app.journal_context.hub_path() {
            return "Hub".to_string();
        }
        for project in &registry.projects {
            if project.journal_path() == *path {
                return project.name.clone();
            }
        }
        "Unknown".to_string()
    };

    let is_quick_adding = matches!(
        app.input_mode,
        InputMode::Edit(EditContext::FilterQuickAdd { .. })
    );

    for (journal_path, entries) in &groups {
        let name = resolve_name(journal_path);
        let header_style = Style::default()
            .fg(super::theme::PALETTE_ACCENT)
            .add_modifier(Modifier::BOLD);
        rows.push(RowModel::from_spans(vec![Span::styled(
            format!("── {} ──", name),
            header_style,
        )]));

        for &(idx, filter_entry) in entries {
            let is_selected = idx == state.selected && !is_quick_adding;
            if is_selected {
                rows.push(rows::build_filter_selected_row(
                    app,
                    filter_entry,
                    idx,
                    width,
                ));
            } else {
                rows.push(rows::build_filter_row(app, filter_entry, idx, width));
            }
        }
    }

    if state.entries.is_empty() && !is_quick_adding {
        rows.push(rows::build_message_row(
            "(no matches across journals)",
            Style::default().dim(),
        ));
    }

    ListModel::from_rows(None, rows, app.scroll_offset())
}
