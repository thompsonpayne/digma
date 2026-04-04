use crate::{NodeId, RectGeometryChange, RectNode, Vec2, history::RectFillChange};

#[derive(Debug, Clone)]
pub enum DocumentOp {
    CreateRect {
        id: NodeId,
        pos: Vec2,
        size: Vec2,
        color: [f32; 4],
    },

    SetRectsGeometry {
        changes: Vec<RectGeometryChange>,
    },
    SetRectsFill {
        changes: Vec<RectFillChange>,
    },
    ReorderNodes {
        node_ids: Vec<NodeId>,
        placement: ReorderPlacement,
    },
    DeleteNodes {
        node_ids: Vec<NodeId>,
    },
    RestoreNodes {
        nodes: Vec<(RectNode, usize)>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReorderPlacement {
    Forward,
    Backward,
}
