use serde::{Deserialize, Serialize};

use crate::types::Vec2;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub pan: Vec2,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera {
            pan: Vec2::new(0.0, 0.0),
            zoom: 1.0,
        }
    }
}

impl Camera {
    /// Convert screen coordinate to world coordinate.
    ///
    /// # Arguments
    /// * `screen_px` - coordinate to convert
    pub fn screen_to_world(&self, screen_px: Vec2) -> Vec2 {
        Vec2::new(
            self.pan.x + screen_px.x / self.zoom,
            self.pan.y + screen_px.y / self.zoom,
        )
    }

    pub fn world_to_screen(&self, world: Vec2) -> Vec2 {
        Vec2::new(
            (world.x - self.pan.x) * self.zoom,
            (world.y - self.pan.y) * self.zoom,
        )
    }

    pub fn pan_by_screen_delta(&mut self, delta_px: Vec2) {
        self.pan.x -= delta_px.x / self.zoom;
        self.pan.y -= delta_px.y / self.zoom;
    }

    pub fn zoom_at_screen_point(&mut self, pivot_px: Vec2, zoom_multiplier: f32) {
        let old_zoom = self.zoom;
        let new_zoom = (self.zoom * zoom_multiplier).clamp(0.05, 64.0);

        if (new_zoom - old_zoom).abs() < f32::EPSILON {
            return;
        }

        let world_under_cursor = Vec2::new(
            self.pan.x + pivot_px.x / old_zoom,
            self.pan.y + pivot_px.y / old_zoom,
        );

        self.zoom = new_zoom;
        self.pan.x = world_under_cursor.x - pivot_px.x / new_zoom;
        self.pan.y = world_under_cursor.y - pivot_px.y / new_zoom;
    }
}
