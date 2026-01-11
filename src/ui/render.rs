use ratatui::Frame;

use crate::app::App;

use super::container::{render_container, render_list};
use super::context::RenderContext;
use super::overlay::{
    render_confirm_modal, render_footer_bar, render_help_modal, render_hint_overlay,
    render_interface_modal, render_journal_indicator, render_status_banner,
};
use super::prep::prepare_render;
use super::scroll::set_edit_cursor;
use super::view_model::build_view_model;

pub fn render_app(f: &mut Frame<'_>, app: &mut App) {
    let context = RenderContext::new(f.area());
    let prep = prepare_render(app, &context);

    if let Some(cursor) = prep.edit_cursor.as_ref() {
        set_edit_cursor(
            f,
            cursor,
            app.scroll_offset_mut(),
            context.scroll_height,
            context.content_area,
        );
    }

    let view_model = build_view_model(app, &context, prep);

    let container_layout = render_container(f, &context, &view_model.container.config);

    render_list(f, view_model.container.list, &container_layout);

    render_status_banner(f, view_model.overlays.status, container_layout.content_area);

    render_footer_bar(f, view_model.overlays.footer, context.footer_area);

    render_hint_overlay(f, view_model.overlays.hint, context.footer_area);

    render_journal_indicator(f, view_model.overlays.journal, context.footer_area);

    if let Some((cursor_x, cursor_y)) = view_model.cursor.prompt {
        f.set_cursor_position((cursor_x, cursor_y));
    }

    if let Some(help) = view_model.overlays.help {
        render_help_modal(f, help, context.help_popup_area);
    }

    if let Some(confirm) = view_model.overlays.confirm {
        render_confirm_modal(f, confirm, context.size);
    }

    if let Some(interface) = view_model.overlays.interface {
        render_interface_modal(f, interface, context.size);
    }
}
