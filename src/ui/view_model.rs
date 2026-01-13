use crate::app::{App, InputMode};

use super::calendar::CalendarModel;
use super::container::ContainerConfig;
use super::context::RenderContext;
use super::header::HeaderModel;
use super::layout::{LayoutNode, PanelId};
use super::model::ListModel;
use super::overlay::{CommandPaletteModel, ConfirmModel, OverlayModel};
use super::prep::RenderPrep;
use super::scroll::CursorContext;
use super::views::build_view_spec;

pub struct ViewModel {
    pub layout: LayoutNode,
    pub panels: PanelRegistry,
    pub overlays: OverlayModel,
    pub cursor: CursorModel,
    pub header: HeaderModel,
    pub focused_panel: Option<PanelId>,
}

pub struct PanelModel {
    pub id: PanelId,
    pub config: ContainerConfig,
    pub content: PanelContent,
}

impl PanelModel {
    #[must_use]
    pub fn new(id: PanelId, config: ContainerConfig, content: PanelContent) -> Self {
        Self {
            id,
            config,
            content,
        }
    }
}

pub enum PanelContent {
    EntryList(ListModel),
    Calendar(CalendarModel),
    Empty,
}

pub struct PanelRegistry {
    panels: Vec<PanelModel>,
}

impl PanelRegistry {
    #[must_use]
    pub fn new(panels: Vec<PanelModel>) -> Self {
        Self { panels }
    }

    pub fn get(&self, id: PanelId) -> Option<&PanelModel> {
        self.panels.get(id.0)
    }
}

pub struct CursorModel {
    pub edit: Option<CursorContext>,
}

pub fn build_view_model(app: &App, context: &RenderContext, prep: RenderPrep) -> ViewModel {
    let overlays = OverlayModel {
        confirm: match &app.input_mode {
            InputMode::Confirm(confirm_context) => Some(ConfirmModel::new(confirm_context.clone())),
            _ => None,
        },
        command_palette: match &app.input_mode {
            InputMode::CommandPalette(state) => Some(CommandPaletteModel::new(state)),
            _ => None,
        },
    };

    let view_spec = build_view_spec(app, context);

    ViewModel {
        layout: view_spec.layout,
        panels: PanelRegistry::new(view_spec.panels),
        overlays,
        cursor: CursorModel {
            edit: prep.edit_cursor,
        },
        header: view_spec.header,
        focused_panel: view_spec.focused_panel,
    }
}
