use serde::{Deserialize, Serialize};

/// RenderScene | contains core shapes, objects
#[derive(Debug, Serialize, Deserialize)]
pub struct RenderScene {
    pub rects: Vec<RectInstance>,
}

/// OverlayScene | contains UI editor elements: selection, highlight
#[derive(Debug, Serialize, Deserialize)]
pub struct OverlayScene {
    pub rects: Vec<RectInstance>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct RectInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}
