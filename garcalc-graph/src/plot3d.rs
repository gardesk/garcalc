//! 3D plotting (Sprint 4)

/// 3D camera state
#[derive(Debug, Clone, Copy)]
pub struct Camera3D {
    pub azimuth: f64,
    pub elevation: f64,
    pub distance: f64,
    pub target: (f64, f64, f64),
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            azimuth: 45.0_f64.to_radians(),
            elevation: 30.0_f64.to_radians(),
            distance: 20.0,
            target: (0.0, 0.0, 0.0),
        }
    }
}
