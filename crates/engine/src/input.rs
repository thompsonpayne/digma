use serde::{Deserialize, Serialize};

use crate::camera::Camera;
use crate::render_scene::{OverlayScene, RenderScene};
use crate::types::Vec2;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InputBatch {
    pub events: Vec<InputEvent>,
    pub tool: ToolMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputEvent {
    CameraPanByScreenDelta {
        delta_px: Vec2,
    },
    CameraZoomAtScreenPoint {
        pivot_px: Vec2,
        zoom_multiplier: f32,
    },
    PointerDown {
        screen_px: Vec2,
        shift: bool,
        button: u8,
    },
    PointerMove {
        screen_px: Vec2,
        buttons: u16,
    },
    PointerUp {
        screen_px: Vec2,
        button: u8,
    },
    PointerCancel,
    SetSelectionFill {
        color: RgbaColor,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RgbaColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorStyle {
    Default,
    ResizeTlBr, // TL and BR corners — ↖↘
    ResizeTrBl, // TR and BL corners — ↗↙
    Move,
    Crosshair,
    Pan,
    Panning,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EngineOutput {
    pub camera: Camera,
    pub render_scene: RenderScene,
    pub overlay_scene: OverlayScene,
    pub cursor: CursorStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolMode {
    #[default]
    Select,
    Rect,
}
