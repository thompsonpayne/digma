use serde::{Deserialize, Serialize};

use crate::{NodeId, RectNode, Vec2, ops::DocumentOp};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RectFillChange {
    pub id: NodeId,
    pub before: [f32; 4],
    pub after: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub forward: DocumentOp,
    pub inverse: DocumentOp,
}

#[derive(Debug, Clone)]
pub struct HistoryGroup {
    pub entries: Vec<HistoryEntry>,
    pub selection_before: Vec<NodeId>,
    pub selection_after: Vec<NodeId>,
}
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct RectGeometryChange {
    pub id: NodeId,
    pub before: RectGeometry,
    pub after: RectGeometry,
}
