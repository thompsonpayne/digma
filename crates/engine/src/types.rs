use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RectNode {
    pub id: NodeId,
    pub pos: Vec2,
    pub size: Vec2,
    pub color: [f32; 4],
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Document {
    pub next_id: u64,
    pub rects: Vec<RectNode>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            rects: vec![],
        }
    }

    pub fn alloc_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        NodeId(id)
    }
}
