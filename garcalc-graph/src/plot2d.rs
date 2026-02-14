//! 2D plotting engine
//!
//! Provides function plotting with:
//! - Explicit functions: y = f(x)
//! - Parametric curves: (x(t), y(t))
//! - Coordinate system with axes and grid
//! - Pan/zoom interaction
//! - Trace mode

use cairo::Context;
use garcalc_cas::{Evaluator, Expr, Symbol};

use crate::{Color, LineStyle, Plottable, Viewport2D};

/// Configuration for the plot appearance
#[derive(Debug, Clone)]
pub struct PlotConfig {
    /// Background color
    pub background: Color,
    /// Axis color
    pub axis_color: Color,
    /// Grid color
    pub grid_color: Color,
    /// Tick label color
    pub label_color: Color,
    /// Whether to show grid
    pub show_grid: bool,
    /// Whether to show axis labels
    pub show_labels: bool,
    /// Line width for curves
    pub curve_width: f64,
    /// Line width for axes
    pub axis_width: f64,
    /// Line width for grid
    pub grid_width: f64,
    /// Number of samples per pixel for function plotting
    pub samples_per_pixel: f64,
}

impl Default for PlotConfig {
    fn default() -> Self {
        Self {
            background: Color {
                r: 30,
                g: 30,
                b: 46,
                a: 255,
            },
            axis_color: Color {
                r: 166,
                g: 173,
                b: 200,
                a: 255,
            },
            grid_color: Color {
                r: 69,
                g: 71,
                b: 90,
                a: 255,
            },
            label_color: Color {
                r: 166,
                g: 173,
                b: 200,
                a: 255,
            },
            show_grid: true,
            show_labels: true,
            curve_width: 2.0,
            axis_width: 1.5,
            grid_width: 0.5,
            samples_per_pixel: 2.0,
        }
    }
}

/// Default curve colors (catppuccin palette)
pub const CURVE_COLORS: [Color; 6] = [
    Color {
        r: 137,
        g: 180,
        b: 250,
        a: 255,
    }, // blue
    Color {
        r: 166,
        g: 227,
        b: 161,
        a: 255,
    }, // green
    Color {
        r: 249,
        g: 226,
        b: 175,
        a: 255,
    }, // yellow
    Color {
        r: 243,
        g: 139,
        b: 168,
        a: 255,
    }, // red
    Color {
        r: 203,
        g: 166,
        b: 247,
        a: 255,
    }, // mauve
    Color {
        r: 148,
        g: 226,
        b: 213,
        a: 255,
    }, // teal
];

/// 2D graph state and renderer
pub struct Graph2D {
    /// Current viewport
    pub viewport: Viewport2D,
    /// Plot configuration
    pub config: PlotConfig,
    /// Functions to plot
    pub functions: Vec<Plottable>,
    /// Trace mode: show cursor position
    pub trace_enabled: bool,
    /// Current trace position (screen coords)
    pub trace_pos: Option<(f64, f64)>,
}

impl Default for Graph2D {
    fn default() -> Self {
        Self {
            viewport: Viewport2D::default(),
            config: PlotConfig::default(),
            functions: Vec::new(),
            trace_enabled: false,
            trace_pos: None,
        }
    }
}

impl Graph2D {
    /// Create a new graph with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the viewport
    pub fn set_viewport(&mut self, viewport: Viewport2D) {
        self.viewport = viewport;
    }

    /// Add a function to plot
    pub fn add_function(&mut self, func: Plottable) {
        self.functions.push(func);
    }

    /// Add an explicit function y = f(x) with auto color
    pub fn add_explicit(&mut self, expr: Expr) {
        let color = CURVE_COLORS[self.functions.len() % CURVE_COLORS.len()];
        self.functions.push(Plottable::Explicit2D {
            expr,
            x_var: "x".to_string(),
            color,
            style: LineStyle::Solid,
        });
    }

