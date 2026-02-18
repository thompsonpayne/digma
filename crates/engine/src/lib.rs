use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Document {
    pub next_id: u64,
}

impl Document {
    pub fn new() -> Self {
        Self { next_id: 1 }
    }

    pub fn alloc_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        NodeId(id)
    }
}
