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

#[derive(Debug, Clone, Default)]
pub struct PendingSelectionMove {
    pub start_screen_px: Vec2,
    pub start_world: Vec2,
    pub previous_selection: Vec<NodeId>,
}

#[derive(Debug, Clone, Copy)]
pub struct PendingResize {
    pub handle: HandleHit,
    pub start_screen_px: Vec2,
    pub start_world: Vec2,
}

#[derive(Debug, Clone, Default)]
pub struct PendingRectCreate {
    pub start_screen_px: Vec2,
    pub start_world: Vec2,
    pub previous_selection: Vec<NodeId>,
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
    pub origins: Vec<(NodeId, Vec2)>,
}

#[derive(Debug, Clone)]
pub struct RectCreateDrag {
    pub start_world: Vec2,
    pub current_world: Vec2,
    pub previous_selection: Vec<NodeId>,
}

#[derive(Debug, Clone)]
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

    PendingRectCreate(PendingRectCreate),
    RectCreate(RectCreateDrag),
}

pub fn compute_resize(
    corner: Corner,
    dx: f32,
    dy: f32,
    origin_pos: Vec2,
    origin_size: Vec2,
    min_size: f32,
) -> (Vec2, Vec2) {
    let mut pos = origin_pos;
    let mut size = origin_size;

    // Helper: clamp one axis. Returns (new_origin, new_size).
    // If the dragged edge would pass the opposite edge, pin at min_size.
    let clamp_axis = |origin: f32, length: f32, delta: f32, anchor_end: bool| -> (f32, f32) {
        if anchor_end {
            // Right/bottom edge moves — origin stays, size changes
            let new_len = (length + delta).max(min_size);
            (origin, new_len)
        } else {
            // Left/top edge moves — origin shifts, size shrinks
            let mut new_origin = origin + delta;
            let mut new_len = length - delta;
            if new_len < min_size {
                new_len = min_size;
                new_origin = origin + length - min_size;
            }
            (new_origin, new_len)
        }
    };

    let anchor_left = matches!(corner, Corner::TR | Corner::BR);
    let anchor_top = matches!(corner, Corner::BL | Corner::BR);

    let (px, sx) = clamp_axis(origin_pos.x, origin_size.x, dx, anchor_left);
    let (py, sy) = clamp_axis(origin_pos.y, origin_size.y, dy, anchor_top);
    pos.x = px;
    pos.y = py;
    size.x = sx;
    size.y = sy;

    (pos, size)
}