    /// Add an implicit curve F(x,y) = 0
    pub fn add_implicit(&mut self, expr: Expr) {
        let color = CURVE_COLORS[self.functions.len() % CURVE_COLORS.len()];
        self.functions.push(Plottable::Implicit2D {
            expr,
            x_var: "x".to_string(),
            y_var: "y".to_string(),
            color,
        });
    }

    /// Add a parametric curve (x(t), y(t))
    pub fn add_parametric(&mut self, x_expr: Expr, y_expr: Expr, t_range: (f64, f64)) {
        let color = CURVE_COLORS[self.functions.len() % CURVE_COLORS.len()];
        self.functions.push(Plottable::Parametric2D {
            x_expr,
            y_expr,
            t_var: "t".to_string(),
            t_range,
            color,
        });
    }

    /// Clear all functions
    pub fn clear_functions(&mut self) {
        self.functions.clear();
    }

    /// Pan the viewport by a screen delta
    pub fn pan(&mut self, dx: f64, dy: f64, width: u32, height: u32) {
        let x_range = self.viewport.x_max - self.viewport.x_min;
        let y_range = self.viewport.y_max - self.viewport.y_min;

        let math_dx = -dx * x_range / width as f64;
        let math_dy = dy * y_range / height as f64;

        self.viewport.x_min += math_dx;
        self.viewport.x_max += math_dx;
        self.viewport.y_min += math_dy;
        self.viewport.y_max += math_dy;
    }

    /// Zoom by a factor centered at screen position
    pub fn zoom(&mut self, factor: f64, cx: f64, cy: f64, width: u32, height: u32) {
        // Convert screen position to math coordinates
        let (mx, my) = self.screen_to_math(cx, cy, width, height);

        // Calculate new ranges
        let x_range = self.viewport.x_max - self.viewport.x_min;
        let y_range = self.viewport.y_max - self.viewport.y_min;
        let new_x_range = x_range / factor;
        let new_y_range = y_range / factor;

        // Calculate relative position of zoom center
        let rx = (mx - self.viewport.x_min) / x_range;
        let ry = (my - self.viewport.y_min) / y_range;

        // Set new viewport centered on zoom point
        self.viewport.x_min = mx - rx * new_x_range;
        self.viewport.x_max = mx + (1.0 - rx) * new_x_range;
        self.viewport.y_min = my - ry * new_y_range;
        self.viewport.y_max = my + (1.0 - ry) * new_y_range;
    }

    /// Reset viewport to default
    pub fn reset_viewport(&mut self) {
        self.viewport = Viewport2D::default();
    }

    /// Convert screen coordinates to math coordinates
    pub fn screen_to_math(&self, sx: f64, sy: f64, width: u32, height: u32) -> (f64, f64) {
        let x_range = self.viewport.x_max - self.viewport.x_min;
        let y_range = self.viewport.y_max - self.viewport.y_min;

        let mx = self.viewport.x_min + (sx / width as f64) * x_range;
        let my = self.viewport.y_max - (sy / height as f64) * y_range;

        (mx, my)
    }

    /// Convert math coordinates to screen coordinates
    pub fn math_to_screen(&self, mx: f64, my: f64, width: u32, height: u32) -> (f64, f64) {
        let x_range = self.viewport.x_max - self.viewport.x_min;
        let y_range = self.viewport.y_max - self.viewport.y_min;

        let sx = ((mx - self.viewport.x_min) / x_range) * width as f64;
        let sy = ((self.viewport.y_max - my) / y_range) * height as f64;

        (sx, sy)
    }

    /// Update trace position
    pub fn set_trace_pos(&mut self, x: f64, y: f64) {
        self.trace_pos = Some((x, y));
    }

    /// Render the graph to a Cairo context
    pub fn render(&self, ctx: &Context, width: u32, height: u32) {
        let w = width as f64;
        let h = height as f64;

        // Background
        set_color(ctx, self.config.background);
        ctx.rectangle(0.0, 0.0, w, h);
        let _ = ctx.fill();

        // Grid
        if self.config.show_grid {
            self.draw_grid(ctx, width, height);
        }

        // Axes
        self.draw_axes(ctx, width, height);

        // Functions
        for func in &self.functions {
            self.draw_function(ctx, func, width, height);
        }

        // Trace
        if self.trace_enabled {
            if let Some((sx, sy)) = self.trace_pos {
                self.draw_trace(ctx, sx, sy, width, height);
            }
        }
    }

