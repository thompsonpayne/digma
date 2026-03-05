use crate::types::{NodeId, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TL,
    TR,
    BL,
    BR,
}

#[derive(Debug, Clone, Copy)]
pub struct HandleHit {
    pub node_id: NodeId,
    pub corner: Corner,
}

#[derive(Debug, Clone, Copy)]
pub struct PendingMarquee {
    pub start_screen_px: Vec2,
    pub start_world: Vec2,
    pub additive: bool,
}

#[derive(Debug, Clone)]
pub struct MarqueeDrag {
    pub start_world: Vec2,
    pub current_world: Vec2,
    pub additive: bool, // shift key active
}

#[derive(Debug, Clone, Default)]
pub struct SelectionDrag {
    pub start_world: Vec2,
    pub current_world: Vec2,

    // Snapshot selected rect index + original position to avoid cumulative drift
    pub origins: Vec<(usize, Vec2)>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PendingSelectionMove {
    pub start_screen_px: Vec2,
    pub start_world: Vec2,
}

#[derive(Debug, Clone, Copy)]
pub struct PendingResize {
    pub handle: HandleHit,
    pub start_screen_px: Vec2,
    pub start_world: Vec2,
}

#[derive(Debug)]
pub struct ResizeDrag {
    pub handle: HandleHit,
    pub start_world: Vec2,
    pub current_world: Vec2,

    // origin snapshot - pos and size at drag start (no drift)
    pub origin_pos: Vec2,
    pub origin_size: Vec2,
    pub rect_idx: usize,
}

#[derive(Debug)]
pub enum DragState {
    Idle,

    PendingMarquee(PendingMarquee),
    Marquee(MarqueeDrag),

    PendingSelectionMove(PendingSelectionMove),
    SelectionMove(SelectionDrag),

    PendingResize(PendingResize),
    Resize(ResizeDrag),
}
