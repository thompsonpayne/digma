use crate::{NodeId, RectGeometryChange, Vec2, history::RectFillChange};

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
}

pub enum ReorderPlacement {
    Forward,
    Backward,
}