    fn draw_grid(&self, ctx: &Context, width: u32, height: u32) {
        set_color(ctx, self.config.grid_color);
        ctx.set_line_width(self.config.grid_width);

        let x_range = self.viewport.x_max - self.viewport.x_min;
        let y_range = self.viewport.y_max - self.viewport.y_min;

        // Calculate nice grid spacing
        let x_step = nice_step(x_range / 10.0);
        let y_step = nice_step(y_range / 10.0);

        // Vertical grid lines
        let x_start = (self.viewport.x_min / x_step).floor() * x_step;
        let mut x = x_start;
        while x <= self.viewport.x_max {
            let (sx, _) = self.math_to_screen(x, 0.0, width, height);
            ctx.move_to(sx, 0.0);
            ctx.line_to(sx, height as f64);
            x += x_step;
        }

        // Horizontal grid lines
        let y_start = (self.viewport.y_min / y_step).floor() * y_step;
        let mut y = y_start;
        while y <= self.viewport.y_max {
            let (_, sy) = self.math_to_screen(0.0, y, width, height);
            ctx.move_to(0.0, sy);
            ctx.line_to(width as f64, sy);
            y += y_step;
        }

        let _ = ctx.stroke();
    }

    fn draw_axes(&self, ctx: &Context, width: u32, height: u32) {
        set_color(ctx, self.config.axis_color);
        ctx.set_line_width(self.config.axis_width);

        // Y-axis (x = 0)
        if self.viewport.x_min <= 0.0 && self.viewport.x_max >= 0.0 {
            let (sx, _) = self.math_to_screen(0.0, 0.0, width, height);
            ctx.move_to(sx, 0.0);
            ctx.line_to(sx, height as f64);
        }

        // X-axis (y = 0)
        if self.viewport.y_min <= 0.0 && self.viewport.y_max >= 0.0 {
            let (_, sy) = self.math_to_screen(0.0, 0.0, width, height);
            ctx.move_to(0.0, sy);
            ctx.line_to(width as f64, sy);
        }

        let _ = ctx.stroke();

        // Tick labels
        if self.config.show_labels {
            self.draw_tick_labels(ctx, width, height);
        }
    }

    fn draw_tick_labels(&self, ctx: &Context, width: u32, height: u32) {
        set_color(ctx, self.config.label_color);
        ctx.set_font_size(11.0);

        let x_range = self.viewport.x_max - self.viewport.x_min;
        let y_range = self.viewport.y_max - self.viewport.y_min;

        let x_step = nice_step(x_range / 10.0);
        let y_step = nice_step(y_range / 10.0);

        // Get y-axis position for x labels
        let (_, y_axis_sy) = self.math_to_screen(0.0, 0.0, width, height);
        let label_y = y_axis_sy.clamp(12.0, height as f64 - 4.0);

        // X-axis labels
        let x_start = (self.viewport.x_min / x_step).ceil() * x_step;
        let mut x = x_start;
        while x <= self.viewport.x_max {
            if x.abs() > x_step * 0.1 {
                let (sx, _) = self.math_to_screen(x, 0.0, width, height);
                let label = format_number(x);
                ctx.move_to(sx - 10.0, label_y + 12.0);
                let _ = ctx.show_text(&label);
            }
            x += x_step;
        }

        // Get x-axis position for y labels
        let (x_axis_sx, _) = self.math_to_screen(0.0, 0.0, width, height);
        let label_x = x_axis_sx.clamp(4.0, width as f64 - 40.0);

        // Y-axis labels
        let y_start = (self.viewport.y_min / y_step).ceil() * y_step;
        let mut y = y_start;
        while y <= self.viewport.y_max {
            if y.abs() > y_step * 0.1 {
                let (_, sy) = self.math_to_screen(0.0, y, width, height);
                let label = format_number(y);
                ctx.move_to(label_x + 4.0, sy + 4.0);
                let _ = ctx.show_text(&label);
            }
            y += y_step;
        }
    }

