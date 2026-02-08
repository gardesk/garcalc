//! 2D plotting (Sprint 3)

use crate::Viewport2D;

/// 2D graph state
pub struct Graph2D {
    pub viewport: Viewport2D,
    pub grid_enabled: bool,
    pub axis_labels: bool,
}

impl Default for Graph2D {
    fn default() -> Self {
        Self {
            viewport: Viewport2D::default(),
            grid_enabled: true,
            axis_labels: true,
        }
    }
}
