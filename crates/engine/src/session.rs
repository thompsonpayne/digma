use std::collections::HashSet;

use crate::{
    Corner, CursorStyle, DragState, HandleHit, NodeId, OverlayScene, RectInstance, RectNode,
    ToolMode, Vec2,
    camera::Camera,
    drag::{
        MarqueeDrag, PendingMarquee, PendingRectCreate, PendingResize, PendingSelectionMove,
        RectCreateDrag, ResizeDrag, SelectionDrag, compute_resize,
    },
    render_scene,
    types::DocumentModel,
};

#[derive(Debug)]
pub struct EditorSession {
    pub camera: Camera,
    pub selected: Vec<NodeId>,
    pub drag_state: DragState,
    pub hover_screen_px: Option<Vec2>,
}

impl Default for EditorSession {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            selected: vec![],
            drag_state: DragState::Idle,
            hover_screen_px: None,
        }
    }
}

impl EditorSession {
    pub fn pointer_down(
        &mut self,
        document: &DocumentModel,
        tool: ToolMode,
        screen_px: Vec2,
        shift: bool,
    ) {
        let world = self.camera.screen_to_world(screen_px);

        if tool == ToolMode::Rect {
            self.drag_state = DragState::PendingRectCreate(PendingRectCreate {
                start_screen_px: screen_px,
                start_world: world,
                previous_selection: self.selected.clone(),
            });
            return;
        }

        if tool == ToolMode::Select
            && let Some(handle_hit) = self.check_collide_handle(world, &document.rects)
        {
            self.drag_state = DragState::PendingResize(PendingResize {
                handle: handle_hit,
                start_screen_px: screen_px,
                start_world: world,
            });
            return;
        }

        let hit = document.check_collide_rects(world);

        self.drag_state = if let Some(hit_id) = hit {
            let hit_was_selected = self.selected.contains(&hit_id);

            if hit_was_selected && !shift {
                DragState::PendingSelectionMove(PendingSelectionMove {
                    start_screen_px: screen_px,
                    start_world: world,
                    previous_selection: self.selected.clone(),
                })
            } else {
                self.apply_selection(Some(hit_id), shift);
                DragState::Idle
            }
        } else {
            self.apply_selection(None, shift);
            DragState::PendingMarquee(PendingMarquee {
                start_screen_px: screen_px,
                start_world: world,
                additive: shift,
            })
        };
    }

    pub fn pointer_move(
        &mut self,
        document: &mut DocumentModel,
        screen_px: Vec2,
        drag_threshold_sq: f32,
    ) {
        self.hover_screen_px = Some(screen_px);
        let world = self.camera.screen_to_world(screen_px);

        self.update_marquee_drag(screen_px, world, drag_threshold_sq, &document.rects);
        self.update_move_drag(screen_px, world, drag_threshold_sq, &mut document.rects);
        self.update_resize_drag(screen_px, world, drag_threshold_sq, &mut document.rects);
        self.update_rect_create_drag(screen_px, world, drag_threshold_sq);
    }

    pub fn pointer_cancel(&mut self, document: &mut DocumentModel) {
        self.rollback_active_drag(&mut document.rects);
    }

