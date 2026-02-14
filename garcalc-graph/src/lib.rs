//! garcalc-graph: 2D/3D graphing engine
//!
//! Provides function plotting, parametric curves, implicit curves,
//! and 3D surface visualization.

pub mod plot2d;
pub mod plot3d;

pub use plot2d::{CURVE_COLORS, Graph2D, PlotConfig};
pub use plot3d::{Camera3D, Colormap, Graph3D, Plot3DConfig, RenderMode, Viewport3D};

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
    /// z = f(x, y)
    Explicit3D {
        expr: Expr,
        x_var: String,
        y_var: String,
    },
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
