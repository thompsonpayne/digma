use serde::{Deserialize, Serialize};

use crate::{
    history::RectFillChange, ActorId, DocumentVersion, NodeId, OpId, RectGeometryChange, RectNode,
    Vec2,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReorderPlacement {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpEnvelope {
    pub op_id: OpId,
    pub actor_id: ActorId,
    pub base_version: DocumentVersion,
    pub op: DocumentOp,
}
