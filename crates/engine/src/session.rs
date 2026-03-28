use crate::{DragState, NodeId, Vec2, camera::Camera};

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
}
