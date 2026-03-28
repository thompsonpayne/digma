use std::collections::HashSet;

use crate::drag::{
    Corner, DragState, HandleHit, MarqueeDrag, PendingMarquee, PendingRectCreate, PendingResize,
    PendingSelectionMove, RectCreateDrag, ResizeDrag, SelectionDrag, compute_resize,
};
use crate::input::{CursorStyle, EngineOutput, InputBatch, InputEvent};
use crate::render_scene::{self, OverlayScene, RectInstance, RenderScene};
use crate::types::{DocumentModel, NodeId, RectNode, Vec2};
use crate::{EditorSession, RectGeometry, RectGeometryChange, ToolCommand, ToolMode};

pub struct Engine {
    pub document: DocumentModel,
    pub session: EditorSession,

    // pub camera: Camera,
    // pub selected: Vec<NodeId>,
    // pub drag_state: DragState,
    // pub hover_screen_px: Option<Vec2>,
    undo_stack: Vec<ToolCommand>,
    redo_stack: Vec<ToolCommand>,
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
                    let world = self.session.camera.screen_to_world(screen_px);

                    // handle rect create takes priority
                    if batch.tool == ToolMode::Rect {
                        self.session.drag_state = DragState::PendingRectCreate(PendingRectCreate {
                            start_screen_px: screen_px,
                            start_world: world,
                            previous_selection: self.session.selected.clone(),
                        });
                        continue;
                    }

                    // Handle hit takes priority over rect selection
                    if batch.tool == ToolMode::Select
                        && let Some(handle_hit) = self.check_collide_handle(world)
                    {
                        self.session.drag_state = DragState::PendingResize(PendingResize {
                            handle: handle_hit,
                            start_screen_px: screen_px,
                            start_world: world,
                        });
                        continue;
                    }

                    let hit = self.document.check_collide_rects(world);

