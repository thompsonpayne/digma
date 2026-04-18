use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::ops::{DocumentOp, ReorderPlacement};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId {
    pub actor_id: ActorId,
    pub counter: u64,
}

impl NodeId {
    pub const fn new(actor_id: ActorId, counter: u64) -> Self {
        Self { actor_id, counter }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorId(pub u64);

impl ActorId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct OpId(pub u64);

impl OpId {
    pub const fn new(counter: u64) -> Self {
        Self(counter)
    }
}

#[derive(
    Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct DocumentVersion(pub u64);

impl DocumentVersion {
    pub const fn initial() -> Self {
        Self(0)
    }

    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

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

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct RectNode {
    pub id: NodeId,
    pub pos: Vec2,
    pub size: Vec2,
    pub color: [f32; 4],
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DocumentModel {
    pub version: DocumentVersion,
    pub rects: Vec<RectNode>,
}

impl DocumentModel {
    pub fn new() -> Self {
        Self {
            version: DocumentVersion::initial(),
            rects: vec![],
        }
    }

    /// Check if position collides with the shape objects.
    ///
    /// # Arguments
    /// * `world` - pointer coordinate in world space
    pub fn check_collide_rects(&self, world: Vec2) -> Option<NodeId> {
        for rect in self.rects.iter().rev() {
            let min_x = rect.pos.x;
            let min_y = rect.pos.y;
            let max_x = rect.pos.x + rect.size.x;
            let max_y = rect.pos.y + rect.size.y;
            if world.x >= min_x && world.x <= max_x && world.y >= min_y && world.y <= max_y {
                return Some(rect.id);
            }
        }
        None
    }

    pub fn rect_index(&self, id: NodeId) -> Option<usize> {
        self.rects.iter().position(|rect| rect.id == id)
    }

    pub fn rect(&self, id: NodeId) -> Option<&RectNode> {
        self.rects.iter().find(|rect| rect.id == id)
    }

    pub fn rect_mut(&mut self, id: NodeId) -> Option<&mut RectNode> {
        self.rects.iter_mut().find(|rect| rect.id == id)
    }

    fn reorder_selected(&mut self, node_ids: &[NodeId], to_front: bool) {
        let selected_ids: HashSet<NodeId> = node_ids.iter().copied().collect();
        let mut indices: Vec<usize> = selected_ids
            .iter()
            .filter_map(|id| self.rect_index(*id))
            .collect();

        if to_front {
            indices.sort_unstable_by(|a, b| b.cmp(a));

            for idx in indices {
                if idx + 1 >= self.rects.len() {
                    continue;
                }
                if selected_ids.contains(&self.rects[idx + 1].id) {
                    continue;
                }
                self.rects.swap(idx, idx + 1);
            }
        } else {
            indices.sort_unstable();

            for idx in indices {
                if idx == 0 {
                    continue;
                }
                if selected_ids.contains(&self.rects[idx - 1].id) {
                    continue;
                }
                self.rects.swap(idx, idx - 1);
            }
        }
    }

    pub fn apply_op(&mut self, op: &DocumentOp) -> bool {
        let mutated: bool = match op {
            DocumentOp::CreateRect {
                id,
                pos,
                size,
                color,
            } => {
                if self.rect_index(*id).is_none() {
                    self.rects.push(RectNode {
                        id: *id,
                        pos: *pos,
                        size: *size,
                        color: *color,
                    });
                    true
                } else {
                    false
                }
            }
            DocumentOp::SetRectsGeometry { changes } => {
                let mut mutated = false;
                for change in changes {
                    if let Some(rect) = self.rect_mut(change.id) {
                        if rect.pos != change.after.pos || rect.size != change.after.size {
                            rect.pos = change.after.pos;
                            rect.size = change.after.size;
                            mutated = true;
                        }
                    }
                }

                mutated
            }
            DocumentOp::SetRectsFill { changes } => {
                let mut mutated = false;
                for change in changes {
                    if let Some(rect) = self.rect_mut(change.id) {
                        if rect.color != change.after {
                            rect.color = change.after;
                            mutated = true;
                        }
                    }
                }

                mutated
            }
            DocumentOp::ReorderNodes {
                node_ids,
                placement,
            } => {
                // let to_front = matches!(placement, ReorderPlacement::Forward);
                // self.reorder_selected(node_ids, to_front);
                let before: Vec<NodeId> = self.rects.iter().map(|rect| rect.id).collect();
                let to_front = matches!(placement, ReorderPlacement::Forward);
                self.reorder_selected(node_ids, to_front);
                let after: Vec<NodeId> = self.rects.iter().map(|rect| rect.id).collect();

                before != after
            }
            DocumentOp::DeleteNodes { node_ids } => {
                let ids: HashSet<NodeId> = node_ids.iter().copied().collect();
                let len_before = self.rects.len();
                self.rects.retain(|rect| !ids.contains(&rect.id));

                len_before != self.rects.len()
            }
            DocumentOp::RestoreNodes { nodes } => {
                let mut restored = nodes.clone();
                let mut mutated = false;
                restored.sort_by_key(|(_, original_index)| *original_index);

                for (rect, original_index) in restored {
                    if self.rect_index(rect.id).is_none() {
                        let insert_at = original_index.min(self.rects.len());
                        self.rects.insert(insert_at, rect);
                        mutated = true;
                    }
                }

                mutated
            }
        };

        if mutated {
            self.version = self.version.next();
        }

        mutated
    }
}

pub type Document = DocumentModel;