    fn draw_function(&self, ctx: &Context, func: &Plottable, width: u32, height: u32) {
        match func {
            Plottable::Explicit2D {
                expr,
                x_var,
                color,
                style,
            } => {
                self.draw_explicit(ctx, expr, x_var, *color, *style, width, height);
            }
            Plottable::Implicit2D {
                expr,
                x_var,
                y_var,
                color,
            } => {
                self.draw_implicit(ctx, expr, x_var, y_var, *color, width, height);
            }
            Plottable::Parametric2D {
                x_expr,
                y_expr,
                t_var,
                t_range,
                color,
            } => {
                self.draw_parametric(ctx, x_expr, y_expr, t_var, *t_range, *color, width, height);
            }
            _ => {}
        }
    }

    fn draw_explicit(
        &self,
        ctx: &Context,
        expr: &Expr,
        x_var: &str,
        color: Color,
        style: LineStyle,
        width: u32,
        height: u32,
    ) {
        set_color(ctx, color);
        ctx.set_line_width(self.config.curve_width);

        match style {
            LineStyle::Solid => ctx.set_dash(&[], 0.0),
            LineStyle::Dashed => ctx.set_dash(&[6.0, 4.0], 0.0),
            LineStyle::Dotted => ctx.set_dash(&[2.0, 2.0], 0.0),
        }

        let mut evaluator = Evaluator::new();
        let _var = Symbol::from(x_var);

        // Sample the function
        let num_samples = (width as f64 * self.config.samples_per_pixel) as usize;
        let x_range = self.viewport.x_max - self.viewport.x_min;
        let dx = x_range / num_samples as f64;

        let mut first = true;
        let mut last_valid = false;

        for i in 0..=num_samples {
            let math_x = self.viewport.x_min + i as f64 * dx;
            evaluator.set_var(x_var, Expr::Float(math_x));

            if let Ok(result) = evaluator.eval(expr) {
                if let Ok(math_y) = expr_to_f64(&result) {
                    if math_y.is_finite()
                        && math_y >= self.viewport.y_min - x_range
                        && math_y <= self.viewport.y_max + x_range
                    {
                        let (sx, sy) = self.math_to_screen(math_x, math_y, width, height);

                        if first || !last_valid {
                            ctx.move_to(sx, sy);
                            first = false;
                        } else {
                            ctx.line_to(sx, sy);
                        }
                        last_valid = true;
                        continue;
                    }
                }
            }
            // Invalid point - break the line
            if last_valid {
                let _ = ctx.stroke();
            }
            last_valid = false;
        }

        let _ = ctx.stroke();
        ctx.set_dash(&[], 0.0);
    }

    fn draw_parametric(
        &self,
        ctx: &Context,
        x_expr: &Expr,
        y_expr: &Expr,
        t_var: &str,
        t_range: (f64, f64),
        color: Color,
        width: u32,
        height: u32,
    ) {
        set_color(ctx, color);
        ctx.set_line_width(self.config.curve_width);

        let mut evaluator = Evaluator::new();

        let num_samples = (width as f64 * self.config.samples_per_pixel) as usize;
        let t_span = t_range.1 - t_range.0;
        let dt = t_span / num_samples as f64;

        let mut first = true;

        for i in 0..=num_samples {
            let t = t_range.0 + i as f64 * dt;
            evaluator.set_var(t_var, Expr::Float(t));

            let x_result = evaluator.eval(x_expr);
            let y_result = evaluator.eval(y_expr);

            if let (Ok(x_val), Ok(y_val)) = (x_result, y_result) {
                if let (Ok(math_x), Ok(math_y)) = (expr_to_f64(&x_val), expr_to_f64(&y_val)) {
                    if math_x.is_finite() && math_y.is_finite() {
                        let (sx, sy) = self.math_to_screen(math_x, math_y, width, height);

                        if first {
                            ctx.move_to(sx, sy);
                            first = false;
                        } else {
                            ctx.line_to(sx, sy);
                        }
                    }
                }
            }
        }

        let _ = ctx.stroke();
    }