                    self.session.drag_state = if let Some(hit_id) = hit {
                        // mouse down on a rect (hit)
                        let hit_was_selected = self.session.selected.contains(&hit_id);

                        if hit_was_selected && !shift {
                            DragState::PendingSelectionMove(PendingSelectionMove {
                                start_screen_px: screen_px,
                                start_world: world,
                                previous_selection: self.session.selected.clone(),
                            })
                        } else {
                            self.session.apply_selection(Some(hit_id), shift);
                            DragState::Idle
                        }
                    } else {
                        // mouse down on empty space with `select` tool
                        self.session.apply_selection(None, shift);
                        DragState::PendingMarquee(PendingMarquee {
                            start_screen_px: screen_px,
                            start_world: world,
                            additive: shift,
                        })
                    };
                }
                InputEvent::PointerMove {
                    screen_px,
                    buttons: _buttons,
                } => {
                    self.session.hover_screen_px = Some(screen_px);
                    let world = self.session.camera.screen_to_world(screen_px);

                    self.update_marquee_drag(screen_px, world, drag_threshold_sq);
                    self.update_move_drag(screen_px, world, drag_threshold_sq);
                    self.update_resize_drag(screen_px, world, drag_threshold_sq);
                    self.update_rect_create_drag(screen_px, world, drag_threshold_sq);
                }
                InputEvent::PointerUp {
                    screen_px,
                    button: _button,
                } => {
                    let world = self.session.camera.screen_to_world(screen_px);

                    if matches!(self.session.drag_state, DragState::Marquee(_)) {
                        self.update_marquee_drag(screen_px, world, drag_threshold_sq);
                    }

                    let drag_state =
                        std::mem::replace(&mut self.session.drag_state, DragState::Idle);

                    let command = match drag_state {
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

                            (!changes.is_empty())
                                .then_some(ToolCommand::SetRectsGeometry { changes })
                        }
                        DragState::Resize(drag) => self
                            .geometry_change_for_rect(
                                drag.handle.node_id,
                                RectGeometry {
                                    pos: drag.origin_pos,
                                    size: drag.origin_size,
                                },
                            )
                            .map(|change| ToolCommand::SetRectsGeometry {
                                changes: vec![change],
                            }),
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

                            Some(ToolCommand::CreateRect {
                                next_selection: vec![rect.id],
                                previous_selection: drag.previous_selection,
                                rect,
                            })
                        }
                        _ => None,
                    };

                    if let Some(command) = command {
                        if matches!(command, ToolCommand::CreateRect { .. }) {
                            self.apply_command(&command, true);
                        }
                        self.push_history(command);
                    }
                }
                InputEvent::PointerCancel => {
                    self.rollback_active_drag();
                }
                InputEvent::SetSelectionFill { color } => {
                    let selected: HashSet<NodeId> = self.session.selected.iter().copied().collect();

                    for rect in &mut self.document.rects {
                        if selected.contains(&rect.id) {
                            rect.color = [color.r, color.g, color.b, color.a];
                        }
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

                    let command = ToolCommand::BringForward(self.session.selected.clone());
                    self.apply_command(&command, true);
                    self.push_history(command);
                }
                InputEvent::SendBackward => {
                    if self.session.selected.is_empty() {
                        continue;
                    }

                    let command = ToolCommand::SendBackward(self.session.selected.clone());
                    self.apply_command(&command, true);
                    self.push_history(command);
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

                    let command = ToolCommand::Delete {
                        rects,
                        previous_selection: self.session.selected.clone(),
                        next_selection: Vec::new(),
                    };

                    self.apply_command(&command, true);
                    self.push_history(command);
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

        let overlay_scene = self.update_overlay_scene(&batch.tool);
        let cursor = self.compute_cursor(&batch.tool);

        EngineOutput {
            camera: self.session.camera,
            render_scene,
            overlay_scene,
            cursor,
        }
    }

    pub fn update_marquee_drag(&mut self, screen_px: Vec2, world: Vec2, threshold_sq: f32) {
        let next: Option<DragState> = match &self.session.drag_state {
            DragState::PendingMarquee(pending) => {
                let dx = screen_px.x - pending.start_screen_px.x;
                let dy = screen_px.y - pending.start_screen_px.y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq >= threshold_sq {
                    Some(DragState::Marquee(MarqueeDrag {
                        start_world: pending.start_world,
                        current_world: world,
                        additive: pending.additive,
                    }))
                } else {
                    None
                }
            }
            DragState::Marquee(d) => {
                let mut d = d.clone();
                d.current_world = world;
                Some(DragState::Marquee(d))
            }
            _ => None,
        };

        if let Some(state) = next {
            self.session.drag_state = state;
            self.update_marquee_selection();
        }
    }

    /// Returns the handle hit if `world` is within grab distance of any
    /// corner handle of the single selected rect. Returns `None` if
    /// nothing is selected, more than one rect is selected, or the point
    /// misses all handles.
    pub fn check_collide_handle(&self, world: Vec2) -> Option<HandleHit> {
        // Only active for single selection
        if self.session.selected.len() != 1 {
            return None;
        }

        let id = self.session.selected[0];
        let rect = self.document.rects.iter().find(|r| r.id == id)?;
        let (x, y, w, h) = (rect.pos.x, rect.pos.y, rect.size.x, rect.size.y);

        // hit radius in world units (slightly larger than visual handle — 8px in init_overlay_scene)
        let hit_px = 12.0_f32;
        let hit_r = hit_px / self.session.camera.zoom;

        let corners = [
            (Vec2::new(x, y), Corner::TL),
            (Vec2::new(x + w, y), Corner::TR),
            (Vec2::new(x, y + h), Corner::BL),
            (Vec2::new(x + w, y + h), Corner::BR),
        ];

        for (center, corner) in corners {
            if (world.x - center.x).abs() <= hit_r && (world.y - center.y).abs() <= hit_r {
                return Some(HandleHit {
                    node_id: id,
                    corner,
                });
            }
        }

        None
    }

    fn update_overlay_scene(&self, tool_mode: &ToolMode) -> OverlayScene {
        let outline_px = 2.0;
        let handle_px = 8.0;
        let outline = outline_px / self.session.camera.zoom;
        let handle = handle_px / self.session.camera.zoom;
        let outline_color = [0.95, 0.95, 0.95, 1.0];
        let handle_color = [0.1, 0.6, 1.0, 1.0];
        let mut overlay_rects = Vec::new();
        for id in &self.session.selected {
            if matches!(tool_mode, ToolMode::Rect) {
                break;
            }

            let Some(rect) = self.document.rects.iter().find(|r| r.id == *id) else {
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

        if let DragState::Marquee(drag) = &self.session.drag_state {
            let min_x = drag.start_world.x.min(drag.current_world.x);
            let min_y = drag.start_world.y.min(drag.current_world.y);
            let max_x = drag.start_world.x.max(drag.current_world.x);
            let max_y = drag.start_world.y.max(drag.current_world.y);

            let w = (max_x - min_x).max(0.0);
            let h = (max_y - min_y).max(0.0);

            let fill_color = [0.2, 0.6, 1.0, 0.08];
            let outline_color = [0.2, 0.6, 1.0, 0.9];
            let outline_px = 1.0 / self.session.camera.zoom;

            // fill
            overlay_rects.push(RectInstance {
                pos: [min_x, min_y],
                size: [w, h],
                color: fill_color,
            });

            // outline (4 thin rects)
            overlay_rects.push(RectInstance {
                pos: [min_x, min_y],
                size: [w, outline_px],
                color: outline_color,
            });
            overlay_rects.push(RectInstance {
                pos: [min_x, max_y - outline_px],
                size: [w, outline_px],
                color: outline_color,
            });
            overlay_rects.push(RectInstance {
                pos: [min_x, min_y],
                size: [outline_px, h],
                color: outline_color,
            });
            overlay_rects.push(RectInstance {
                pos: [max_x - outline_px, min_y],
                size: [outline_px, h],
                color: outline_color,
            });
        }

        if let DragState::RectCreate(drag) = &self.session.drag_state {
            let min_x = drag.start_world.x.min(drag.current_world.x);
            let min_y = drag.start_world.y.min(drag.current_world.y);
            let max_x = drag.start_world.x.max(drag.current_world.x);
            let max_y = drag.start_world.y.max(drag.current_world.y);

            let w = (max_x - min_x).max(0.0);
            let h = (max_y - min_y).max(0.0);

            let fill_color = [0.2, 0.6, 1.0, 0.08];
            let outline_color = [0.2, 0.6, 1.0, 0.9];
            let outline_px = 1.0 / self.session.camera.zoom;

            // fill
            overlay_rects.push(RectInstance {
                pos: [min_x, min_y],
                size: [w, h],
                color: fill_color,
            });

            // outline (4 thin rects)
            overlay_rects.push(RectInstance {
                pos: [min_x, min_y],
                size: [w, outline_px],
                color: outline_color,
            });
            overlay_rects.push(RectInstance {
                pos: [min_x, max_y - outline_px],
                size: [w, outline_px],
                color: outline_color,
            });
            overlay_rects.push(RectInstance {
                pos: [min_x, min_y],
                size: [outline_px, h],
                color: outline_color,
            });
            overlay_rects.push(RectInstance {
                pos: [max_x - outline_px, min_y],
                size: [outline_px, h],
                color: outline_color,
            });
        }

        render_scene::OverlayScene {
            rects: overlay_rects,
        }
    }

    /// Update marquee selection bounds and recompute the selected set.
    fn update_marquee_selection(&mut self) {
        let DragState::Marquee(drag) = &mut self.session.drag_state else {
            return;
        };

        let min_x = drag.start_world.x.min(drag.current_world.x);
        let min_y = drag.start_world.y.min(drag.current_world.y);
        let max_x = drag.start_world.x.max(drag.current_world.x);
        let max_y = drag.start_world.y.max(drag.current_world.y);

        let mut selected = if drag.additive {
            self.session.selected.clone()
        } else {
            Vec::new()
        };

        for rect in &self.document.rects {
            let rect_min_x = rect.pos.x;
            let rect_min_y = rect.pos.y;
            let rect_max_x = rect.pos.x + rect.size.x;
            let rect_max_y = rect.pos.y + rect.size.y;

            let intersects = rect_min_x < max_x
                && rect_max_x > min_x
                && rect_min_y < max_y
                && rect_max_y > min_y;

            if intersects && !selected.contains(&rect.id) {
                selected.push(rect.id);
            }
        }

        self.session.selected = selected;
    }

    /// Update rect positions when `DragState` is `SelectionMove`.
    fn apply_selection_drag(&mut self) {
        let (dx, dy, origins) = match &self.session.drag_state {
            DragState::SelectionMove(drag) => (
                drag.current_world.x - drag.start_world.x,
                drag.current_world.y - drag.start_world.y,
                drag.origins.clone(),
            ),
            _ => return,
        };

        for (node_id, origin) in &origins {
            if let Some(rect) = self
                .document
                .rects
                .iter_mut()
                .find(|rect| rect.id == *node_id)
            {
                rect.pos.x = origin.x + dx;
                rect.pos.y = origin.y + dy;
            }
        }
    }

    fn push_history(&mut self, command: ToolCommand) {
        self.undo_stack.push(command);
        self.redo_stack.clear();
    }

    fn apply_command(&mut self, command: &ToolCommand, forward: bool) {
        match command {
            ToolCommand::CreateRect {
                rect,
                previous_selection,
                next_selection,
            } => {
                if forward {
                    if self.document.rect_index(rect.id).is_none() {
                        self.document.rects.push(*rect);
                    }
                    self.session.selected = next_selection.clone();
                } else {
                    if let Some(idx) = self.document.rect_index(rect.id) {
                        self.document.rects.remove(idx);
                    }
                    self.session.selected = previous_selection.clone();
                }
            }
            ToolCommand::SetRectsGeometry { changes } => {
                for change in changes {
                    let geometry = if forward { change.after } else { change.before };
                    if let Some(rect) = self.document.rect_mut(change.id) {
                        rect.pos = geometry.pos;
                        rect.size = geometry.size;
                    }
                }
            }
            ToolCommand::BringForward(node_ids) => {
                self.document.reorder_selected(node_ids, forward);
            }
            ToolCommand::SendBackward(node_ids) => {
                self.document.reorder_selected(node_ids, !forward);
            }
            ToolCommand::Delete {
                rects,
                previous_selection,
                next_selection,
            } => {
                let deleted_ids: HashSet<NodeId> = rects.iter().map(|r| r.0.id).collect();
                if forward {
                    self.document
                        .rects
                        .retain(|rect| !deleted_ids.contains(&rect.id));
                    self.session.selected.retain(|id| !deleted_ids.contains(id));

                    self.session.selected = next_selection.clone();
                } else {
                    let mut restored = rects.clone();
                    restored.sort_by_key(|(_, original_index)| *original_index);

                    for (rect, original_index) in restored {
                        if self.document.rect_index(rect.id).is_none() {
                            let insert_at = original_index.min(self.document.rects.len());
                            self.document.rects.insert(insert_at, rect);
                        }
                    }
                    self.session.selected = previous_selection.clone();
                }
            }
        }
    }

    fn rollback_active_drag(&mut self) {
        enum Rollback {
            SelectionMove(Vec<(NodeId, Vec2)>),
            Resize {
                node_id: NodeId,
                origin_pos: Vec2,
                origin_size: Vec2,
            },
            None,
        }

        let drag_state = std::mem::replace(&mut self.session.drag_state, DragState::Idle);

        let rollback = match drag_state {
            DragState::SelectionMove(drag) => Rollback::SelectionMove(drag.origins),
            DragState::Resize(drag) => Rollback::Resize {
                node_id: drag.handle.node_id,
                origin_pos: drag.origin_pos,
                origin_size: drag.origin_size,
            },
            _ => Rollback::None,
        };

        match rollback {
            Rollback::SelectionMove(origins) => {
                for (id, origin) in origins {
                    if let Some(rect) = self.document.rect_mut(id) {
                        rect.pos = origin;
                    }
                }
            }
            Rollback::Resize {
                node_id,
                origin_pos,
                origin_size,
            } => {
                if let Some(rect) = self.document.rect_mut(node_id) {
                    rect.pos = origin_pos;
                    rect.size = origin_size;
                }
            }
            _ => {}
        }
    }

    fn undo(&mut self) {
        if !matches!(self.session.drag_state, DragState::Idle) {
            return;
        }

        if let Some(command) = self.undo_stack.pop() {
            self.apply_command(&command, false);
            self.redo_stack.push(command);
        }
    }

    fn redo(&mut self) {
        if !matches!(self.session.drag_state, DragState::Idle) {
            return;
        }

        if let Some(command) = self.redo_stack.pop() {
            self.apply_command(&command, true);
            self.undo_stack.push(command);
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

    fn apply_selection_resize(&mut self) {
        let (corner, rect_idx, dx, dy, origin_pos, origin_size) = match &self.session.drag_state {
            DragState::Resize(d) => (
                d.handle.corner,
                d.rect_idx,
                d.current_world.x - d.start_world.x,
                d.current_world.y - d.start_world.y,
                d.origin_pos,
                d.origin_size,
            ),
            _ => return,
        };

        let min_size = 1.0_f32;
        let (new_pos, new_size) = compute_resize(corner, dx, dy, origin_pos, origin_size, min_size);

        if let Some(rect) = self.document.rects.get_mut(rect_idx) {
            rect.pos = new_pos;
            rect.size = new_size;
        }
    }

    /// Determine the cursor style to show based on current hover position and drag state.
    pub fn compute_cursor(&self, tool_mode: &ToolMode) -> CursorStyle {
        // Show the rect create cross hair cursor
        if matches!(
            self.session.drag_state,
            DragState::PendingRectCreate(_) | DragState::RectCreate(_)
        ) {
            return CursorStyle::Crosshair;
        }

        // During an active move drag, always show the move cursor.
        if matches!(
            self.session.drag_state,
            DragState::SelectionMove(_) | DragState::PendingSelectionMove(_)
        ) {
            return CursorStyle::Move;
        }

        // If hovering over a handle, show the appropriate resize cursor.
        if matches!(tool_mode, ToolMode::Select)
            && let Some(screen_px) = self.session.hover_screen_px
        {
            let world = self.session.camera.screen_to_world(screen_px);
            if let Some(hit) = self.check_collide_handle(world) {
                return match hit.corner {
                    Corner::TL | Corner::BR => CursorStyle::ResizeTlBr,
                    Corner::TR | Corner::BL => CursorStyle::ResizeTrBl,
                };
            }
        }

        CursorStyle::Default
    }

    fn update_move_drag(&mut self, screen_px: Vec2, world: Vec2, drag_threshold_sq: f32) {
        let next: Option<DragState> = match &self.session.drag_state {
            DragState::PendingSelectionMove(pending) => {
                let dx = screen_px.x - pending.start_screen_px.x;
                let dy = screen_px.y - pending.start_screen_px.y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq >= drag_threshold_sq {
                    let selected_ids: HashSet<NodeId> =
                        self.session.selected.iter().copied().collect();
                    let origins: Vec<(NodeId, Vec2)> = self
                        .document
                        .rects
                        .iter()
                        .filter_map(|rect| {
                            selected_ids
                                .contains(&rect.id)
                                .then_some((rect.id, rect.pos))
                        })
                        .collect();

                    Some(DragState::SelectionMove(SelectionDrag {
                        start_world: pending.start_world,
                        current_world: world,
                        origins,
                    }))
                } else {
                    None
                }
            }
            DragState::SelectionMove(d) => {
                let mut d = d.clone();
                d.current_world = world;
                Some(DragState::SelectionMove(d))
            }
            _ => None,
        };

        if let Some(state) = next {
            self.session.drag_state = state;
            self.apply_selection_drag();
        }
    }

    fn update_resize_drag(&mut self, screen_px: Vec2, world: Vec2, drag_threshold_sq: f32) {
        let next: Option<DragState> = match &self.session.drag_state {
            DragState::PendingResize(pending) => {
                let dx = screen_px.x - pending.start_screen_px.x;
                let dy = screen_px.y - pending.start_screen_px.y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq >= drag_threshold_sq {
                    let rect_idx = self
                        .document
                        .rects
                        .iter()
                        .position(|r| r.id == pending.handle.node_id)
                        .unwrap();

                    let rect = &self.document.rects[rect_idx];

                    Some(DragState::Resize(ResizeDrag {
                        handle: pending.handle,
                        start_world: pending.start_world,
                        current_world: world,
                        origin_pos: rect.pos,   // snapshot
                        origin_size: rect.size, // snapshot
                        rect_idx,
                    }))
                } else {
                    None
                }
            }
            DragState::Resize(drag) => {
                let mut drag = drag.clone();
                drag.current_world = world;
                Some(DragState::Resize(drag))
            }
            _ => None,
        };

        if let Some(state) = next {
            self.session.drag_state = state;
            self.apply_selection_resize();
        }
    }

    fn update_rect_create_drag(&mut self, screen_px: Vec2, world: Vec2, drag_threshold_sq: f32) {
        let next: Option<DragState> = match &self.session.drag_state {
            DragState::PendingRectCreate(pending) => {
                let dx = screen_px.x - pending.start_screen_px.x;
                let dy = screen_px.y - pending.start_screen_px.y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq >= drag_threshold_sq {
                    Some(DragState::RectCreate(RectCreateDrag {
                        start_world: pending.start_world,
                        current_world: world,
                        previous_selection: pending.previous_selection.clone(),
                    }))
                } else {
                    None
                }
            }
            DragState::RectCreate(drag) => {
                let mut drag = drag.clone();
                drag.current_world = world;
                Some(DragState::RectCreate(drag))
            }
            _ => None,
        };

        if let Some(state) = next {
            self.session.drag_state = state;
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
            session: EditorSession::default(),
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
            session: EditorSession::default(),
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
        let cursor = engine.compute_cursor(&ToolMode::Select);
        assert_eq!(cursor, CursorStyle::Default);
    }

    #[test]
    fn cursor_is_resize_tl_br_when_hovering_tl_handle() {
        let mut engine = engine_with_one_rect();
        let id = engine.document.rects[0].id;
        engine.session.selected = vec![id];

        engine.session.hover_screen_px = Some(Vec2::new(50.0, 50.0));
        let cursor = engine.compute_cursor(&ToolMode::Select);
        assert_eq!(cursor, CursorStyle::ResizeTlBr);
    }

    #[test]
    fn cursor_is_resize_tr_bl_when_hovering_tr_handle() {
        let mut engine = engine_with_one_rect();
        let id = engine.document.rects[0].id;
        engine.session.selected = vec![id];

        engine.session.hover_screen_px = Some(Vec2::new(150.0, 50.0));
        let cursor = engine.compute_cursor(&ToolMode::Select);
        assert_eq!(cursor, CursorStyle::ResizeTrBl);
    }

    #[test]
    fn cursor_is_default_when_outside_handle_radius() {
        let mut engine = engine_with_one_rect();
        let id = engine.document.rects[0].id;
        engine.session.selected = vec![id];
        // Far from any handle
        engine.session.hover_screen_px = Some(Vec2::new(100.0, 100.0)); // center of rect
        let cursor = engine.compute_cursor(&ToolMode::Select);
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
        let cursor = engine.compute_cursor(&ToolMode::Select);
        assert_eq!(cursor, CursorStyle::Move);
    }
}
