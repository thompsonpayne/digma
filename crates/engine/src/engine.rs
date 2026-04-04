use std::collections::HashSet;

use crate::drag::DragState;
use crate::history::{HistoryEntry, HistoryGroup, RectFillChange};
use crate::input::{EngineOutput, InputBatch, InputEvent};
use crate::ops::{DocumentOp, ReorderPlacement};
use crate::render_scene::{RectInstance, RenderScene};
use crate::types::{DocumentModel, NodeId, RectNode, Vec2};
use crate::{EditorSession, RectGeometry, RectGeometryChange};

pub struct Engine {
    pub document: DocumentModel,
    pub session: EditorSession,

    undo_stack: Vec<HistoryGroup>,
    redo_stack: Vec<HistoryGroup>,
}

fn invert_geometry_changes(changes: &[RectGeometryChange]) -> Vec<RectGeometryChange> {
    changes
        .iter()
        .map(|change| RectGeometryChange {
            id: change.id,
            before: change.after,
            after: change.before,
        })
        .collect()
}

fn invert_fill_changes(changes: &[RectFillChange]) -> Vec<RectFillChange> {
    changes
        .iter()
        .map(|change| RectFillChange {
            id: change.id,
            before: change.after,
            after: change.before,
        })
        .collect()
}

fn single_entry_group(
    forward: DocumentOp,
    inverse: DocumentOp,
    selection_before: Vec<NodeId>,
    selection_after: Vec<NodeId>,
) -> HistoryGroup {
    HistoryGroup {
        entries: vec![HistoryEntry { forward, inverse }],
        selection_before,
        selection_after,
    }
}