    /// Apply a selection change.
    ///
    /// # Arguments
    /// * `hit` - The `NodeId` that was interacted with, or `None` if empty space was clicked.
    /// * `shift` - `true` if the shift key was held down (typically used for multi-selection).
    pub fn apply_selection(&mut self, hit: Option<NodeId>, shift: bool) {
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

    /// Returns the handle hit if `world` is within grab distance of any
    /// corner handle of the single selected rect. Returns `None` if
    /// nothing is selected, more than one rect is selected, or the point
    /// misses all handles.
    pub fn check_collide_handle(&self, world: Vec2, rects: &[RectNode]) -> Option<HandleHit> {
        // Only active for single selection
        if self.selected.len() != 1 {
            return None;
        }

        let id = self.selected[0];
        let rect = rects.iter().find(|r| r.id == id)?;
        let (x, y, w, h) = (rect.pos.x, rect.pos.y, rect.size.x, rect.size.y);

        // hit radius in world units (slightly larger than visual handle — 8px in init_overlay_scene)
        let hit_px = 12.0_f32;
        let hit_r = hit_px / self.camera.zoom;

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

    pub fn update_marquee_drag(
        &mut self,
        screen_px: Vec2,
        world: Vec2,
        threshold_sq: f32,
        rects: &[RectNode],
    ) {
        let next: Option<DragState> = match &self.drag_state {
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
            self.drag_state = state;
            self.update_marquee_selection(rects);
        }
    }

    /// Update marquee selection bounds and recompute the selected set.
    pub fn update_marquee_selection(&mut self, rects: &[RectNode]) {
        let DragState::Marquee(drag) = &mut self.drag_state else {
            return;
        };

        let min_x = drag.start_world.x.min(drag.current_world.x);
        let min_y = drag.start_world.y.min(drag.current_world.y);
        let max_x = drag.start_world.x.max(drag.current_world.x);
        let max_y = drag.start_world.y.max(drag.current_world.y);

        let mut selected = if drag.additive {
            self.selected.clone()
        } else {
            Vec::new()
        };

        for rect in rects {
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

        self.selected = selected;
    }

    pub fn update_move_drag(
        &mut self,
        screen_px: Vec2,
        world: Vec2,
        drag_threshold_sq: f32,
        rects: &mut [RectNode],
    ) {
        let next: Option<DragState> = match &self.drag_state {
            DragState::PendingSelectionMove(pending) => {
                let dx = screen_px.x - pending.start_screen_px.x;
                let dy = screen_px.y - pending.start_screen_px.y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq >= drag_threshold_sq {
                    let selected_ids: HashSet<NodeId> = self.selected.iter().copied().collect();
                    let origins: Vec<(NodeId, Vec2)> = rects
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
            self.drag_state = state;
            self.apply_selection_drag(rects);
        }
    }

    /// Update rect positions when `DragState` is `SelectionMove`.
    fn apply_selection_drag(&mut self, rects: &mut [RectNode]) {
        let (dx, dy, origins) = match &self.drag_state {
            DragState::SelectionMove(drag) => (
                drag.current_world.x - drag.start_world.x,
                drag.current_world.y - drag.start_world.y,
                drag.origins.clone(),
            ),
            _ => return,
        };

        for (node_id, origin) in &origins {
            if let Some(rect) = rects.iter_mut().find(|rect| rect.id == *node_id) {
                rect.pos.x = origin.x + dx;
                rect.pos.y = origin.y + dy;
            }
        }
    }

    pub fn update_resize_drag(
        &mut self,
        screen_px: Vec2,
        world: Vec2,
        drag_threshold_sq: f32,
        rects: &mut [RectNode],
    ) {
        let next: Option<DragState> = match &self.drag_state {
            DragState::PendingResize(pending) => {
                let dx = screen_px.x - pending.start_screen_px.x;
                let dy = screen_px.y - pending.start_screen_px.y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq >= drag_threshold_sq {
                    let rect_idx = rects
                        .iter()
                        .position(|r| r.id == pending.handle.node_id)
                        .unwrap();

                    let rect = rects[rect_idx];

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
            self.drag_state = state;
            self.apply_selection_resize(rects);
        }
    }

    fn apply_selection_resize(&mut self, rects: &mut [RectNode]) {
        let (corner, rect_idx, dx, dy, origin_pos, origin_size) = match &self.drag_state {
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

        if let Some(rect) = rects.get_mut(rect_idx) {
            rect.pos = new_pos;
            rect.size = new_size;
        }
    }

    pub fn update_rect_create_drag(
        &mut self,
        screen_px: Vec2,
        world: Vec2,
        drag_threshold_sq: f32,
    ) {
        let next: Option<DragState> = match &self.drag_state {
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
            self.drag_state = state;
        }
    }

    pub fn rollback_active_drag(&mut self, rects: &mut [RectNode]) {
        enum Rollback {
            SelectionMove(Vec<(NodeId, Vec2)>),
            Resize {
                node_id: NodeId,
                origin_pos: Vec2,
                origin_size: Vec2,
            },
            None,
        }

        let drag_state = std::mem::replace(&mut self.drag_state, DragState::Idle);

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
                    if let Some(rect) = rects.iter_mut().find(|rect| rect.id == id) {
                        rect.pos = origin;
                    }
                }
            }
            Rollback::Resize {
                node_id,
                origin_pos,
                origin_size,
            } => {
                if let Some(rect) = rects.iter_mut().find(|rect| rect.id == node_id) {
                    rect.pos = origin_pos;
                    rect.size = origin_size;
                }
            }
            _ => {}
        }
    }

    /// Determine the cursor style to show based on current hover position and drag state.
    pub fn compute_cursor(&self, tool_mode: &ToolMode, rects: &[RectNode]) -> CursorStyle {
        // Show the rect create cross hair cursor
        if matches!(
            self.drag_state,
            DragState::PendingRectCreate(_) | DragState::RectCreate(_)
        ) {
            return CursorStyle::Crosshair;
        }

        // During an active move drag, always show the move cursor.
        if matches!(
            self.drag_state,
            DragState::SelectionMove(_) | DragState::PendingSelectionMove(_)
        ) {
            return CursorStyle::Move;
        }

        // If hovering over a handle, show the appropriate resize cursor.
        if matches!(tool_mode, ToolMode::Select)
            && let Some(screen_px) = self.hover_screen_px
        {
            let world = self.camera.screen_to_world(screen_px);
            if let Some(hit) = self.check_collide_handle(world, rects) {
                return match hit.corner {
                    Corner::TL | Corner::BR => CursorStyle::ResizeTlBr,
                    Corner::TR | Corner::BL => CursorStyle::ResizeTrBl,
                };
            }
        }

        CursorStyle::Default
    }

    pub fn update_overlay_scene(&self, tool_mode: &ToolMode, rects: &[RectNode]) -> OverlayScene {
        let outline_px = 2.0;
        let handle_px = 8.0;
        let outline = outline_px / self.camera.zoom;
        let handle = handle_px / self.camera.zoom;
        let outline_color = [0.95, 0.95, 0.95, 1.0];
        let handle_color = [0.1, 0.6, 1.0, 1.0];
        let mut overlay_rects = Vec::new();
        for id in &self.selected {
            if matches!(tool_mode, ToolMode::Rect) {
                break;
            }

            let Some(rect) = rects.iter().find(|r| r.id == *id) else {
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

        if let DragState::Marquee(drag) = &self.drag_state {
            let min_x = drag.start_world.x.min(drag.current_world.x);
            let min_y = drag.start_world.y.min(drag.current_world.y);
            let max_x = drag.start_world.x.max(drag.current_world.x);
            let max_y = drag.start_world.y.max(drag.current_world.y);

            let w = (max_x - min_x).max(0.0);
            let h = (max_y - min_y).max(0.0);

            let fill_color = [0.2, 0.6, 1.0, 0.08];
            let outline_color = [0.2, 0.6, 1.0, 0.9];
            let outline_px = 1.0 / self.camera.zoom;

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

        if let DragState::RectCreate(drag) = &self.drag_state {
            let min_x = drag.start_world.x.min(drag.current_world.x);
            let min_y = drag.start_world.y.min(drag.current_world.y);
            let max_x = drag.start_world.x.max(drag.current_world.x);
            let max_y = drag.start_world.y.max(drag.current_world.y);

            let w = (max_x - min_x).max(0.0);
            let h = (max_y - min_y).max(0.0);

            let fill_color = [0.2, 0.6, 1.0, 0.08];
            let outline_color = [0.2, 0.6, 1.0, 0.9];
            let outline_px = 1.0 / self.camera.zoom;

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
}
