use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub enum LayoutNode {
    Row {
        children: Vec<LayoutNode>,
        ratios: Vec<u16>,
    },
    Column {
        children: Vec<LayoutNode>,
        ratios: Vec<u16>,
    },
    Panel {
        id: PanelId,
    },
}

impl LayoutNode {
    #[must_use]
    pub fn row(children: Vec<LayoutNode>, ratios: Vec<u16>) -> Self {
        Self::Row { children, ratios }
    }

    #[must_use]
    pub fn column(children: Vec<LayoutNode>, ratios: Vec<u16>) -> Self {
        Self::Column { children, ratios }
    }

    #[must_use]
    pub fn panel(id: PanelId) -> Self {
        Self::Panel { id }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PanelId(pub usize);

pub fn layout_nodes(area: Rect, node: &LayoutNode) -> Vec<(PanelId, Rect)> {
    match node {
        LayoutNode::Panel { id } => vec![(*id, area)],
        LayoutNode::Row { children, ratios } => {
            split_children(area, Direction::Horizontal, children, ratios)
        }
        LayoutNode::Column { children, ratios } => {
            split_children(area, Direction::Vertical, children, ratios)
        }
    }
}

pub fn padded_content_area_with_buffer(inner: Rect, bottom_buffer: u16) -> Rect {
    let horizontally_padded = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(inner)[1];

    Rect {
        height: horizontally_padded.height.saturating_sub(bottom_buffer),
        ..horizontally_padded
    }
}

fn split_children(
    area: Rect,
    direction: Direction,
    children: &[LayoutNode],
    ratios: &[u16],
) -> Vec<(PanelId, Rect)> {
    let constraints: Vec<Constraint> = if ratios.is_empty() || ratios.len() != children.len() {
        vec![Constraint::Ratio(1, children.len() as u32); children.len()]
    } else {
        ratios
            .iter()
            .map(|ratio| Constraint::Percentage(*ratio))
            .collect()
    };

    let chunks = Layout::default()
        .direction(direction)
        .constraints(constraints)
        .split(area);

    children
        .iter()
        .zip(chunks.iter().copied())
        .flat_map(|(child, rect)| layout_nodes(rect, child))
        .collect()
}
