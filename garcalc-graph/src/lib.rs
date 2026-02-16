//! garcalc-graph: 2D/3D graphing engine
//!
//! Provides function plotting, parametric curves, implicit curves,
//! and 3D surface visualization.

pub mod plot2d;
pub mod plot3d;

pub use plot2d::{CURVE_COLORS, Graph2D, PlotConfig, PlottedFunction, ViewportPreset};
pub use plot3d::{Camera3D, CameraPreset, Colormap, Graph3D, Plot3DConfig, RenderMode, Surface3D, Viewport3D};

use garcalc_cas::Expr;
use serde::{Deserialize, Serialize};

/// Color for graph elements
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const RED: Self = Self {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const BLUE: Self = Self {
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    };
    pub const GREEN: Self = Self {
        r: 0,
        g: 128,
        b: 0,
        a: 255,
    };
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
}

/// Line style for curves
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LineStyle {
    Solid,
    Dashed,
    Dotted,
}

/// A single piece of a piecewise function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiecewisePiece {
    pub expr: Expr,
    pub condition: PiecewiseCondition,
}

/// Condition for a piecewise segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PiecewiseCondition {
    LessThan(f64),
    LessEqual(f64),
    GreaterThan(f64),
    GreaterEqual(f64),
    Between(f64, f64),
    BetweenInclusive(f64, f64),
    Always,
}

impl PiecewiseCondition {
    pub fn matches(&self, x: f64) -> bool {
        match self {
            Self::LessThan(v) => x < *v,
            Self::LessEqual(v) => x <= *v,
            Self::GreaterThan(v) => x > *v,
            Self::GreaterEqual(v) => x >= *v,
            Self::Between(a, b) => x > *a && x < *b,
            Self::BetweenInclusive(a, b) => x >= *a && x <= *b,
            Self::Always => true,
        }
    }
}

/// A plottable function or relation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Plottable {
    /// y = f(x)
    Explicit2D {
        expr: Expr,
        x_var: String,
        color: Color,
        style: LineStyle,
    },
    /// F(x, y) = 0
    Implicit2D {
        expr: Expr,
        x_var: String,
        y_var: String,
        color: Color,
    },
    /// (x(t), y(t))
    Parametric2D {
        x_expr: Expr,
        y_expr: Expr,
        t_var: String,
        t_range: (f64, f64),
        color: Color,
    },
    /// r = f(theta)
    Polar2D {
        expr: Expr,
        theta_var: String,
        theta_range: (f64, f64),
        color: Color,
        style: LineStyle,
    },
    /// Piecewise-defined function
    Piecewise2D {
        pieces: Vec<PiecewisePiece>,
        x_var: String,
        color: Color,
        style: LineStyle,
    },
    /// z = f(x, y)
    Explicit3D {
        expr: Expr,
        x_var: String,
        y_var: String,
    },
}

/// Extended color palette (6 base catppuccin + 6 additional)
pub const COLOR_PALETTE: [Color; 12] = [
    // Original 6 from CURVE_COLORS
    Color { r: 137, g: 180, b: 250, a: 255 }, // blue
    Color { r: 166, g: 227, b: 161, a: 255 }, // green
    Color { r: 249, g: 226, b: 175, a: 255 }, // yellow
    Color { r: 243, g: 139, b: 168, a: 255 }, // red
    Color { r: 203, g: 166, b: 247, a: 255 }, // mauve
    Color { r: 148, g: 226, b: 213, a: 255 }, // teal
    // Additional 6
    Color { r: 245, g: 224, b: 220, a: 255 }, // rosewater
    Color { r: 242, g: 205, b: 205, a: 255 }, // flamingo
    Color { r: 245, g: 194, b: 231, a: 255 }, // pink
    Color { r: 250, g: 179, b: 135, a: 255 }, // peach
    Color { r: 180, g: 190, b: 254, a: 255 }, // lavender
    Color { r: 137, g: 220, b: 235, a: 255 }, // sky
];

impl Plottable {
    pub fn color(&self) -> Option<Color> {
        match self {
            Self::Explicit2D { color, .. }
            | Self::Implicit2D { color, .. }
            | Self::Parametric2D { color, .. }
            | Self::Polar2D { color, .. }
            | Self::Piecewise2D { color, .. } => Some(*color),
            Self::Explicit3D { .. } => None,
        }
    }

    pub fn set_color(&mut self, new_color: Color) {
        match self {
            Self::Explicit2D { color, .. }
            | Self::Implicit2D { color, .. }
            | Self::Parametric2D { color, .. }
            | Self::Polar2D { color, .. }
            | Self::Piecewise2D { color, .. } => *color = new_color,
            Self::Explicit3D { .. } => {}
        }
    }
}

/// 2D viewport bounds
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Viewport2D {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

impl Default for Viewport2D {
    fn default() -> Self {
        Self {
            x_min: -10.0,
            x_max: 10.0,
            y_min: -10.0,
            y_max: 10.0,
        }
    }
}