    /// Draw implicit curve F(x,y) = 0 using marching squares
    fn draw_implicit(
        &self,
        ctx: &Context,
        expr: &Expr,
        x_var: &str,
        y_var: &str,
        color: Color,
        width: u32,
        height: u32,
    ) {
        set_color(ctx, color);
        ctx.set_line_width(self.config.curve_width);

        let mut evaluator = Evaluator::new();

        // Grid resolution for marching squares
        let grid_size = 100; // Number of cells per dimension
        let x_range = self.viewport.x_max - self.viewport.x_min;
        let y_range = self.viewport.y_max - self.viewport.y_min;
        let dx = x_range / grid_size as f64;
        let dy = y_range / grid_size as f64;

        // Evaluate F(x,y) at grid points
        let mut values = vec![vec![0.0f64; grid_size + 1]; grid_size + 1];
        for i in 0..=grid_size {
            let x = self.viewport.x_min + i as f64 * dx;
            for j in 0..=grid_size {
                let y = self.viewport.y_min + j as f64 * dy;
                evaluator.set_var(x_var, Expr::Float(x));
                evaluator.set_var(y_var, Expr::Float(y));
                if let Ok(result) = evaluator.eval(expr) {
                    if let Ok(v) = expr_to_f64(&result) {
                        values[i][j] = if v.is_finite() { v } else { f64::NAN };
                    } else {
                        values[i][j] = f64::NAN;
                    }
                } else {
                    values[i][j] = f64::NAN;
                }
            }
        }

        // Marching squares: for each cell, draw line segments where F=0
        for i in 0..grid_size {
            for j in 0..grid_size {
                let x0 = self.viewport.x_min + i as f64 * dx;
                let y0 = self.viewport.y_min + j as f64 * dy;
                let x1 = x0 + dx;
                let y1 = y0 + dy;

                let v00 = values[i][j];
                let v10 = values[i + 1][j];
                let v01 = values[i][j + 1];
                let v11 = values[i + 1][j + 1];

                // Skip cells with NaN
                if v00.is_nan() || v10.is_nan() || v01.is_nan() || v11.is_nan() {
                    continue;
                }

                // Classify cell corners by sign
                let s00 = v00 >= 0.0;
                let s10 = v10 >= 0.0;
                let s01 = v01 >= 0.0;
                let s11 = v11 >= 0.0;

                // Build case index (4-bit)
                let case =
                    (s00 as u8) | ((s10 as u8) << 1) | ((s01 as u8) << 2) | ((s11 as u8) << 3);

                // Linear interpolation to find zero crossing on an edge
                let interp = |va: f64, vb: f64| -> f64 {
                    if (va - vb).abs() < 1e-15 {
                        0.5
                    } else {
                        va / (va - vb)
                    }
                };

                // Edge midpoints where contour crosses
                let e_bottom = || {
                    let t = interp(v00, v10);
                    (x0 + t * dx, y0)
                };
                let e_top = || {
                    let t = interp(v01, v11);
                    (x0 + t * dx, y1)
                };
                let e_left = || {
                    let t = interp(v00, v01);
                    (x0, y0 + t * dy)
                };
                let e_right = || {
                    let t = interp(v10, v11);
                    (x1, y0 + t * dy)
                };

                // Draw line segments based on marching squares case
                let draw_line = |p1: (f64, f64), p2: (f64, f64)| {
                    let (sx1, sy1) = self.math_to_screen(p1.0, p1.1, width, height);
                    let (sx2, sy2) = self.math_to_screen(p2.0, p2.1, width, height);
                    ctx.move_to(sx1, sy1);
                    ctx.line_to(sx2, sy2);
                };

                match case {
                    0 | 15 => {} // All same sign - no contour
                    1 | 14 => draw_line(e_bottom(), e_left()),
                    2 | 13 => draw_line(e_bottom(), e_right()),
                    3 | 12 => draw_line(e_left(), e_right()),
                    4 | 11 => draw_line(e_left(), e_top()),
                    5 | 10 => {
                        // Ambiguous - saddle point. Draw both segments.
                        draw_line(e_bottom(), e_left());
                        draw_line(e_top(), e_right());
                    }
                    6 | 9 => {
                        draw_line(e_bottom(), e_top());
                    }
                    7 | 8 => draw_line(e_top(), e_right()),
                    _ => {}
                }
            }
        }

        let _ = ctx.stroke();
    }

