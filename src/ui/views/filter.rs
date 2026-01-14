use ratatui::style::Color;

use crate::app::App;
use crate::ui::container::view_content_container_config;
use crate::ui::context::RenderContext;
use crate::ui::filter::build_filter_list;
use crate::ui::layout::PanelId;
use crate::ui::view_model::{PanelContent, PanelModel};

use super::{ViewSpec, list_panel_content_area};

pub fn build_filter_view_spec(app: &App, context: &RenderContext) -> ViewSpec {
    let config = view_content_container_config(Color::Magenta);
    let list = build_filter_list(app, list_content_width_for_filter(context));

    let panel_id = PanelId(0);
    let panel = PanelModel::new(panel_id, config, PanelContent::EntryList(list));

    ViewSpec::single_panel(panel)
}

pub(crate) fn list_content_width_for_filter(context: &RenderContext) -> usize {
    list_panel_content_area(context, Color::Magenta).width as usize
}

pub(crate) fn list_content_height_for_filter(context: &RenderContext) -> usize {
    list_panel_content_area(context, Color::Magenta).height as usize
}