impl Engine {
    pub fn new() -> Self {
        let mut doc = DocumentModel::new();

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
            document: doc,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            session: EditorSession::default(),
        }
    }

    /// Process a batch of input events and return the new engine output.
    ///
    /// # Arguments
    /// * `batch` - list of input events to process
    pub fn tick(&mut self, batch: &InputBatch) -> EngineOutput {
        let drag_threshold_px: f32 = 6.0;
        let drag_threshold_sq: f32 = drag_threshold_px * drag_threshold_px;

        for ev in &batch.events {
            match *ev {
                InputEvent::CameraPanByScreenDelta { delta_px } => {
                    self.session.camera.pan_by_screen_delta(delta_px);
                }
                InputEvent::CameraZoomAtScreenPoint {
                    pivot_px,
                    zoom_multiplier,
                } => {
                    self.session
                        .camera
                        .zoom_at_screen_point(pivot_px, zoom_multiplier);
                }
                InputEvent::PointerDown {
                    screen_px,
                    shift,
                    button: _,
                } => {
                    self.session
                        .pointer_down(&self.document, batch.tool, screen_px, shift);
                }
                InputEvent::PointerMove {
                    screen_px,
                    buttons: _buttons,
                } => {
                    self.session
                        .pointer_move(&mut self.document, screen_px, drag_threshold_sq);
                }
                InputEvent::PointerUp {
                    screen_px,
                    button: _button,
                } => {
                    let world = self.session.camera.screen_to_world(screen_px);

                    if matches!(self.session.drag_state, DragState::Marquee(_)) {
                        self.session.update_marquee_drag(
                            screen_px,
                            world,
                            drag_threshold_sq,
                            &self.document.rects,
                        );
                    }

                    let drag_state =
                        std::mem::replace(&mut self.session.drag_state, DragState::Idle);

                    let history_group = match drag_state {
                        DragState::SelectionMove(drag) => {
                            let changes: Vec<RectGeometryChange> = drag
                                .origins
                                .into_iter()
                                .filter_map(|(id, origin_pos)| {
                                    let before = RectGeometry {
                                        pos: origin_pos,
                                        size: self.document.rect(id)?.size,
                                    };
                                    self.geometry_change_for_rect(id, before)
                                })
                                .collect();

                            if changes.is_empty() {
                                None
                            } else {
                                Some((
                                    single_entry_group(
                                        DocumentOp::SetRectsGeometry {
                                            changes: changes.clone(),
                                        },
                                        DocumentOp::SetRectsGeometry {
                                            changes: invert_geometry_changes(&changes),
                                        },
                                        self.session.selected.clone(),
                                        self.session.selected.clone(),
                                    ),
                                    false,
                                ))
                            }
                        }
                        DragState::Resize(drag) => {
                            let change = self.geometry_change_for_rect(
                                drag.handle.node_id,
                                RectGeometry {
                                    pos: drag.origin_pos,
                                    size: drag.origin_size,
                                },
                            );

                            change.map(|change| {
                                let changes = vec![change];
                                (
                                    single_entry_group(
                                        DocumentOp::SetRectsGeometry {
                                            changes: changes.clone(),
                                        },
                                        DocumentOp::SetRectsGeometry {
                                            changes: invert_geometry_changes(&changes),
                                        },
                                        self.session.selected.clone(),
                                        self.session.selected.clone(),
                                    ),
                                    false,
                                )
                            })
                        }
                        DragState::RectCreate(drag) => {
                            let min_size = 1.0f32;

                            let min_x = drag.start_world.x.min(drag.current_world.x);
                            let min_y = drag.start_world.y.min(drag.current_world.y);
                            let raw_w = (drag.start_world.x - drag.current_world.x).abs();
                            let raw_h = (drag.start_world.y - drag.current_world.y).abs();

                            let w = raw_w.max(min_size);
                            let h = raw_h.max(min_size);

                            let rect = RectNode {
                                id: self.document.alloc_id(),
                                pos: Vec2::new(min_x, min_y),
                                size: Vec2::new(w, h),
                                color: [0.769, 0.769, 0.769, 1.0],
                            };

                            Some((
                                single_entry_group(
                                    DocumentOp::CreateRect {
                                        id: rect.id,
                                        pos: rect.pos,
                                        size: rect.size,
                                        color: rect.color,
                                    },
                                    DocumentOp::DeleteNodes {
                                        node_ids: vec![rect.id],
                                    },
                                    drag.previous_selection,
                                    vec![rect.id],
                                ),
                                false,
                            ))
                        }
                        _ => None,
                    };

                    if let Some((group, apply_now)) = history_group {
                        if apply_now {
                            self.apply_history_group(&group, true)
                        }
                        self.push_history_group(group)
                    }
                }
                InputEvent::PointerCancel => {
                    self.session.pointer_cancel(&mut self.document);
                }
                InputEvent::SetSelectionFill { color } => {
                    let selected: HashSet<NodeId> = self.session.selected.iter().copied().collect();
                    //
                    // for rect in &mut self.document.rects {
                    //     if selected.contains(&rect.id) {
                    //         rect.color = [color.r, color.g, color.b, color.a];
                    //     }
                    // }
                    let changes: Vec<RectFillChange> = self
                        .document
                        .rects
                        .iter()
                        .filter(|r| selected.contains(&r.id))
                        .map(|r| RectFillChange {
                            id: r.id,
                            before: r.color,
                            after: [color.r, color.g, color.b, color.a],
                        })
                        .collect();

                    if !changes.is_empty() {
                        let group = single_entry_group(
                            DocumentOp::SetRectsFill {
                                changes: changes.clone(),
                            },
                            DocumentOp::SetRectsFill {
                                changes: invert_fill_changes(&changes),
                            },
                            self.session.selected.clone(),
                            self.session.selected.clone(),
                        );

                        self.apply_history_group(&group, true);
                        self.push_history_group(group)
                    }
                }
                InputEvent::Undo => {
                    self.undo();
                }
                InputEvent::Redo => {
                    self.redo();
                }
                InputEvent::BringForward => {
                    if self.session.selected.is_empty() {
                        continue;
                    }

                    let group = single_entry_group(
                        DocumentOp::ReorderNodes {
                            node_ids: self.session.selected.clone(),
                            placement: ReorderPlacement::Forward,
                        },
                        DocumentOp::ReorderNodes {
                            node_ids: self.session.selected.clone(),
                            placement: ReorderPlacement::Backward,
                        },
                        self.session.selected.clone(),
                        self.session.selected.clone(),
                    );
                    self.apply_history_group(&group, true);
                    self.push_history_group(group);
                }
                InputEvent::SendBackward => {
                    if self.session.selected.is_empty() {
                        continue;
                    }

                    let group = single_entry_group(
                        DocumentOp::ReorderNodes {
                            node_ids: self.session.selected.clone(),
                            placement: ReorderPlacement::Backward,
                        },
                        DocumentOp::ReorderNodes {
                            node_ids: self.session.selected.clone(),
                            placement: ReorderPlacement::Forward,
                        },
                        self.session.selected.clone(),
                        self.session.selected.clone(),
                    );

                    self.apply_history_group(&group, true);
                    self.push_history_group(group);
                }
                InputEvent::DeleteSelected => {
                    let selected_ids: HashSet<NodeId> =
                        self.session.selected.iter().copied().collect();
                    let rects: Vec<(RectNode, usize)> = self
                        .document
                        .rects
                        .iter()
                        .enumerate()
                        .filter_map(|(idx, rect)| {
                            selected_ids.contains(&rect.id).then_some((*rect, idx))
                        })
                        .collect();

                    if rects.is_empty() {
                        continue;
                    }

                    let node_ids: Vec<NodeId> = rects.iter().map(|(rect, _)| rect.id).collect();

                    let group = single_entry_group(
                        DocumentOp::DeleteNodes { node_ids },
                        DocumentOp::RestoreNodes {
                            nodes: rects.clone(),
                        },
                        self.session.selected.clone(),
                        Vec::new(),
                    );

                    self.apply_history_group(&group, true);
                    self.push_history_group(group);
                }
            }
        }

        let render_scene = RenderScene {
            rects: self
                .document
                .rects
                .iter()
                .map(|r| RectInstance {
                    pos: [r.pos.x, r.pos.y],
                    size: [r.size.x, r.size.y],
                    color: r.color,
                })
                .collect(),
        };

        let overlay_scene = self
            .session
            .update_overlay_scene(&batch.tool, &self.document.rects);

        let cursor = self
            .session
            .compute_cursor(&batch.tool, &self.document.rects);

        EngineOutput {
            camera: self.session.camera,
            render_scene,
            overlay_scene,
            cursor,
        }
    }

    fn undo(&mut self) {
        if !matches!(self.session.drag_state, DragState::Idle) {
            return;
        }

        if let Some(group) = self.undo_stack.pop() {
            self.apply_history_group(&group, false);
            self.redo_stack.push(group);
        }
    }

    fn redo(&mut self) {
        if !matches!(self.session.drag_state, DragState::Idle) {
            return;
        }

        if let Some(group) = self.redo_stack.pop() {
            self.apply_history_group(&group, true);
            self.undo_stack.push(group);
        }
    }

    fn geometry_change_for_rect(
        &self,
        id: NodeId,
        before: RectGeometry,
    ) -> Option<RectGeometryChange> {
        let rect = self.document.rect(id)?;
        let after = RectGeometry::from_rect(rect);
        if before != after {
            Some(RectGeometryChange { id, before, after })
        } else {
            None
        }
    }

    fn push_history_group(&mut self, group: HistoryGroup) {
        self.undo_stack.push(group);
        self.redo_stack.clear();
    }

    fn apply_history_group(&mut self, group: &HistoryGroup, forward: bool) {
        if forward {
            for entry in &group.entries {
                self.document.apply_op(&entry.forward);
            }
            self.session.selected = group.selection_after.clone();
        } else {
            for entry in group.entries.iter().rev() {
                self.document.apply_op(&entry.inverse);
            }
            self.session.selected = group.selection_before.clone();
        }
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use crate::Camera;
    use crate::CursorStyle;
    use crate::PendingSelectionMove;
    use crate::ToolMode;

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
            document: DocumentModel::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            session: EditorSession {
                camera: Camera {
                    pan: Vec2::new(0.0, 0.0),
                    zoom: 2.0,
                },
                ..EditorSession::default()
            },
        };

        let batch = InputBatch {
            events: vec![InputEvent::CameraPanByScreenDelta {
                delta_px: Vec2::new(20.0, 10.0),
            }],
            tool: ToolMode::Select,
        };

        engine.tick(&batch);

        // same expectations as the direct Camera test
        assert_vec2_approx(engine.session.camera.pan, Vec2::new(-10.0, -5.0), 1e-6);
        assert_approx(engine.session.camera.zoom, 2.0, 1e-6);
    }

    #[test]
    fn tick_applies_zoom_event_and_preserves_world_point_under_cursor() {
        let mut engine = Engine {
            document: DocumentModel::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            session: EditorSession {
                camera: Camera {
                    pan: Vec2::new(0.0, 0.0),
                    zoom: 2.0,
                },
                ..EditorSession::default()
            },
        };

        let pivot = Vec2::new(300.0, 120.0);
        let world_before = engine.session.camera.screen_to_world(pivot);

        let batch = InputBatch {
            events: vec![InputEvent::CameraZoomAtScreenPoint {
                pivot_px: pivot,
                zoom_multiplier: 1.5,
            }],
            tool: ToolMode::Select,
        };

        engine.tick(&batch);

        assert_approx(engine.session.camera.zoom, 3.0, 1e-6);
        let world_after = engine.session.camera.screen_to_world(pivot);
        assert_vec2_approx(world_after, world_before, 1e-4);
    }

    #[test]
    fn tick_applies_events_in_order() {
        let mut engine = Engine::new();
        engine.session.camera.pan = Vec2::new(5.0, -7.0);
        engine.session.camera.zoom = 2.0;

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
            tool: ToolMode::Select,
        };

        // expected result: applying the same ops directly, in the same order
        let mut expected = engine.session.camera;
        expected.pan_by_screen_delta(Vec2::new(40.0, 0.0));
        expected.zoom_at_screen_point(Vec2::new(100.0, 50.0), 0.5);

        engine.tick(&batch);

        assert_vec2_approx(engine.session.camera.pan, expected.pan, 1e-5);
        assert_approx(engine.session.camera.zoom, expected.zoom, 1e-6);
    }

    #[test]
    fn hit_test_picks_topmost_rect() {
        let engine = Engine::new();
        let top_id = engine.document.rects[2].id;
        let hit = engine.document.check_collide_rects(Vec2::new(610.0, 910.0));
        assert_eq!(hit, Some(top_id));
    }

    #[test]
    fn selection_rules_apply_correcly() {
        let mut engine = Engine::new();
        let id = engine.document.rects[0].id;

        engine.session.apply_selection(Some(id), false);
        assert_eq!(engine.session.selected, vec![id]);

        engine.session.apply_selection(Some(id), true);
        assert!(engine.session.selected.is_empty());

        engine.session.apply_selection(None, false);
        assert!(engine.session.selected.is_empty());
    }

    /// Helper: build a minimal Engine with a single 100x100 rect at (50, 50).
    fn engine_with_one_rect() -> Engine {
        let mut doc = DocumentModel::new();
        let id = doc.alloc_id();
        doc.rects.push(RectNode {
            id,
            pos: Vec2::new(50.0, 50.0),
            size: Vec2::new(100.0, 100.0),
            color: [1.0, 0.0, 0.0, 1.0],
        });
        Engine {
            document: doc,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            session: EditorSession::default(),
        }
    }

    /// Helper: build a minimal Engine with two non-overlapping 100x100 rects.
    fn engine_with_two_rects() -> Engine {
        let mut doc = DocumentModel::new();
        let id0 = doc.alloc_id();
        let id1 = doc.alloc_id();
        doc.rects.push(RectNode {
            id: id0,
            pos: Vec2::new(50.0, 50.0),
            size: Vec2::new(100.0, 100.0),
            color: [1.0, 0.0, 0.0, 1.0],
        });
        doc.rects.push(RectNode {
            id: id1,
            pos: Vec2::new(300.0, 50.0),
            size: Vec2::new(100.0, 100.0),
            color: [0.0, 0.0, 1.0, 1.0],
        });
        Engine {
            document: doc,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            session: EditorSession::default(),
        }
    }

    // -----------------------------------------------------------------------
    // Selection move — single selection
    // -----------------------------------------------------------------------

    /// Clicking a rect selects it (PendingSelectionMove after PointerDown on
    /// an already-selected rect). Moving beyond the drag threshold actually
    /// moves the rect. PointerUp finalises the position.
    #[test]
    fn single_selection_drag_moves_rect() {
        let mut engine = engine_with_one_rect();
        let id = engine.document.rects[0].id;
        let origin = engine.document.rects[0].pos;

        // First click: select the rect (hit on unselected → apply_selection, stays Idle).
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerDown {
                screen_px: Vec2::new(100.0, 100.0), // inside rect (50..150, 50..150)
                shift: false,
                button: 0,
            }],
            tool: ToolMode::Select,
        });
        assert_eq!(engine.session.selected, vec![id]);

        // Second click on the now-selected rect → PendingSelectionMove.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerDown {
                screen_px: Vec2::new(100.0, 100.0),
                shift: false,
                button: 0,
            }],
            tool: ToolMode::Select,
        });
        assert!(matches!(
            engine.session.drag_state,
            DragState::PendingSelectionMove(_)
        ));

        // Move beyond 6 px threshold → SelectionMove + rect displaced.
        // World == screen at zoom=1, pan=(0,0).  Delta in world = (30, 20).
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerMove {
                screen_px: Vec2::new(130.0, 120.0),
                buttons: 1,
            }],
            tool: ToolMode::Select,
        });
        assert!(matches!(
            engine.session.drag_state,
            DragState::SelectionMove(_)
        ));
        let pos_mid = engine.document.rects[0].pos;
        assert_vec2_approx(pos_mid, Vec2::new(origin.x + 30.0, origin.y + 20.0), 1e-4);

        // Continue dragging further.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerMove {
                screen_px: Vec2::new(160.0, 150.0),
                buttons: 1,
            }],
            tool: ToolMode::Select,
        });
        let pos_far = engine.document.rects[0].pos;
        assert_vec2_approx(pos_far, Vec2::new(origin.x + 60.0, origin.y + 50.0), 1e-4);

        // Release pointer → Idle, position retained.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerUp {
                screen_px: Vec2::new(160.0, 150.0),
                button: 0,
            }],
            tool: ToolMode::Select,
        });
        assert!(matches!(engine.session.drag_state, DragState::Idle));
        assert_vec2_approx(engine.document.rects[0].pos, pos_far, 1e-4);
    }

    /// A move below the 6 px drag threshold must NOT start SelectionMove and
    /// must NOT displace the rect.
    #[test]
    fn single_selection_drag_below_threshold_does_not_move_rect() {
        let mut engine = engine_with_one_rect();
        let id = engine.document.rects[0].id;
        let origin = engine.document.rects[0].pos;

        // Select then enter PendingSelectionMove.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerDown {
                screen_px: Vec2::new(100.0, 100.0),
                shift: false,
                button: 0,
            }],
            tool: ToolMode::Select,
        });
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerDown {
                screen_px: Vec2::new(100.0, 100.0),
                shift: false,
                button: 0,
            }],
            tool: ToolMode::Select,
        });
        assert!(matches!(
            engine.session.drag_state,
            DragState::PendingSelectionMove(_)
        ));
        assert_eq!(engine.session.selected, vec![id]);

        // Move only 3 px — below 6 px threshold.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerMove {
                screen_px: Vec2::new(103.0, 100.0),
                buttons: 1,
            }],
            tool: ToolMode::Select,
        });
        // Should still be pending, not moved.
        assert!(matches!(
            engine.session.drag_state,
            DragState::PendingSelectionMove(_)
        ));
        assert_vec2_approx(engine.document.rects[0].pos, origin, 1e-4);
    }

    /// Cancelling a drag mid-flight must stop the move (state → Idle) but the
    /// rect keeps its last displaced position (no automatic rollback).
    #[test]
    fn single_selection_drag_cancel_stops_move() {
        let mut engine = engine_with_one_rect();
        let origin = engine.document.rects[0].pos;

        // Select.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerDown {
                screen_px: Vec2::new(100.0, 100.0),
                shift: false,
                button: 0,
            }],
            tool: ToolMode::Select,
        });
        // Enter PendingSelectionMove.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerDown {
                screen_px: Vec2::new(100.0, 100.0),
                shift: false,
                button: 0,
            }],
            tool: ToolMode::Select,
        });
        // Start move.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerMove {
                screen_px: Vec2::new(150.0, 100.0),
                buttons: 1,
            }],
            tool: ToolMode::Select,
        });
        assert!(matches!(
            engine.session.drag_state,
            DragState::SelectionMove(_)
        ));
        let pos_after_move = engine.document.rects[0].pos;
        assert_vec2_approx(pos_after_move, Vec2::new(origin.x + 50.0, origin.y), 1e-4);

        // Cancel.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerCancel],
            tool: ToolMode::Select,
        });
        assert!(matches!(engine.session.drag_state, DragState::Idle));
        assert_vec2_approx(engine.document.rects[0].pos, origin, 1e-4);
    }

    // -----------------------------------------------------------------------
    // Selection move — multi-selection
    // -----------------------------------------------------------------------

    /// Dragging one of multiple selected rects moves ALL of them by the same
    /// world-space delta, preserving their relative positions.
    #[test]
    fn multi_selection_drag_moves_all_selected_rects() {
        let mut engine = engine_with_two_rects();
        let id0 = engine.document.rects[0].id;
        let id1 = engine.document.rects[1].id;
        let origin0 = engine.document.rects[0].pos;
        let origin1 = engine.document.rects[1].pos;

        // Select rect 0.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerDown {
                screen_px: Vec2::new(100.0, 100.0), // inside rect 0
                shift: false,
                button: 0,
            }],
            tool: ToolMode::Select,
        });
        // Add rect 1 to selection with shift-click.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerDown {
                screen_px: Vec2::new(350.0, 100.0), // inside rect 1
                shift: true,
                button: 0,
            }],
            tool: ToolMode::Select,
        });
        assert!(engine.session.selected.contains(&id0));
        assert!(engine.session.selected.contains(&id1));

        // Click on rect 0 again (already selected, no shift) → PendingSelectionMove.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerDown {
                screen_px: Vec2::new(100.0, 100.0),
                shift: false,
                button: 0,
            }],
            tool: ToolMode::Select,
        });
        assert!(matches!(
            engine.session.drag_state,
            DragState::PendingSelectionMove(_)
        ));

        // Drag beyond threshold (dx=40, dy=25 world units at zoom=1).
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerMove {
                screen_px: Vec2::new(140.0, 125.0),
                buttons: 1,
            }],
            tool: ToolMode::Select,
        });
        assert!(matches!(
            engine.session.drag_state,
            DragState::SelectionMove(_)
        ));

        let pos0 = engine.document.rects[0].pos;
        let pos1 = engine.document.rects[1].pos;
        assert_vec2_approx(pos0, Vec2::new(origin0.x + 40.0, origin0.y + 25.0), 1e-4);
        assert_vec2_approx(pos1, Vec2::new(origin1.x + 40.0, origin1.y + 25.0), 1e-4);

        // Release.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerUp {
                screen_px: Vec2::new(140.0, 125.0),
                button: 0,
            }],
            tool: ToolMode::Select,
        });
        assert!(matches!(engine.session.drag_state, DragState::Idle));
        // Positions retained.
        assert_vec2_approx(engine.document.rects[0].pos, pos0, 1e-4);
        assert_vec2_approx(engine.document.rects[1].pos, pos1, 1e-4);
    }

    /// Only selected rects are moved; unselected rects remain in place.
    #[test]
    fn multi_selection_drag_does_not_move_unselected_rect() {
        let mut engine = engine_with_two_rects();
        let origin1 = engine.document.rects[1].pos;

        // Select only rect 0.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerDown {
                screen_px: Vec2::new(100.0, 100.0),
                shift: false,
                button: 0,
            }],
            tool: ToolMode::Select,
        });
        // Enter PendingSelectionMove on rect 0.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerDown {
                screen_px: Vec2::new(100.0, 100.0),
                shift: false,
                button: 0,
            }],
            tool: ToolMode::Select,
        });
        // Drag.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerMove {
                screen_px: Vec2::new(150.0, 100.0),
                buttons: 1,
            }],
            tool: ToolMode::Select,
        });

        // Rect 1 (unselected) must not have moved.
        assert_vec2_approx(engine.document.rects[1].pos, origin1, 1e-4);
    }

    /// Dragging preserves no cumulative drift across multiple sequential move
    /// events: the final position equals origin + total_delta, not a sum of
    /// per-frame deltas applied on top of each other.
    #[test]
    fn selection_drag_no_cumulative_drift() {
        let mut engine = engine_with_one_rect();
        let origin = engine.document.rects[0].pos;

        // Select.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerDown {
                screen_px: Vec2::new(100.0, 100.0),
                shift: false,
                button: 0,
            }],
            tool: ToolMode::Select,
        });
        // PendingSelectionMove.
        engine.tick(&InputBatch {
            events: vec![InputEvent::PointerDown {
                screen_px: Vec2::new(100.0, 100.0),
                shift: false,
                button: 0,
            }],
            tool: ToolMode::Select,
        });

        // Many small moves — each frame advances 1 px.
        for i in 1..=50u32 {
            engine.tick(&InputBatch {
                events: vec![InputEvent::PointerMove {
                    screen_px: Vec2::new(100.0 + i as f32, 100.0),
                    buttons: 1,
                }],
                tool: ToolMode::Select,
            });
        }

        // Final position should be origin + 50 px, NOT origin + sum(1..=50).
        assert_vec2_approx(
            engine.document.rects[0].pos,
            Vec2::new(origin.x + 50.0, origin.y),
            1e-3,
        );
    }

    #[test]
    fn cursor_defaults_to_default_with_not_hover() {
        let engine = engine_with_one_rect();
        let cursor = engine
            .session
            .compute_cursor(&ToolMode::Select, &engine.document.rects);
        assert_eq!(cursor, CursorStyle::Default);
    }

    #[test]
    fn cursor_is_resize_tl_br_when_hovering_tl_handle() {
        let mut engine = engine_with_one_rect();
        let id = engine.document.rects[0].id;
        engine.session.selected = vec![id];

        engine.session.hover_screen_px = Some(Vec2::new(50.0, 50.0));
        let cursor = engine
            .session
            .compute_cursor(&ToolMode::Select, &engine.document.rects);
        assert_eq!(cursor, CursorStyle::ResizeTlBr);
    }

    #[test]
    fn cursor_is_resize_tr_bl_when_hovering_tr_handle() {
        let mut engine = engine_with_one_rect();
        let id = engine.document.rects[0].id;
        engine.session.selected = vec![id];

        engine.session.hover_screen_px = Some(Vec2::new(150.0, 50.0));
        let cursor = engine
            .session
            .compute_cursor(&ToolMode::Select, &engine.document.rects);
        assert_eq!(cursor, CursorStyle::ResizeTrBl);
    }

    #[test]
    fn cursor_is_default_when_outside_handle_radius() {
        let mut engine = engine_with_one_rect();
        let id = engine.document.rects[0].id;
        engine.session.selected = vec![id];
        // Far from any handle
        engine.session.hover_screen_px = Some(Vec2::new(100.0, 100.0)); // center of rect
        let cursor = engine
            .session
            .compute_cursor(&ToolMode::Select, &engine.document.rects);
        assert_eq!(cursor, CursorStyle::Default);
    }

    #[test]
    fn cursor_is_move_during_selection_drag() {
        let mut engine = engine_with_one_rect();
        engine.session.drag_state = DragState::PendingSelectionMove(PendingSelectionMove {
            start_screen_px: Vec2::new(100.0, 100.0),
            start_world: Vec2::new(100.0, 100.0),
            previous_selection: vec![],
        });
        let cursor = engine
            .session
            .compute_cursor(&ToolMode::Select, &engine.document.rects);
        assert_eq!(cursor, CursorStyle::Move);
    }
}