    fn draw_trace(&self, ctx: &Context, sx: f64, sy: f64, width: u32, height: u32) {
        let (mx, my) = self.screen_to_math(sx, sy, width, height);

        // Crosshair
        set_color(ctx, self.config.axis_color);
        ctx.set_line_width(0.5);
        ctx.set_dash(&[4.0, 4.0], 0.0);

        ctx.move_to(sx, 0.0);
        ctx.line_to(sx, height as f64);
        ctx.move_to(0.0, sy);
        ctx.line_to(width as f64, sy);
        let _ = ctx.stroke();
        ctx.set_dash(&[], 0.0);

        // Coordinate display
        set_color(
            ctx,
            Color {
                r: 30,
                g: 30,
                b: 46,
                a: 200,
            },
        );
        let label = format!("({:.4}, {:.4})", mx, my);
        let label_w = label.len() as f64 * 7.0 + 8.0;
        let label_h = 18.0;
        let lx = (sx + 10.0).min(width as f64 - label_w - 4.0);
        let ly = (sy - 24.0).max(4.0);

        ctx.rectangle(lx, ly, label_w, label_h);
        let _ = ctx.fill();

        set_color(ctx, self.config.label_color);
        ctx.set_font_size(12.0);
        ctx.move_to(lx + 4.0, ly + 13.0);
        let _ = ctx.show_text(&label);
    }
}

/// Set Cairo source color
fn set_color(ctx: &Context, color: Color) {
    ctx.set_source_rgba(
        color.r as f64 / 255.0,
        color.g as f64 / 255.0,
        color.b as f64 / 255.0,
        color.a as f64 / 255.0,
    );
}

/// Calculate a nice step size for grid/ticks
fn nice_step(rough: f64) -> f64 {
    let exp = rough.abs().log10().floor();
    let frac = rough / 10f64.powf(exp);

    let nice = if frac < 1.5 {
        1.0
    } else if frac <= 3.0 {
        2.0
    } else if frac < 7.0 {
        5.0
    } else {
        10.0
    };

    nice * 10f64.powf(exp)
}

/// Format a number for axis labels
fn format_number(x: f64) -> String {
    if x == 0.0 {
        return "0".to_string();
    }
    let abs = x.abs();
    if abs >= 1000.0 || abs < 0.01 {
        format!("{:.1e}", x)
    } else if abs >= 1.0 {
        format!("{:.1}", x)
    } else {
        format!("{:.2}", x)
    }
}

/// Convert an Expr to f64
fn expr_to_f64(expr: &Expr) -> Result<f64, ()> {
    match expr {
        Expr::Integer(n) => Ok(*n as f64),
        Expr::Float(x) => Ok(*x),
        Expr::Rational(r) => Ok(r.to_f64()),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinate_transform() {
        let graph = Graph2D::new();
        let (mx, my) = graph.screen_to_math(250.0, 250.0, 500, 500);
        assert!((mx - 0.0).abs() < 0.01);
        assert!((my - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_nice_step() {
        assert_eq!(nice_step(0.03), 0.02);
        assert_eq!(nice_step(0.3), 0.2);
        assert_eq!(nice_step(3.0), 2.0);
        assert_eq!(nice_step(30.0), 20.0);
    }
}
