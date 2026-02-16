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

/// Viewport preset configurations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportPreset {
    Standard,
    Trig,
    ZoomFit,
}

/// A plotted function with visibility and label
#[derive(Debug, Clone)]
pub struct PlottedFunction {
    pub func: Plottable,
    pub visible: bool,
    pub label: String,
}

/// 2D graph state and renderer
pub struct Graph2D {
    /// Current viewport
    pub viewport: Viewport2D,
    /// Plot configuration
    pub config: PlotConfig,
    /// Functions to plot
    pub functions: Vec<PlottedFunction>,
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
    pub fn add_function(&mut self, func: Plottable, label: String) {
        self.functions.push(PlottedFunction {
            func,
            visible: true,
            label,
        });
    }

    /// Add an explicit function y = f(x) with auto color
    pub fn add_explicit(&mut self, expr: Expr) {
        let color = CURVE_COLORS[self.functions.len() % CURVE_COLORS.len()];
        let label = format!("y = {}", expr);
        self.functions.push(PlottedFunction {
            func: Plottable::Explicit2D {
                expr,
                x_var: "x".to_string(),
                color,
                style: LineStyle::Solid,
            },
            visible: true,
            label,
        });
    }

    /// Add an implicit curve F(x,y) = 0
    pub fn add_implicit(&mut self, expr: Expr) {
        let color = CURVE_COLORS[self.functions.len() % CURVE_COLORS.len()];
        let label = format!("{} = 0", expr);
        self.functions.push(PlottedFunction {
            func: Plottable::Implicit2D {
                expr,
                x_var: "x".to_string(),
                y_var: "y".to_string(),
                color,
            },
            visible: true,
            label,
        });
    }

    /// Add a parametric curve (x(t), y(t))
    pub fn add_parametric(&mut self, x_expr: Expr, y_expr: Expr, t_range: (f64, f64)) {
        let color = CURVE_COLORS[self.functions.len() % CURVE_COLORS.len()];
        let label = format!("({}, {})", x_expr, y_expr);
        self.functions.push(PlottedFunction {
            func: Plottable::Parametric2D {
                x_expr,
                y_expr,
                t_var: "t".to_string(),
                t_range,
                color,
            },
            visible: true,
            label,
        });
    }

    /// Add a polar curve r = f(theta)
    pub fn add_polar(&mut self, expr: Expr, theta_range: (f64, f64)) {
        let color = CURVE_COLORS[self.functions.len() % CURVE_COLORS.len()];
        let label = format!("r = {}", expr);
        self.functions.push(PlottedFunction {
            func: Plottable::Polar2D {
                expr,
                theta_var: "theta".to_string(),
                theta_range,
                color,
                style: LineStyle::Solid,
            },
            visible: true,
            label,
        });
    }

    /// Add a piecewise function
    pub fn add_piecewise(&mut self, pieces: Vec<crate::PiecewisePiece>) {
        let color = CURVE_COLORS[self.functions.len() % CURVE_COLORS.len()];
        let label = format!("piecewise ({} pieces)", pieces.len());
        self.functions.push(PlottedFunction {
            func: Plottable::Piecewise2D {
                pieces,
                x_var: "x".to_string(),
                color,
                style: LineStyle::Solid,
            },
            visible: true,
            label,
        });
    }

    /// Apply a viewport preset
    pub fn apply_viewport_preset(&mut self, preset: ViewportPreset) {
        match preset {
            ViewportPreset::Standard => {
                self.viewport = Viewport2D::default();
            }
            ViewportPreset::Trig => {
                self.viewport = Viewport2D {
                    x_min: -2.0 * std::f64::consts::PI,
                    x_max: 2.0 * std::f64::consts::PI,
                    y_min: -1.5,
                    y_max: 1.5,
                };
            }
            ViewportPreset::ZoomFit => {
                self.zoom_to_fit();
            }
        }
    }

