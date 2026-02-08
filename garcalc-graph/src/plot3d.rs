//! 3D plotting engine
//!
//! Provides surface plotting with:
//! - Explicit surfaces: z = f(x, y)
//! - Parametric surfaces: (x(u,v), y(u,v), z(u,v))
//! - Wireframe and filled rendering
//! - Height-based colormaps
//! - Rotation and zoom

use cairo::Context;
use garcalc_cas::{Evaluator, Expr};

use crate::Color;

/// 3D camera state (spherical coordinates around target)
#[derive(Debug, Clone, Copy)]
pub struct Camera3D {
    /// Horizontal angle (radians)
    pub azimuth: f64,
    /// Vertical angle (radians)
    pub elevation: f64,
    /// Distance from target
    pub distance: f64,
    /// Look-at point
    pub target: (f64, f64, f64),
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            azimuth: 45.0_f64.to_radians(),
            elevation: 30.0_f64.to_radians(),
            distance: 25.0,
            target: (0.0, 0.0, 0.0),
        }
    }
}

impl Camera3D {
    /// Rotate the camera by delta angles
    pub fn rotate(&mut self, d_azimuth: f64, d_elevation: f64) {
        self.azimuth += d_azimuth;
        self.elevation = (self.elevation + d_elevation)
            .clamp(-89.0_f64.to_radians(), 89.0_f64.to_radians());
    }

    /// Zoom by a factor
    pub fn zoom(&mut self, factor: f64) {
        self.distance = (self.distance / factor).clamp(0.5, 2000.0);
    }

    /// Get camera position in world coordinates
    pub fn position(&self) -> (f64, f64, f64) {
        let cos_elev = self.elevation.cos();
        let sin_elev = self.elevation.sin();
        let cos_azim = self.azimuth.cos();
        let sin_azim = self.azimuth.sin();

        (
            self.target.0 + self.distance * cos_elev * cos_azim,
            self.target.1 + self.distance * cos_elev * sin_azim,
            self.target.2 + self.distance * sin_elev,
        )
    }
}

/// 3D viewport bounds
#[derive(Debug, Clone, Copy)]
pub struct Viewport3D {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub z_min: f64,
    pub z_max: f64,
}

impl Default for Viewport3D {
    fn default() -> Self {
        Self {
            x_min: -5.0,
            x_max: 5.0,
            y_min: -5.0,
            y_max: 5.0,
            z_min: -5.0,
            z_max: 5.0,
        }
    }
}

/// Colormap for surface coloring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Colormap {
    Viridis,
    Plasma,
    Coolwarm,
    Grayscale,
}

impl Colormap {
    /// Map a value in [0, 1] to a color
    pub fn color(&self, t: f64) -> Color {
        let t = t.clamp(0.0, 1.0);
        match self {
            Colormap::Viridis => viridis(t),
            Colormap::Plasma => plasma(t),
            Colormap::Coolwarm => coolwarm(t),
            Colormap::Grayscale => {
                let v = (t * 255.0) as u8;
                Color { r: v, g: v, b: v, a: 255 }
            }
        }
    }
}

/// A 3D plottable surface
#[derive(Debug, Clone)]
pub enum Surface3D {
    /// z = f(x, y)
    Explicit {
        expr: Expr,
        x_var: String,
        y_var: String,
    },
    /// Parametric: (x(u,v), y(u,v), z(u,v))
    Parametric {
        x_expr: Expr,
        y_expr: Expr,
        z_expr: Expr,
        u_var: String,
        v_var: String,
        u_range: (f64, f64),
        v_range: (f64, f64),
    },
}

/// Render mode for surfaces
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Wireframe,
    Filled,
    FilledWithWireframe,
}

/// Configuration for 3D plot appearance
#[derive(Debug, Clone)]
pub struct Plot3DConfig {
    pub background: Color,
    pub axis_color: Color,
    pub wireframe_color: Color,
    pub colormap: Colormap,
    pub render_mode: RenderMode,
    pub grid_lines: usize,
    pub show_axes: bool,
    pub show_labels: bool,
}

impl Default for Plot3DConfig {
    fn default() -> Self {
        Self {
            background: Color { r: 30, g: 30, b: 46, a: 255 },
            axis_color: Color { r: 166, g: 173, b: 200, a: 255 },
            wireframe_color: Color { r: 100, g: 100, b: 120, a: 255 },
            colormap: Colormap::Viridis,
            render_mode: RenderMode::FilledWithWireframe,
            grid_lines: 40,
            show_axes: true,
            show_labels: true,
        }
    }
}

