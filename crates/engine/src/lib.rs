mod render_scene;

use serde::{Deserialize, Serialize};

pub use crate::render_scene::{OverlayScene, RectInstance, RenderScene};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct InputBatch {
    pub events: Vec<InputEvent>,
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
    },
}

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

#[derive(Debug, Default, Clone)]
pub struct Engine {
    pub doc: Document,
    pub camera: Camera,
    pub selected: Vec<NodeId>,
}

impl Engine {
    pub fn new() -> Self {
        let mut doc = Document::new();

        let rects = vec![
            RectNode {
                id: doc.alloc_id(),
                pos: Vec2::new(100.0, 100.0),
                size: Vec2::new(120.0, 80.0),
                color: [0.2, 0.7, 0.9, 1.0],
            },
            RectNode {
                id: doc.alloc_id(),
                pos: Vec2::new(300.0, 220.0),
                size: Vec2::new(140.0, 80.0),
                color: [0.9, 0.3, 0.9, 1.0],
            },
            RectNode {
                id: doc.alloc_id(),
                pos: Vec2::new(600.0, 900.0),
                size: Vec2::new(200.0, 100.0),
                color: [0.5, 0.8, 0.4, 1.0],
            },
        ];

        doc.rects = rects;

        Self {
            doc,
            camera: Camera::default(),
            selected: vec![],
        }
    }

