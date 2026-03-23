use crate::{NodeId, RectNode, Vec2};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectGeometry {
    pub pos: Vec2,
    pub size: Vec2,
}

impl RectGeometry {
    pub fn from_rect(rect: &RectNode) -> Self {
        Self {
            pos: rect.pos,
            size: rect.size,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RectGeometryChange {
    pub id: NodeId,
    pub before: RectGeometry,
    pub after: RectGeometry,
}

#[derive(Debug)]
pub enum ToolCommand {
    CreateRect {
        rect: RectNode,
        previous_selection: Vec<NodeId>,
        next_selection: Vec<NodeId>,
    },

    // move and resize
    SetRectsGeometry {
        changes: Vec<RectGeometryChange>,
    },

    BringForward(NodeId),
    SendBackward(NodeId),

    Delete {
        rect: RectNode,
        original_index: usize,
    },
}