/// 3D graph state and renderer
pub struct Graph3D {
    pub camera: Camera3D,
    pub viewport: Viewport3D,
    pub config: Plot3DConfig,
    pub surfaces: Vec<Surface3D>,
}

impl Default for Graph3D {
    fn default() -> Self {
        Self {
            camera: Camera3D::default(),
            viewport: Viewport3D::default(),
            config: Plot3DConfig::default(),
            surfaces: Vec::new(),
        }
    }
}

impl Graph3D {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an explicit surface z = f(x, y)
    pub fn add_explicit(&mut self, expr: Expr) {
        self.surfaces.push(Surface3D::Explicit {
            expr,
            x_var: "x".to_string(),
            y_var: "y".to_string(),
        });
    }

    /// Clear all surfaces
    pub fn clear_surfaces(&mut self) {
        self.surfaces.clear();
    }

    /// Reset camera to default
    pub fn reset_camera(&mut self) {
        self.camera = Camera3D::default();
    }

    /// Project 3D point to 2D screen coordinates
    fn project(&self, x: f64, y: f64, z: f64, width: u32, height: u32) -> (f64, f64) {
        let cam_pos = self.camera.position();

        // View direction (camera looks at target)
        let dx = self.camera.target.0 - cam_pos.0;
        let dy = self.camera.target.1 - cam_pos.1;
        let dz = self.camera.target.2 - cam_pos.2;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

        // Normalize
        let forward = (dx / dist, dy / dist, dz / dist);

        // Up vector (world Z)
        let world_up = (0.0, 0.0, 1.0);

        // Right vector = forward x up
        let right = (
            forward.1 * world_up.2 - forward.2 * world_up.1,
            forward.2 * world_up.0 - forward.0 * world_up.2,
            forward.0 * world_up.1 - forward.1 * world_up.0,
        );
        let right_len = (right.0 * right.0 + right.1 * right.1 + right.2 * right.2).sqrt();
        let right = (right.0 / right_len, right.1 / right_len, right.2 / right_len);

        // Actual up = right x forward
        let up = (
            right.1 * forward.2 - right.2 * forward.1,
            right.2 * forward.0 - right.0 * forward.2,
            right.0 * forward.1 - right.1 * forward.0,
        );

        // Point relative to camera
        let px = x - cam_pos.0;
        let py = y - cam_pos.1;
        let pz = z - cam_pos.2;

        // Project onto camera plane
        let cam_x = px * right.0 + py * right.1 + pz * right.2;
        let cam_y = px * up.0 + py * up.1 + pz * up.2;
        let cam_z = px * forward.0 + py * forward.1 + pz * forward.2;

        // Perspective projection
        let scale = if cam_z > 0.1 {
            self.camera.distance / cam_z
        } else {
            self.camera.distance / 0.1
        };

        // Screen coordinates
        let aspect = width as f64 / height as f64;
        let fov_scale = 0.8;
        let sx = width as f64 / 2.0 + cam_x * scale * height as f64 * fov_scale / aspect;
        let sy = height as f64 / 2.0 - cam_y * scale * height as f64 * fov_scale;

        (sx, sy)
    }

    /// Render the 3D graph to a Cairo context
    pub fn render(&self, ctx: &Context, width: u32, height: u32) {
        // Background
        set_color(ctx, self.config.background);
        ctx.rectangle(0.0, 0.0, width as f64, height as f64);
        let _ = ctx.fill();

        // Draw axes
        if self.config.show_axes {
            self.draw_axes(ctx, width, height);
        }

        // Draw surfaces
        for surface in &self.surfaces {
            self.draw_surface(ctx, surface, width, height);
        }
    }