    /// Auto-zoom to fit visible function outputs
    pub fn zoom_to_fit(&mut self) {
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        let mut evaluator = Evaluator::new();
        let num_samples = 500;
        let x_range = self.viewport.x_max - self.viewport.x_min;
        let dx = x_range / num_samples as f64;

        for pf in &self.functions {
            if !pf.visible {
                continue;
            }
            if let Plottable::Explicit2D { ref expr, ref x_var, .. } = pf.func {
                for i in 0..=num_samples {
                    let x = self.viewport.x_min + i as f64 * dx;
                    evaluator.set_var(x_var, Expr::Float(x));
                    if let Ok(result) = evaluator.eval(expr) {
                        if let Ok(y) = expr_to_f64(&result) {
                            if y.is_finite() {
                                y_min = y_min.min(y);
                                y_max = y_max.max(y);
                            }
                        }
                    }
                }
            }
        }

        if y_min.is_finite() && y_max.is_finite() && (y_max - y_min).abs() > 1e-10 {
            let margin = (y_max - y_min) * 0.1;
            self.viewport.y_min = y_min - margin;
            self.viewport.y_max = y_max + margin;
        }
    }

    /// Set the color of a function at the given index
    pub fn set_function_color(&mut self, index: usize, color: crate::Color) {
        if let Some(pf) = self.functions.get_mut(index) {
            pf.func.set_color(color);
        }
    }

