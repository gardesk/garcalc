//! Math rendering with Cairo
//!
//! Draws mathematical notation with proper typesetting including
//! fraction bars, radical signs, integral symbols, etc.

use crate::layout::{LayoutBox, MathLayoutEngine};
use crate::mathbox::{MathBox, Operator, LimitDirection};
use cairo::Context;
use gartk_core::Color;

/// Renderer for mathematical notation
pub struct MathRenderer<'a> {
    ctx: &'a Context,
    layout_engine: MathLayoutEngine,
    /// Text/foreground color
    pub fg_color: Color,
    /// Slot background color
    pub slot_bg_color: Color,
    /// Slot border color when focused
    pub slot_focus_color: Color,
}

impl<'a> MathRenderer<'a> {
    /// Create a new renderer with the given Cairo context
    pub fn new(ctx: &'a Context, font_family: &str, font_size: f64) -> Self {
        Self {
            ctx,
            layout_engine: MathLayoutEngine::new(font_family, font_size),
            fg_color: Color::new(0.0, 0.0, 0.0, 1.0),
            slot_bg_color: Color::new(0.9, 0.9, 0.95, 1.0),
            slot_focus_color: Color::new(0.3, 0.5, 0.9, 1.0),
        }
    }

    /// Render a MathBox tree at the given position
    /// (x, y) is the baseline position
    pub fn render(&self, mathbox: &MathBox, x: f64, y: f64) {
        self.render_at_depth(mathbox, x, y, 0, None);
    }

    /// Render with cursor highlighting
    pub fn render_with_cursor(&self, mathbox: &MathBox, x: f64, y: f64, cursor_path: &[usize]) {
        self.render_at_depth(mathbox, x, y, 0, Some(cursor_path));
    }

    /// Internal render with depth tracking
    fn render_at_depth(
        &self,
        mathbox: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
    ) {
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;

        // Check if cursor is at this node
        let is_cursor_here = cursor_path.map(|p| p.is_empty()).unwrap_or(false);

        match mathbox {
            MathBox::Number(s) => {
                self.draw_text(s, x, y, font_size, false);
                if is_cursor_here {
                    self.draw_cursor(x, y, font_size);
                }
            }
            MathBox::Symbol(s) => {
                self.draw_text(s, x, y, font_size, true);
                if is_cursor_here {
                    self.draw_cursor(x, y, font_size);
                }
            }
            MathBox::Operator(op) => {
                self.draw_operator(*op, x, y, font_size);
            }
            MathBox::Slot => {
                self.draw_slot(x, y, font_size, is_cursor_here);
            }
            MathBox::Fraction { num, den } => {
                self.render_fraction(num, den, x, y, depth, cursor_path);
            }
            MathBox::Power { base, exp } => {
                self.render_power(base, exp, x, y, depth, cursor_path);
            }
            MathBox::Subscript { base, sub } => {
                self.render_subscript(base, sub, x, y, depth, cursor_path);
            }
            MathBox::Root { index, radicand } => {
                self.render_root(index.as_deref(), radicand, x, y, depth, cursor_path);
            }
            MathBox::Func { name, args } => {
                self.render_func(name, args, x, y, depth, cursor_path);
            }
            MathBox::Abs(inner) => {
                self.render_abs(inner, x, y, depth, cursor_path);
            }
            MathBox::Parens(inner) => {
                self.render_parens(inner, x, y, depth, cursor_path);
            }
            MathBox::Integral { lower, upper, body, var } => {
                self.render_integral(
                    lower.as_deref(),
                    upper.as_deref(),
                    body,
                    var,
                    x,
                    y,
                    depth,
                    cursor_path,
                );
            }
            MathBox::Derivative { order, var, body } => {
                self.render_derivative(*order, var, body, x, y, depth, cursor_path);
            }
            MathBox::Limit { var, to, direction, body } => {
                self.render_limit(var, to, *direction, body, x, y, depth, cursor_path);
            }
            MathBox::Sum { var, lower, upper, body } => {
                self.render_bigop("∑", var, lower, upper, body, x, y, depth, cursor_path);
            }
            MathBox::Product { var, lower, upper, body } => {
                self.render_bigop("∏", var, lower, upper, body, x, y, depth, cursor_path);
            }
            MathBox::Matrix { rows } => {
                self.render_matrix(rows, x, y, depth, cursor_path);
            }
            MathBox::Row(items) => {
                self.render_row(items, x, y, depth, cursor_path);
            }
        }
    }