    fn draw_axes(&self, ctx: &Context, width: u32, height: u32) {
        set_color(ctx, self.config.axis_color);
        ctx.set_line_width(1.5);

        let len = 6.0;

        // X axis (red tint)
        ctx.set_source_rgba(0.9, 0.4, 0.4, 1.0);
        let (x0, y0) = self.project(0.0, 0.0, 0.0, width, height);
        let (x1, y1) = self.project(len, 0.0, 0.0, width, height);
        ctx.move_to(x0, y0);
        ctx.line_to(x1, y1);
        let _ = ctx.stroke();

        // Y axis (green tint)
        ctx.set_source_rgba(0.4, 0.9, 0.4, 1.0);
        let (x1, y1) = self.project(0.0, len, 0.0, width, height);
        ctx.move_to(x0, y0);
        ctx.line_to(x1, y1);
        let _ = ctx.stroke();

        // Z axis (blue tint)
        ctx.set_source_rgba(0.4, 0.4, 0.9, 1.0);
        let (x1, y1) = self.project(0.0, 0.0, len, width, height);
        ctx.move_to(x0, y0);
        ctx.line_to(x1, y1);
        let _ = ctx.stroke();

        // Labels
        if self.config.show_labels {
            set_color(ctx, self.config.axis_color);
            ctx.set_font_size(12.0);

            let (lx, ly) = self.project(len + 0.5, 0.0, 0.0, width, height);
            ctx.move_to(lx, ly);
            let _ = ctx.show_text("X");

            let (lx, ly) = self.project(0.0, len + 0.5, 0.0, width, height);
            ctx.move_to(lx, ly);
            let _ = ctx.show_text("Y");

            let (lx, ly) = self.project(0.0, 0.0, len + 0.5, width, height);
            ctx.move_to(lx, ly);
            let _ = ctx.show_text("Z");
        }
    }

    fn draw_surface(&self, ctx: &Context, surface: &Surface3D, width: u32, height: u32) {
        match surface {
            Surface3D::Explicit { expr, x_var, y_var } => {
                self.draw_explicit_surface(ctx, expr, x_var, y_var, width, height);
            }
            Surface3D::Parametric { x_expr, y_expr, z_expr, u_var, v_var, u_range, v_range } => {
                self.draw_parametric_surface(
                    ctx, x_expr, y_expr, z_expr, u_var, v_var, *u_range, *v_range, width, height
                );
            }
        }
    }