    /// Toggle visibility of function at index
    pub fn toggle_visibility(&mut self, index: usize) {
        if let Some(f) = self.functions.get_mut(index) {
            f.visible = !f.visible;
        }
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
        for pf in &self.functions {
            if pf.visible {
                self.draw_function(ctx, &pf.func, width, height);
            }
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
            Plottable::Polar2D {
                expr,
                theta_var,
                theta_range,
                color,
                style,
            } => {
                self.draw_polar(ctx, expr, theta_var, *theta_range, *color, *style, width, height);
            }
            Plottable::Piecewise2D {
                pieces,
                x_var,
                color,
                style,
            } => {
                self.draw_piecewise(ctx, pieces, x_var, *color, *style, width, height);
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

    fn draw_polar(
        &self,
        ctx: &Context,
        expr: &Expr,
        theta_var: &str,
        theta_range: (f64, f64),
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
        let num_samples = (width as f64 * self.config.samples_per_pixel) as usize;
        let theta_span = theta_range.1 - theta_range.0;
        let dtheta = theta_span / num_samples as f64;
        let mut first = true;

        for i in 0..=num_samples {
            let theta = theta_range.0 + i as f64 * dtheta;
            evaluator.set_var(theta_var, Expr::Float(theta));
            if let Ok(result) = evaluator.eval(expr) {
                if let Ok(r) = expr_to_f64(&result) {
                    if r.is_finite() {
                        let math_x = r * theta.cos();
                        let math_y = r * theta.sin();
                        let (sx, sy) = self.math_to_screen(math_x, math_y, width, height);
                        if first {
                            ctx.move_to(sx, sy);
                            first = false;
                        } else {
                            ctx.line_to(sx, sy);
                        }
                        continue;
                    }
                }
            }
            if !first {
                let _ = ctx.stroke();
                first = true;
            }
        }
        let _ = ctx.stroke();
        ctx.set_dash(&[], 0.0);
    }

    fn draw_piecewise(
        &self,
        ctx: &Context,
        pieces: &[crate::PiecewisePiece],
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
        let num_samples = (width as f64 * self.config.samples_per_pixel) as usize;
        let x_range = self.viewport.x_max - self.viewport.x_min;
        let dx = x_range / num_samples as f64;
        let mut first = true;
        let mut last_valid = false;
        let mut last_piece_idx: Option<usize> = None;

        for i in 0..=num_samples {
            let math_x = self.viewport.x_min + i as f64 * dx;
            let piece_idx = pieces.iter().position(|p| p.condition.matches(math_x));

            if piece_idx != last_piece_idx && last_valid {
                let _ = ctx.stroke();
                first = true;
                last_valid = false;
            }
            last_piece_idx = piece_idx;

            if let Some(idx) = piece_idx {
                evaluator.set_var(x_var, Expr::Float(math_x));
                if let Ok(result) = evaluator.eval(&pieces[idx].expr) {
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
            }
            if last_valid {
                let _ = ctx.stroke();
            }
            last_valid = false;
        }
        let _ = ctx.stroke();
        ctx.set_dash(&[], 0.0);
    }

    /// Find zeros (x-intercepts) of all visible Explicit2D functions
    pub fn find_zeros(&self) -> Vec<(usize, Vec<(f64, f64)>)> {
        let mut result = Vec::new();
        for (idx, pf) in self.functions.iter().enumerate() {
            if !pf.visible {
                continue;
            }
            if let Plottable::Explicit2D { ref expr, ref x_var, .. } = pf.func {
                let zeros = self.find_zeros_for_expr(expr, x_var);
                if !zeros.is_empty() {
                    result.push((idx, zeros));
                }
            }
        }
        result
    }

    fn find_zeros_for_expr(&self, expr: &Expr, x_var: &str) -> Vec<(f64, f64)> {
        let mut evaluator = Evaluator::new();
        let mut zeros = Vec::new();
        let num_samples = 500;
        let x_range = self.viewport.x_max - self.viewport.x_min;
        let dx = x_range / num_samples as f64;

        let eval_at = |eval: &mut Evaluator, x: f64| -> Option<f64> {
            eval.set_var(x_var, Expr::Float(x));
            eval.eval(expr)
                .ok()
                .and_then(|r| expr_to_f64(&r).ok())
                .filter(|y| y.is_finite())
        };

        let mut prev: Option<f64> = None;
        let mut prev_x = self.viewport.x_min;

        for i in 0..=num_samples {
            let x = self.viewport.x_min + i as f64 * dx;
            if let Some(y) = eval_at(&mut evaluator, x) {
                if let Some(py) = prev {
                    if py * y < 0.0 {
                        if let Some(zx) = self.bisect(expr, x_var, prev_x, x, 50) {
                            if zeros
                                .last()
                                .map_or(true, |&(lx, _): &(f64, f64)| (lx - zx).abs() > dx * 0.1)
                            {
                                zeros.push((zx, 0.0));
                            }
                        }
                    }
                }
                prev = Some(y);
                prev_x = x;
            } else {
                prev = None;
            }
        }
        zeros
    }

    fn bisect(
        &self,
        expr: &Expr,
        x_var: &str,
        mut a: f64,
        mut b: f64,
        max_iter: usize,
    ) -> Option<f64> {
        let mut evaluator = Evaluator::new();
        let eval_at = |eval: &mut Evaluator, x: f64| -> Option<f64> {
            eval.set_var(x_var, Expr::Float(x));
            eval.eval(expr)
                .ok()
                .and_then(|r| expr_to_f64(&r).ok())
                .filter(|y| y.is_finite())
        };

        let fa = eval_at(&mut evaluator, a)?;
        if fa.abs() < 1e-12 {
            return Some(a);
        }

        for _ in 0..max_iter {
            let mid = (a + b) / 2.0;
            let fm = eval_at(&mut evaluator, mid)?;
            if fm.abs() < 1e-12 || (b - a).abs() < 1e-12 {
                return Some(mid);
            }
            if fa * fm < 0.0 {
                b = mid;
            } else {
                a = mid;
            }
        }
        Some((a + b) / 2.0)
    }

    /// Find intersections between pairs of visible Explicit2D functions
    pub fn find_intersections(&self) -> Vec<((usize, usize), Vec<(f64, f64)>)> {
        let mut result = Vec::new();
        let explicit_indices: Vec<usize> = self
            .functions
            .iter()
            .enumerate()
            .filter(|(_, pf)| pf.visible && matches!(pf.func, Plottable::Explicit2D { .. }))
            .map(|(i, _)| i)
            .collect();

        for i in 0..explicit_indices.len() {
            for j in (i + 1)..explicit_indices.len() {
                let idx_i = explicit_indices[i];
                let idx_j = explicit_indices[j];
                let points = self.find_intersection_points(idx_i, idx_j);
                if !points.is_empty() {
                    result.push(((idx_i, idx_j), points));
                }
            }
        }
        result
    }

    fn find_intersection_points(&self, idx_i: usize, idx_j: usize) -> Vec<(f64, f64)> {
        let (expr_i, var_i) = match &self.functions[idx_i].func {
            Plottable::Explicit2D { expr, x_var, .. } => (expr, x_var.as_str()),
            _ => return Vec::new(),
        };
        let (expr_j, _) = match &self.functions[idx_j].func {
            Plottable::Explicit2D { expr, x_var, .. } => (expr, x_var.as_str()),
            _ => return Vec::new(),
        };

        let mut eval_i = Evaluator::new();
        let mut eval_j = Evaluator::new();
        let mut points = Vec::new();
        let num_samples = 500;
        let x_range = self.viewport.x_max - self.viewport.x_min;
        let dx = x_range / num_samples as f64;

        let eval_diff = |ei: &mut Evaluator, ej: &mut Evaluator, x: f64| -> Option<f64> {
            ei.set_var(var_i, Expr::Float(x));
            ej.set_var(var_i, Expr::Float(x));
            let yi = ei.eval(expr_i).ok().and_then(|r| expr_to_f64(&r).ok())?;
            let yj = ej.eval(expr_j).ok().and_then(|r| expr_to_f64(&r).ok())?;
            if yi.is_finite() && yj.is_finite() {
                Some(yi - yj)
            } else {
                None
            }
        };

        let mut prev_diff: Option<f64> = None;
        let mut prev_x = self.viewport.x_min;

        for i in 0..=num_samples {
            let x = self.viewport.x_min + i as f64 * dx;
            if let Some(diff) = eval_diff(&mut eval_i, &mut eval_j, x) {
                if let Some(pd) = prev_diff {
                    if pd * diff < 0.0 {
                        let mut a = prev_x;
                        let mut b = x;
                        for _ in 0..50 {
                            let mid = (a + b) / 2.0;
                            if let Some(dm) = eval_diff(&mut eval_i, &mut eval_j, mid) {
                                if dm.abs() < 1e-12 || (b - a).abs() < 1e-12 {
                                    a = mid;
                                    b = mid;
                                    break;
                                }
                                if pd * dm < 0.0 {
                                    b = mid;
                                } else {
                                    a = mid;
                                }
                            } else {
                                break;
                            }
                        }
                        let zx = (a + b) / 2.0;
                        eval_i.set_var(var_i, Expr::Float(zx));
                        if let Ok(r) = eval_i.eval(expr_i) {
                            if let Ok(zy) = expr_to_f64(&r) {
                                if zy.is_finite()
                                    && points.last().map_or(true, |&(lx, _): &(f64, f64)| {
                                        (lx - zx).abs() > dx * 0.1
                                    })
                                {
                                    points.push((zx, zy));
                                }
                            }
                        }
                    }
                }
                prev_diff = Some(diff);
                prev_x = x;
            } else {
                prev_diff = None;
            }
        }
        points
    }

    /// Draw marker circles at given points
    pub fn draw_markers(
        &self,
        ctx: &Context,
        points: &[(f64, f64)],
        color: Color,
        width: u32,
        height: u32,
    ) {
        set_color(ctx, color);
        for &(mx, my) in points {
            let (sx, sy) = self.math_to_screen(mx, my, width, height);
            ctx.arc(sx, sy, 4.0, 0.0, std::f64::consts::TAU);
            let _ = ctx.fill();

            let label = format!("({:.3}, {:.3})", mx, my);
            ctx.set_font_size(10.0);
            ctx.move_to(sx + 6.0, sy - 6.0);
            let _ = ctx.show_text(&label);
        }
    }

    /// Generate a table of (x, f(x)) values for an Explicit2D function
    pub fn generate_table(
        &self,
        func_index: usize,
        x_min: f64,
        x_max: f64,
        step: f64,
    ) -> Vec<(f64, Option<f64>)> {
        let pf = match self.functions.get(func_index) {
            Some(f) => f,
            None => return Vec::new(),
        };
        let (expr, x_var) = match &pf.func {
            Plottable::Explicit2D { expr, x_var, .. } => (expr, x_var.as_str()),
            _ => return Vec::new(),
        };

        let mut evaluator = Evaluator::new();
        let mut table = Vec::new();
        let mut x = x_min;
        while x <= x_max + step * 0.01 {
            evaluator.set_var(x_var, Expr::Float(x));
            let y = evaluator
                .eval(expr)
                .ok()
                .and_then(|r| expr_to_f64(&r).ok())
                .filter(|v| v.is_finite());
            table.push((x, y));
            x += step;
        }
        table
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