    fn check_collide_rects(&self, world: Vec2) -> Option<NodeId> {
        for rect in self.doc.rects.iter().rev() {
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

    fn apply_selection(&mut self, hit: Option<NodeId>, shift: bool) {
        match (hit, shift) {
            (Some(id), false) => {
                self.selected.clear();
                self.selected.push(id);
            }
            (Some(id), true) => {
                if let Some(idx) = self.selected.iter().position(|&v| v == id) {
                    self.selected.swap_remove(idx);
                } else {
                    self.selected.push(id);
                }
            }
            (None, false) => {
                self.selected.clear();
            }
            (None, true) => {}
        }
    }

    pub fn tick(&mut self, batch: &InputBatch) -> EngineOutput {
        let render_scene = render_scene::RenderScene {
            rects: self
                .doc
                .rects
                .iter()
                .map(|r| RectInstance {
                    pos: [r.pos.x, r.pos.y],
                    size: [r.size.x, r.size.y],
                    color: r.color,
                })
                .collect(),
        };

        for ev in &batch.events {
            match *ev {
                InputEvent::CameraPanByScreenDelta { delta_px } => {
                    self.camera.pan_by_screen_delta(delta_px);
                }
                InputEvent::CameraZoomAtScreenPoint {
                    pivot_px,
                    zoom_multiplier,
                } => {
                    self.camera.zoom_at_screen_point(pivot_px, zoom_multiplier);
                }
                InputEvent::PointerDown { screen_px, shift } => {
                    let world = self.camera.screen_to_world(screen_px);
                    let hit = self.check_collide_rects(world);
                    self.apply_selection(hit, shift);
                }
            }
        }

        let overlay_scene = self.init_overlay_scene();

        EngineOutput {
            camera: self.camera,
            render_scene,
            overlay_scene,
        }
    }

    fn init_overlay_scene(&self) -> OverlayScene {
        let outline_px = 2.0;
        let handle_px = 8.0;
        let outline = outline_px / self.camera.zoom;
        let handle = handle_px / self.camera.zoom;
        let outline_color = [0.95, 0.95, 0.95, 1.0];
        let handle_color = [0.1, 0.6, 1.0, 1.0];
        let mut overlay_rects = Vec::new();
        for id in &self.selected {
            let Some(rect) = self.doc.rects.iter().find(|r| r.id == *id) else {
                continue;
            };
            let x = rect.pos.x;
            let y = rect.pos.y;
            let w = rect.size.x;
            let h = rect.size.y;
            // outline
            overlay_rects.push(RectInstance {
                pos: [x, y],
                size: [w, outline],
                color: outline_color,
            });
            overlay_rects.push(RectInstance {
                pos: [x, y + h - outline],
                size: [w, outline],
                color: outline_color,
            });
            overlay_rects.push(RectInstance {
                pos: [x, y],
                size: [outline, h],
                color: outline_color,
            });
            overlay_rects.push(RectInstance {
                pos: [x + w - outline, y],
                size: [outline, h],
                color: outline_color,
            });
            // handles
            overlay_rects.push(RectInstance {
                pos: [x - handle * 0.5, y - handle * 0.5],
                size: [handle, handle],
                color: handle_color,
            });
            overlay_rects.push(RectInstance {
                pos: [x + w - handle * 0.5, y - handle * 0.5],
                size: [handle, handle],
                color: handle_color,
            });
            overlay_rects.push(RectInstance {
                pos: [x - handle * 0.5, y + h - handle * 0.5],
                size: [handle, handle],
                color: handle_color,
            });
            overlay_rects.push(RectInstance {
                pos: [x + w - handle * 0.5, y + h - handle * 0.5],
                size: [handle, handle],
                color: handle_color,
            });
        }
        render_scene::OverlayScene {
            rects: overlay_rects,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EngineOutput {
    pub camera: Camera,
    pub render_scene: RenderScene,
    pub overlay_scene: OverlayScene,
}

#[cfg(test)]
mod test {
    use super::*;
    fn assert_approx(a: f32, b: f32, eps: f32) {
        if (a - b).abs() > eps {
            panic!("Expected {a} ~= {b} (eps {eps})");
        }
    }

    fn assert_vec2_approx(a: Vec2, b: Vec2, eps: f32) {
        assert_approx(a.x, b.x, eps);
        assert_approx(a.y, b.y, eps);
    }

    #[test]
    fn world_screen_roundtrip_is_stable() {
        let cam = Camera {
            pan: Vec2::new(100.0, -50.0),
            zoom: 2.5,
        };

        let world = Vec2::new(12.0, 34.0);
        let screen = cam.world_to_screen(world);
        let world2 = cam.screen_to_world(screen);

        assert_vec2_approx(world2, world, 1e-4);
    }

    #[test]
    fn pan_by_screen_delta_moves_world_origin_expected_direction() {
        let mut cam = Camera {
            pan: Vec2::new(0.0, 0.0),
            zoom: 2.0,
        };

        // Drag pointer right/down by 20px/10px => camera should pan left/up in world units.
        cam.pan_by_screen_delta(Vec2::new(20.0, 10.0));
        assert_vec2_approx(cam.pan, Vec2::new(-10.0, -5.0), 1e-6);
    }

    #[test]
    fn zoom_at_cursor_keeps_world_point_under_cursor_fixed() {
        let mut cam = Camera {
            pan: Vec2::new(10.0, 20.0),
            zoom: 2.0,
        };

        let pivot_screen = Vec2::new(300.0, 120.0);
        let world_before = cam.screen_to_world(pivot_screen);

        cam.zoom_at_screen_point(pivot_screen, 1.5);

        let world_after = cam.screen_to_world(pivot_screen);
        assert_vec2_approx(world_after, world_before, 1e-4);
    }

    #[test]
    fn tick_applies_pan_event() {
        let mut engine = Engine {
            doc: Document::new(),
            camera: Camera {
                pan: Vec2::new(0.0, 0.0),
                zoom: 2.0,
            },
            selected: vec![],
        };

        let batch = InputBatch {
            events: vec![InputEvent::CameraPanByScreenDelta {
                delta_px: Vec2::new(20.0, 10.0),
            }],
        };

        engine.tick(&batch);

        // same expections as the direct Camera test
        assert_vec2_approx(engine.camera.pan, Vec2::new(-10.0, -5.0), 1e-6);
        assert_approx(engine.camera.zoom, 2.0, 1e-6);
    }

    #[test]
    fn tick_applies_zoom_event_and_preserves_world_point_under_cursor() {
        let mut engine = Engine {
            doc: Document::new(),
            camera: Camera {
                pan: Vec2::new(10.0, 20.0),
                zoom: 2.0,
            },
            selected: vec![],
        };

        let pivot = Vec2::new(300.0, 120.0);
        let world_before = engine.camera.screen_to_world(pivot);

        let batch = InputBatch {
            events: vec![InputEvent::CameraZoomAtScreenPoint {
                pivot_px: pivot,
                zoom_multiplier: 1.5,
            }],
        };

        engine.tick(&batch);

        assert_approx(engine.camera.zoom, 3.0, 1e-6);
        let world_after = engine.camera.screen_to_world(pivot);
        assert_vec2_approx(world_after, world_before, 1e-4);
    }

    #[test]
    fn tick_applies_events_in_order() {
        let mut engine = Engine::new();
        engine.camera.pan = Vec2::new(5.0, -7.0);
        engine.camera.zoom = 2.0;

        let batch = InputBatch {
            events: vec![
                InputEvent::CameraPanByScreenDelta {
                    delta_px: Vec2::new(40.0, 0.0),
                },
                InputEvent::CameraZoomAtScreenPoint {
                    pivot_px: Vec2::new(100.0, 50.0),
                    zoom_multiplier: 0.5,
                },
            ],
        };

        // expected result: applying the same ops directly, in the same order
        let mut expected = engine.camera;
        expected.pan_by_screen_delta(Vec2::new(40.0, 0.0));
        expected.zoom_at_screen_point(Vec2::new(100.0, 50.0), 0.5);

        engine.tick(&batch);

        assert_vec2_approx(engine.camera.pan, expected.pan, 1e-5);
        assert_approx(engine.camera.zoom, expected.zoom, 1e-6);
    }

    #[test]
    fn hit_test_picks_topmost_rect() {
        let engine = Engine::new();
        let top_id = engine.doc.rects[2].id;
        let hit = engine.check_collide_rects(Vec2::new(610.0, 910.0));
        assert_eq!(hit, Some(top_id));
    }

    #[test]
    fn selection_rules_apply_correcly() {
        let mut engine = Engine::new();
        let id = engine.doc.rects[0].id;

        engine.apply_selection(Some(id), false);
        assert_eq!(engine.selected, vec![id]);

        engine.apply_selection(Some(id), true);
        assert!(engine.selected.is_empty());

        engine.apply_selection(None, false);
        assert!(engine.selected.is_empty());
    }
}