    fn draw_explicit_surface(
        &self,
        ctx: &Context,
        expr: &Expr,
        x_var: &str,
        y_var: &str,
        width: u32,
        height: u32,
    ) {
        let mut evaluator = Evaluator::new();
        let n = self.config.grid_lines;

        let x_range = self.viewport.x_max - self.viewport.x_min;
        let y_range = self.viewport.y_max - self.viewport.y_min;
        let dx = x_range / n as f64;
        let dy = y_range / n as f64;

        // Sample the surface
        let mut points: Vec<Vec<Option<(f64, f64, f64)>>> = Vec::with_capacity(n + 1);
        let mut z_min = f64::INFINITY;
        let mut z_max = f64::NEG_INFINITY;

        for i in 0..=n {
            let mut row = Vec::with_capacity(n + 1);
            let x = self.viewport.x_min + i as f64 * dx;

            for j in 0..=n {
                let y = self.viewport.y_min + j as f64 * dy;

                evaluator.set_var(x_var, Expr::Float(x));
                evaluator.set_var(y_var, Expr::Float(y));

                if let Ok(result) = evaluator.eval(expr) {
                    if let Ok(z) = expr_to_f64(&result) {
                        if z.is_finite() && z >= self.viewport.z_min && z <= self.viewport.z_max {
                            z_min = z_min.min(z);
                            z_max = z_max.max(z);
                            row.push(Some((x, y, z)));
                        } else {
                            row.push(None);
                        }
                    } else {
                        row.push(None);
                    }
                } else {
                    row.push(None);
                }
            }
            points.push(row);
        }

        // Avoid division by zero
        if (z_max - z_min).abs() < 1e-10 {
            z_max = z_min + 1.0;
        }

        // Collect quads for depth sorting
        let mut quads: Vec<Quad> = Vec::new();

        for i in 0..n {
            for j in 0..n {
                if let (Some(p00), Some(p10), Some(p11), Some(p01)) = (
                    points[i][j],
                    points[i + 1][j],
                    points[i + 1][j + 1],
                    points[i][j + 1],
                ) {
                    let avg_z = (p00.2 + p10.2 + p11.2 + p01.2) / 4.0;
                    let t = (avg_z - z_min) / (z_max - z_min);
                    let color = self.config.colormap.color(t);

                    // Calculate depth for sorting (distance from camera)
                    let cam_pos = self.camera.position();
                    let cx = (p00.0 + p10.0 + p11.0 + p01.0) / 4.0;
                    let cy = (p00.1 + p10.1 + p11.1 + p01.1) / 4.0;
                    let cz = (p00.2 + p10.2 + p11.2 + p01.2) / 4.0;
                    let depth = (cx - cam_pos.0).powi(2)
                              + (cy - cam_pos.1).powi(2)
                              + (cz - cam_pos.2).powi(2);

                    quads.push(Quad {
                        corners: [p00, p10, p11, p01],
                        color,
                        depth,
                    });
                }
            }
        }

        // Sort by depth (painter's algorithm - far to near)
        quads.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal));

        // Draw quads
        for quad in &quads {
            let corners: Vec<(f64, f64)> = quad.corners
                .iter()
                .map(|p| self.project(p.0, p.1, p.2, width, height))
                .collect();

            if self.config.render_mode != RenderMode::Wireframe {
                // Fill
                set_color(ctx, quad.color);
                ctx.move_to(corners[0].0, corners[0].1);
                for c in &corners[1..] {
                    ctx.line_to(c.0, c.1);
                }
                ctx.close_path();
                let _ = ctx.fill();
            }

            if self.config.render_mode != RenderMode::Filled {
                // Wireframe
                set_color(ctx, self.config.wireframe_color);
                ctx.set_line_width(0.5);
                ctx.move_to(corners[0].0, corners[0].1);
                for c in &corners[1..] {
                    ctx.line_to(c.0, c.1);
                }
                ctx.close_path();
                let _ = ctx.stroke();
            }
        }
    }

    fn draw_parametric_surface(
        &self,
        ctx: &Context,
        x_expr: &Expr,
        y_expr: &Expr,
        z_expr: &Expr,
        u_var: &str,
        v_var: &str,
        u_range: (f64, f64),
        v_range: (f64, f64),
        width: u32,
        height: u32,
    ) {
        let mut evaluator = Evaluator::new();
        let n = self.config.grid_lines;

        let du = (u_range.1 - u_range.0) / n as f64;
        let dv = (v_range.1 - v_range.0) / n as f64;

        // Sample the surface
        let mut points: Vec<Vec<Option<(f64, f64, f64)>>> = Vec::with_capacity(n + 1);
        let mut z_min = f64::INFINITY;
        let mut z_max = f64::NEG_INFINITY;

        for i in 0..=n {
            let mut row = Vec::with_capacity(n + 1);
            let u = u_range.0 + i as f64 * du;

            for j in 0..=n {
                let v = v_range.0 + j as f64 * dv;

                evaluator.set_var(u_var, Expr::Float(u));
                evaluator.set_var(v_var, Expr::Float(v));

                let x_result = evaluator.eval(x_expr);
                let y_result = evaluator.eval(y_expr);
                let z_result = evaluator.eval(z_expr);

                if let (Ok(xr), Ok(yr), Ok(zr)) = (x_result, y_result, z_result) {
                    if let (Ok(x), Ok(y), Ok(z)) = (expr_to_f64(&xr), expr_to_f64(&yr), expr_to_f64(&zr)) {
                        if x.is_finite() && y.is_finite() && z.is_finite() {
                            z_min = z_min.min(z);
                            z_max = z_max.max(z);
                            row.push(Some((x, y, z)));
                        } else {
                            row.push(None);
                        }
                    } else {
                        row.push(None);
                    }
                } else {
                    row.push(None);
                }
            }
            points.push(row);
        }

        if (z_max - z_min).abs() < 1e-10 {
            z_max = z_min + 1.0;
        }

        // Collect and sort quads
        let mut quads: Vec<Quad> = Vec::new();

        for i in 0..n {
            for j in 0..n {
                if let (Some(p00), Some(p10), Some(p11), Some(p01)) = (
                    points[i][j],
                    points[i + 1][j],
                    points[i + 1][j + 1],
                    points[i][j + 1],
                ) {
                    let avg_z = (p00.2 + p10.2 + p11.2 + p01.2) / 4.0;
                    let t = (avg_z - z_min) / (z_max - z_min);
                    let color = self.config.colormap.color(t);

                    let cam_pos = self.camera.position();
                    let cx = (p00.0 + p10.0 + p11.0 + p01.0) / 4.0;
                    let cy = (p00.1 + p10.1 + p11.1 + p01.1) / 4.0;
                    let cz = (p00.2 + p10.2 + p11.2 + p01.2) / 4.0;
                    let depth = (cx - cam_pos.0).powi(2)
                              + (cy - cam_pos.1).powi(2)
                              + (cz - cam_pos.2).powi(2);

                    quads.push(Quad {
                        corners: [p00, p10, p11, p01],
                        color,
                        depth,
                    });
                }
            }
        }

        quads.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal));

        for quad in &quads {
            let corners: Vec<(f64, f64)> = quad.corners
                .iter()
                .map(|p| self.project(p.0, p.1, p.2, width, height))
                .collect();

            if self.config.render_mode != RenderMode::Wireframe {
                set_color(ctx, quad.color);
                ctx.move_to(corners[0].0, corners[0].1);
                for c in &corners[1..] {
                    ctx.line_to(c.0, c.1);
                }
                ctx.close_path();
                let _ = ctx.fill();
            }

            if self.config.render_mode != RenderMode::Filled {
                set_color(ctx, self.config.wireframe_color);
                ctx.set_line_width(0.5);
                ctx.move_to(corners[0].0, corners[0].1);
                for c in &corners[1..] {
                    ctx.line_to(c.0, c.1);
                }
                ctx.close_path();
                let _ = ctx.stroke();
            }
        }
    }
}