    fn scale_for_depth(&self, depth: u32) -> f64 {
        self.layout_engine.scale_for_depth(depth)
    }

    /// Draw plain text
    fn draw_text(&self, text: &str, x: f64, y: f64, font_size: f64, italic: bool) {
        self.ctx.save().unwrap();
        self.set_color(&self.fg_color);

        let slant = if italic {
            cairo::FontSlant::Italic
        } else {
            cairo::FontSlant::Normal
        };

        self.ctx.select_font_face(
            &self.layout_engine.font_family,
            slant,
            cairo::FontWeight::Normal,
        );
        self.ctx.set_font_size(font_size);
        self.ctx.move_to(x, y);
        self.ctx.show_text(text).unwrap();
        self.ctx.restore().unwrap();
    }

    /// Draw an operator
    fn draw_operator(&self, op: Operator, x: f64, y: f64, font_size: f64) {
        let padding = font_size * 0.15;
        self.draw_text(&op.as_char().to_string(), x + padding, y, font_size, false);
    }

    /// Draw an empty slot
    fn draw_slot(&self, x: f64, y: f64, font_size: f64, focused: bool) {
        let width = font_size * 0.8;
        let height = font_size * 0.8;
        let ascent = height * 0.6;

        self.ctx.save().unwrap();

        // Background
        self.set_color(&self.slot_bg_color);
        self.ctx.rectangle(x, y - ascent, width, height);
        self.ctx.fill().unwrap();

        // Border
        if focused {
            self.set_color(&self.slot_focus_color);
            self.ctx.set_line_width(2.0);
        } else {
            self.set_color(&self.fg_color);
            self.ctx.set_line_width(0.5);
        }
        self.ctx.rectangle(x, y - ascent, width, height);
        self.ctx.stroke().unwrap();

        self.ctx.restore().unwrap();
    }

    /// Draw cursor
    fn draw_cursor(&self, x: f64, y: f64, font_size: f64) {
        self.ctx.save().unwrap();
        self.set_color(&self.slot_focus_color);
        self.ctx.set_line_width(2.0);

        let height = font_size * 0.8;
        self.ctx.move_to(x, y - height * 0.6);
        self.ctx.line_to(x, y + height * 0.4);
        self.ctx.stroke().unwrap();

        self.ctx.restore().unwrap();
    }

