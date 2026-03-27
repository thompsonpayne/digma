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

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct RectGeometryChange {
    pub id: NodeId,
    pub before: RectGeometry,
    pub after: RectGeometry,
}

#[derive(Debug, Clone)]
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

    BringForward(Vec<NodeId>),
    SendBackward(Vec<NodeId>),

    Delete {
        rects: Vec<(RectNode, usize)>,   // (rect, original_index) pairs
        previous_selection: Vec<NodeId>, // what self.selected was before applying delete
        next_selection: Vec<NodeId>,     // what self.selected should be after applying delete
    },
}