/// A quad face for depth sorting
struct Quad {
    corners: [(f64, f64, f64); 4],
    color: Color,
    depth: f64,
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

/// Convert Expr to f64
fn expr_to_f64(expr: &Expr) -> Result<f64, ()> {
    match expr {
        Expr::Integer(n) => Ok(*n as f64),
        Expr::Float(x) => Ok(*x),
        Expr::Rational(r) => Ok(r.to_f64()),
        _ => Err(()),
    }
}

// Colormap implementations

fn viridis(t: f64) -> Color {
    // Approximation of viridis colormap
    let r = (0.267 + t * (0.329 + t * (1.421 + t * (-1.685 + t * 0.668)))).clamp(0.0, 1.0);
    let g = (0.004 + t * (1.260 + t * (-0.569 + t * 0.305))).clamp(0.0, 1.0);
    let b = (0.329 + t * (1.440 + t * (-2.814 + t * (2.768 - t * 0.723)))).clamp(0.0, 1.0);
    Color {
        r: (r * 255.0) as u8,
        g: (g * 255.0) as u8,
        b: (b * 255.0) as u8,
        a: 255,
    }
}

fn plasma(t: f64) -> Color {
    // Approximation of plasma colormap
    let r = (0.050 + t * (2.735 + t * (-2.811 + t * 0.826))).clamp(0.0, 1.0);
    let g = (0.029 + t * (-0.278 + t * (2.149 + t * (-1.100)))).clamp(0.0, 1.0);
    let b = (0.533 + t * (0.667 + t * (-2.519 + t * 1.319))).clamp(0.0, 1.0);
    Color {
        r: (r * 255.0) as u8,
        g: (g * 255.0) as u8,
        b: (b * 255.0) as u8,
        a: 255,
    }
}

fn coolwarm(t: f64) -> Color {
    // Blue (cool) to red (warm)
    let r = if t < 0.5 {
        0.2 + t * 1.6
    } else {
        1.0
    };
    let g = if t < 0.5 {
        0.2 + t * 1.2
    } else {
        0.8 - (t - 0.5) * 1.6
    };
    let b = if t < 0.5 {
        1.0
    } else {
        1.0 - (t - 0.5) * 1.6
    };
    Color {
        r: (r.clamp(0.0, 1.0) * 255.0) as u8,
        g: (g.clamp(0.0, 1.0) * 255.0) as u8,
        b: (b.clamp(0.0, 1.0) * 255.0) as u8,
        a: 255,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_default() {
        let cam = Camera3D::default();
        assert!((cam.azimuth - 45.0_f64.to_radians()).abs() < 0.01);
        assert!((cam.elevation - 30.0_f64.to_radians()).abs() < 0.01);
    }

    #[test]
    fn test_camera_rotate() {
        let mut cam = Camera3D::default();
        cam.rotate(0.1, 0.1);
        assert!(cam.azimuth > 45.0_f64.to_radians());
        assert!(cam.elevation > 30.0_f64.to_radians());
    }

    #[test]
    fn test_colormap_bounds() {
        let cm = Colormap::Viridis;
        let c0 = cm.color(0.0);
        let c1 = cm.color(1.0);
        assert!(c0.r < 100); // Dark at 0
        assert!(c1.g > 200); // Yellowish at 1
    }
}