    /// Render a fraction
    fn render_fraction(
        &self,
        num: &MathBox,
        den: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
    ) {
        let layout = self.layout_engine.layout(&MathBox::Fraction {
            num: Box::new(num.clone()),
            den: Box::new(den.clone()),
        }, self.ctx);

        let scale = self.scale_for_depth(depth);
        let bar_thickness = 1.0 * scale;
        let gap = self.layout_engine.base_font_size * 0.1 * scale;

        // Get child layouts for positioning
        let num_layout = self.layout_engine.layout(num, self.ctx);
        let den_layout = self.layout_engine.layout(den, self.ctx);

        let num_x = x + (layout.width - num_layout.width) / 2.0;
        let den_x = x + (layout.width - den_layout.width) / 2.0;
        let num_y = y - (gap + bar_thickness / 2.0 + num_layout.descent);
        let den_y = y + (gap + bar_thickness / 2.0 + den_layout.ascent);

        // Draw fraction bar
        self.ctx.save().unwrap();
        self.set_color(&self.fg_color);
        self.ctx.set_line_width(bar_thickness);
        self.ctx.move_to(x, y);
        self.ctx.line_to(x + layout.width, y);
        self.ctx.stroke().unwrap();
        self.ctx.restore().unwrap();

        // Draw numerator and denominator
        let num_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 { Some(&p[1..]) } else { None }
        });
        let den_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 1 { Some(&p[1..]) } else { None }
        });

        self.render_at_depth(num, num_x, num_y, depth + 1, num_cursor);
        self.render_at_depth(den, den_x, den_y, depth + 1, den_cursor);
    }

    /// Render a power (superscript)
    fn render_power(
        &self,
        base: &MathBox,
        exp: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
    ) {
        let base_layout = self.layout_engine.layout(base, self.ctx);

        let base_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 { Some(&p[1..]) } else { None }
        });
        let exp_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 1 { Some(&p[1..]) } else { None }
        });

        self.render_at_depth(base, x, y, depth, base_cursor);

        let exp_raise = base_layout.ascent * 0.5;
        self.render_at_depth(exp, x + base_layout.width, y - exp_raise, depth + 1, exp_cursor);
    }

    /// Render a subscript
    fn render_subscript(
        &self,
        base: &MathBox,
        sub: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
    ) {
        let base_layout = self.layout_engine.layout(base, self.ctx);
        let sub_layout = self.layout_engine.layout(sub, self.ctx);

        let base_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 { Some(&p[1..]) } else { None }
        });
        let sub_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 1 { Some(&p[1..]) } else { None }
        });

        self.render_at_depth(base, x, y, depth, base_cursor);

        let sub_lower = base_layout.descent + sub_layout.ascent * 0.3;
        self.render_at_depth(sub, x + base_layout.width, y + sub_lower, depth + 1, sub_cursor);
    }

    /// Render a root (square or nth)
    fn render_root(
        &self,
        index: Option<&MathBox>,
        radicand: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
    ) {
        let radicand_layout = self.layout_engine.layout(radicand, self.ctx);
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;

        let radical_width = font_size * 0.6;
        let bar_overhang = font_size * 0.1;
        let gap = font_size * 0.1;

        let _height = radicand_layout.height() + gap;
        let mut radicand_x = x + radical_width;

        // Draw index if present
        if let Some(idx) = index {
            let idx_layout = self.layout_engine.layout(idx, self.ctx);
            let idx_cursor = cursor_path.and_then(|p| {
                if !p.is_empty() && p[0] == 0 { Some(&p[1..]) } else { None }
            });
            self.render_at_depth(
                idx,
                x,
                y - radicand_layout.ascent * 0.5 - idx_layout.descent,
                depth + 2,
                idx_cursor,
            );
            radicand_x += idx_layout.width;
        }

        // Draw radical symbol
        self.ctx.save().unwrap();
        self.set_color(&self.fg_color);
        self.ctx.set_line_width(1.0 * scale);

        // Radical checkmark
        let check_width = radical_width * 0.4;
        self.ctx.move_to(radicand_x - radical_width, y);
        self.ctx.line_to(radicand_x - radical_width + check_width * 0.3, y + radicand_layout.descent * 0.3);
        self.ctx.line_to(radicand_x - radical_width + check_width, y + radicand_layout.descent);
        self.ctx.line_to(radicand_x, y - radicand_layout.ascent - gap);

        // Overbar
        self.ctx.line_to(radicand_x + radicand_layout.width + bar_overhang, y - radicand_layout.ascent - gap);
        self.ctx.stroke().unwrap();
        self.ctx.restore().unwrap();

        // Draw radicand
        let rad_cursor = cursor_path.and_then(|p| {
            let idx = if index.is_some() { 1 } else { 0 };
            if !p.is_empty() && p[0] == idx { Some(&p[1..]) } else { None }
        });
        self.render_at_depth(radicand, radicand_x, y, depth, rad_cursor);
    }

    /// Render a function call
    fn render_func(
        &self,
        name: &str,
        args: &[MathBox],
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
    ) {
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;
        let paren_width = font_size * 0.3;

        // Draw function name
        self.draw_text(name, x, y, font_size, false);

        self.ctx.set_font_size(font_size);
        let name_extents = self.ctx.text_extents(name).unwrap();
        let mut current_x = x + name_extents.x_advance();

        // Draw opening paren
        self.draw_text("(", current_x, y, font_size, false);
        current_x += paren_width;

        // Draw arguments
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                self.draw_text(",", current_x, y, font_size, false);
                current_x += font_size * 0.3;
            }

            let arg_cursor = cursor_path.and_then(|p| {
                if !p.is_empty() && p[0] == i { Some(&p[1..]) } else { None }
            });
            self.render_at_depth(arg, current_x, y, depth, arg_cursor);

            let arg_layout = self.layout_engine.layout(arg, self.ctx);
            current_x += arg_layout.width;
        }

        // Draw closing paren
        self.draw_text(")", current_x, y, font_size, false);
    }

    /// Render absolute value
    fn render_abs(
        &self,
        inner: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
    ) {
        let inner_layout = self.layout_engine.layout(inner, self.ctx);
        let scale = self.scale_for_depth(depth);
        let bar_width = self.layout_engine.base_font_size * 0.15 * scale;

        let _height = inner_layout.height();

        // Draw vertical bars
        self.ctx.save().unwrap();
        self.set_color(&self.fg_color);
        self.ctx.set_line_width(1.5 * scale);

        // Left bar
        self.ctx.move_to(x + bar_width / 2.0, y - inner_layout.ascent);
        self.ctx.line_to(x + bar_width / 2.0, y + inner_layout.descent);
        self.ctx.stroke().unwrap();

        // Right bar
        let right_x = x + bar_width + inner_layout.width + bar_width / 2.0;
        self.ctx.move_to(right_x, y - inner_layout.ascent);
        self.ctx.line_to(right_x, y + inner_layout.descent);
        self.ctx.stroke().unwrap();

        self.ctx.restore().unwrap();

        // Draw inner expression
        let inner_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 { Some(&p[1..]) } else { None }
        });
        self.render_at_depth(inner, x + bar_width, y, depth, inner_cursor);
    }

    /// Render parenthesized expression
    fn render_parens(
        &self,
        inner: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
    ) {
        let inner_layout = self.layout_engine.layout(inner, self.ctx);
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;
        let paren_width = font_size * 0.25;

        // For now, draw text parentheses (could be replaced with curved paths)
        self.draw_text("(", x, y, font_size * 1.2, false);
        self.draw_text(")", x + paren_width + inner_layout.width, y, font_size * 1.2, false);

        let inner_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 { Some(&p[1..]) } else { None }
        });
        self.render_at_depth(inner, x + paren_width, y, depth, inner_cursor);
    }

    /// Render an integral
    fn render_integral(
        &self,
        lower: Option<&MathBox>,
        upper: Option<&MathBox>,
        body: &MathBox,
        var: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
    ) {
        let body_layout = self.layout_engine.layout(body, self.ctx);
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;

        let int_height = body_layout.height().max(font_size * 1.5);
        let int_width = font_size * 0.5;

        // Draw integral symbol using a large font size
        self.ctx.save().unwrap();
        self.set_color(&self.fg_color);
        self.ctx.set_font_size(int_height);
        self.ctx.move_to(x, y + int_height * 0.3);
        self.ctx.show_text("∫").unwrap();
        self.ctx.restore().unwrap();

        let mut bounds_width = int_width;
        let mut child_idx = 0;

        // Draw lower bound
        if let Some(lo) = lower {
            let lo_layout = self.layout_engine.layout(lo, self.ctx);
            let lo_cursor = cursor_path.and_then(|p| {
                if !p.is_empty() && p[0] == child_idx { Some(&p[1..]) } else { None }
            });
            self.render_at_depth(
                lo,
                x,
                y + int_height / 2.0 + lo_layout.ascent,
                depth + 1,
                lo_cursor,
            );
            bounds_width = bounds_width.max(lo_layout.width);
            child_idx += 1;
        }

        // Draw upper bound
        if let Some(hi) = upper {
            let hi_layout = self.layout_engine.layout(hi, self.ctx);
            let hi_cursor = cursor_path.and_then(|p| {
                if !p.is_empty() && p[0] == child_idx { Some(&p[1..]) } else { None }
            });
            self.render_at_depth(
                hi,
                x,
                y - int_height / 2.0 - hi_layout.descent,
                depth + 1,
                hi_cursor,
            );
            bounds_width = bounds_width.max(hi_layout.width);
            child_idx += 1;
        }

        // Draw body
        let body_x = x + bounds_width + font_size * 0.2;
        let body_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == child_idx { Some(&p[1..]) } else { None }
        });
        self.render_at_depth(body, body_x, y, depth, body_cursor);
        child_idx += 1;

        // Draw "dx"
        let var_x = body_x + body_layout.width + font_size * 0.1;
        self.draw_text("d", var_x, y, font_size, false);

        let d_extents = self.ctx.text_extents("d").unwrap();
        let var_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == child_idx { Some(&p[1..]) } else { None }
        });
        self.render_at_depth(var, var_x + d_extents.x_advance(), y, depth, var_cursor);
    }

    /// Render a derivative
    fn render_derivative(
        &self,
        order: u32,
        var: &MathBox,
        body: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
    ) {
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;

        // Build strings for numerator and denominator
        let num_str = if order > 1 {
            format!("d{}", superscript_digits(order))
        } else {
            "d".to_string()
        };

        let den_prefix = if order > 1 {
            format!("d{}", superscript_digits(order))
        } else {
            "d".to_string()
        };

        // Measure text
        self.ctx.set_font_size(font_size * 0.8);
        let num_extents = self.ctx.text_extents(&num_str).unwrap();
        let den_prefix_extents = self.ctx.text_extents(&den_prefix).unwrap();

        let var_layout = self.layout_engine.layout(var, self.ctx);
        let _body_layout = self.layout_engine.layout(body, self.ctx);

        let frac_width = num_extents.x_advance().max(den_prefix_extents.x_advance() + var_layout.width);
        let bar_gap = font_size * 0.15;

        // Draw numerator
        self.draw_text(&num_str, x + (frac_width - num_extents.x_advance()) / 2.0, y - bar_gap - font_size * 0.3, font_size * 0.8, false);

        // Draw fraction bar
        self.ctx.save().unwrap();
        self.set_color(&self.fg_color);
        self.ctx.set_line_width(1.0 * scale);
        self.ctx.move_to(x, y);
        self.ctx.line_to(x + frac_width, y);
        self.ctx.stroke().unwrap();
        self.ctx.restore().unwrap();

        // Draw denominator
        let den_x = x + (frac_width - den_prefix_extents.x_advance() - var_layout.width) / 2.0;
        self.draw_text(&den_prefix, den_x, y + bar_gap + font_size * 0.6, font_size * 0.8, false);

        let var_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 { Some(&p[1..]) } else { None }
        });
        self.render_at_depth(var, den_x + den_prefix_extents.x_advance(), y + bar_gap + font_size * 0.6, depth + 1, var_cursor);

        // Draw body
        let body_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 1 { Some(&p[1..]) } else { None }
        });
        self.render_at_depth(body, x + frac_width + font_size * 0.3, y, depth, body_cursor);
    }

    /// Render a limit
    fn render_limit(
        &self,
        var: &MathBox,
        to: &MathBox,
        direction: Option<LimitDirection>,
        body: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
    ) {
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;

        // Draw "lim"
        self.draw_text("lim", x, y, font_size, false);
        self.ctx.set_font_size(font_size);
        let lim_extents = self.ctx.text_extents("lim").unwrap();

        // Draw subscript: var → to
        let subscript_y = y + font_size * 0.5;
        let sub_font_size = font_size * 0.7;

        let var_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 { Some(&p[1..]) } else { None }
        });
        self.render_at_depth(var, x, subscript_y, depth + 1, var_cursor);

        let var_layout = self.layout_engine.layout(var, self.ctx);
        let arrow_x = x + var_layout.width;
        self.draw_text("→", arrow_x, subscript_y, sub_font_size, false);

        self.ctx.set_font_size(sub_font_size);
        let arrow_extents = self.ctx.text_extents("→").unwrap();

        let to_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 1 { Some(&p[1..]) } else { None }
        });
        self.render_at_depth(to, arrow_x + arrow_extents.x_advance(), subscript_y, depth + 1, to_cursor);

        let to_layout = self.layout_engine.layout(to, self.ctx);

        // Draw direction indicator if present
        if let Some(dir) = direction {
            let dir_str = match dir {
                LimitDirection::FromRight => "⁺",
                LimitDirection::FromLeft => "⁻",
            };
            self.draw_text(dir_str, arrow_x + arrow_extents.x_advance() + to_layout.width, subscript_y - sub_font_size * 0.3, sub_font_size * 0.6, false);
        }

        // Draw body
        let body_x = x + lim_extents.x_advance() + font_size * 0.3;
        let body_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 2 { Some(&p[1..]) } else { None }
        });
        self.render_at_depth(body, body_x, y, depth, body_cursor);
    }

    /// Render a big operator (sum, product)
    fn render_bigop(
        &self,
        symbol: &str,
        _var: &MathBox,
        lower: &MathBox,
        upper: &MathBox,
        body: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
    ) {
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;
        let symbol_size = font_size * 1.5;

        // Draw the big symbol
        self.ctx.save().unwrap();
        self.set_color(&self.fg_color);
        self.ctx.set_font_size(symbol_size);
        self.ctx.move_to(x, y + symbol_size * 0.3);
        self.ctx.show_text(symbol).unwrap();
        let symbol_extents = self.ctx.text_extents(symbol).unwrap();
        self.ctx.restore().unwrap();

        let op_width = symbol_extents.x_advance();
        let gap = font_size * 0.15;

        // Draw upper bound
        let upper_layout = self.layout_engine.layout(upper, self.ctx);
        let upper_x = x + (op_width - upper_layout.width) / 2.0;
        let upper_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 2 { Some(&p[1..]) } else { None }
        });
        self.render_at_depth(upper, upper_x, y - symbol_size / 2.0 - gap - upper_layout.descent, depth + 1, upper_cursor);

        // Draw lower bound (includes "var=")
        let lower_layout = self.layout_engine.layout(lower, self.ctx);
        let lower_x = x + (op_width - lower_layout.width) / 2.0;
        let lower_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 1 { Some(&p[1..]) } else { None }
        });
        self.render_at_depth(lower, lower_x, y + symbol_size / 2.0 + gap + lower_layout.ascent, depth + 1, lower_cursor);

        // Draw body
        let body_x = x + op_width + font_size * 0.3;
        let body_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 3 { Some(&p[1..]) } else { None }
        });
        self.render_at_depth(body, body_x, y, depth, body_cursor);
    }

    /// Render a matrix
    fn render_matrix(
        &self,
        rows: &[Vec<MathBox>],
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
    ) {
        if rows.is_empty() {
            return;
        }

        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;
        let cell_padding = font_size * 0.3;
        let bracket_width = font_size * 0.2;

        let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        if num_cols == 0 {
            return;
        }

        // Compute cell layouts
        let cell_layouts: Vec<Vec<LayoutBox>> = rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| self.layout_engine.layout(cell, self.ctx))
                    .collect()
            })
            .collect();

        // Find column widths and row heights
        let mut col_widths = vec![0.0f64; num_cols];
        let mut row_heights = vec![0.0f64; rows.len()];

        for (r, row) in cell_layouts.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                col_widths[c] = col_widths[c].max(cell.width);
                row_heights[r] = row_heights[r].max(cell.height());
            }
        }

        let total_width: f64 = col_widths.iter().sum::<f64>()
            + cell_padding * (num_cols as f64 - 1.0)
            + 2.0 * bracket_width;
        let total_height: f64 = row_heights.iter().sum::<f64>()
            + cell_padding * (rows.len() as f64 - 1.0);

        // Draw brackets
        self.ctx.save().unwrap();
        self.set_color(&self.fg_color);
        self.ctx.set_line_width(1.5 * scale);

        // Left bracket
        let bracket_gap = font_size * 0.15;
        self.ctx.move_to(x + bracket_gap, y - total_height / 2.0);
        self.ctx.line_to(x, y - total_height / 2.0);
        self.ctx.line_to(x, y + total_height / 2.0);
        self.ctx.line_to(x + bracket_gap, y + total_height / 2.0);
        self.ctx.stroke().unwrap();

        // Right bracket
        let right_x = x + total_width - bracket_width;
        self.ctx.move_to(right_x - bracket_gap, y - total_height / 2.0);
        self.ctx.line_to(right_x, y - total_height / 2.0);
        self.ctx.line_to(right_x, y + total_height / 2.0);
        self.ctx.line_to(right_x - bracket_gap, y + total_height / 2.0);
        self.ctx.stroke().unwrap();

        self.ctx.restore().unwrap();

        // Draw cells
        let mut cell_y = y - total_height / 2.0;
        let mut cell_idx = 0;

        for (r, row) in rows.iter().enumerate() {
            let row_h = row_heights[r];
            let mut cell_x = x + bracket_width;

            for (c, cell) in row.iter().enumerate() {
                let col_w = col_widths[c];
                let layout = &cell_layouts[r][c];

                // Center cell in its column
                let cx = cell_x + (col_w - layout.width) / 2.0;
                let cy = cell_y + row_h / 2.0;

                let cell_cursor = cursor_path.and_then(|p| {
                    if !p.is_empty() && p[0] == cell_idx { Some(&p[1..]) } else { None }
                });
                self.render_at_depth(cell, cx, cy, depth, cell_cursor);

                cell_x += col_w + cell_padding;
                cell_idx += 1;
            }

            cell_y += row_h + cell_padding;
        }
    }

    /// Render a horizontal row
    fn render_row(
        &self,
        items: &[MathBox],
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
    ) {
        let mut current_x = x;

        for (i, item) in items.iter().enumerate() {
            let item_cursor = cursor_path.and_then(|p| {
                if !p.is_empty() && p[0] == i { Some(&p[1..]) } else { None }
            });
            self.render_at_depth(item, current_x, y, depth, item_cursor);

            let layout = self.layout_engine.layout(item, self.ctx);
            current_x += layout.width;
        }
    }

    /// Set the current drawing color
    fn set_color(&self, color: &Color) {
        self.ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    }
}

/// Convert a number to superscript Unicode digits
fn superscript_digits(n: u32) -> String {
    n.to_string()
        .chars()
        .map(|c| match c {
            '0' => '⁰',
            '1' => '¹',
            '2' => '²',
            '3' => '³',
            '4' => '⁴',
            '5' => '⁵',
            '6' => '⁶',
            '7' => '⁷',
            '8' => '⁸',
            '9' => '⁹',
            _ => c,
        })
        .collect()
}
