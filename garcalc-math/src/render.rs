//! Math rendering with Cairo
//!
//! Draws mathematical notation with proper typesetting including
//! fraction bars, radical signs, integral symbols, etc.

use crate::layout::{LayoutBox, MathLayoutEngine};
use crate::mathbox::{LimitDirection, MathBox, Operator};
use cairo::Context;
use gartk_core::Color;
use std::cell::Cell;

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
    /// Whether the insertion cursor should be drawn
    cursor_visible: Cell<bool>,
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
            cursor_visible: Cell::new(true),
        }
    }

    /// Render a MathBox tree at the given position
    /// (x, y) is the baseline position
    pub fn render(&self, mathbox: &MathBox, x: f64, y: f64) {
        self.render_at_depth(mathbox, x, y, 0, None, 0);
    }

    /// Render with cursor highlighting
    pub fn render_with_cursor(
        &self,
        mathbox: &MathBox,
        x: f64,
        y: f64,
        cursor_path: &[usize],
        cursor_offset: usize,
        cursor_visible: bool,
    ) {
        self.cursor_visible.set(cursor_visible);
        self.render_at_depth(mathbox, x, y, 0, Some(cursor_path), cursor_offset);
        self.cursor_visible.set(true);
    }

    /// Internal render with depth tracking
    fn render_at_depth(
        &self,
        mathbox: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
        cursor_offset: usize,
    ) {
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;

        // Check if cursor is at this node
        let is_cursor_here = cursor_path.map(|p| p.is_empty()).unwrap_or(false);
        let is_active_container = cursor_path.map(|p| p.len() == 1).unwrap_or(false)
            && Self::is_focusable_container(mathbox);

        match mathbox {
            MathBox::Number(s) => {
                self.draw_text(s, x, y, font_size, false);
                if is_cursor_here {
                    let width = self.text_advance(s, font_size, false);
                    self.draw_focus_frame(x, y, width, font_size, 1.4, 0.35);
                }
                if is_cursor_here && self.cursor_visible.get() {
                    let cursor_x =
                        x + self.text_advance_for_offset(s, font_size, false, cursor_offset);
                    self.draw_cursor(cursor_x, y, font_size);
                }
            }
            MathBox::Symbol(s) => {
                self.draw_text(s, x, y, font_size, true);
                if is_cursor_here {
                    let width = self.text_advance(s, font_size, true);
                    self.draw_focus_frame(x, y, width, font_size, 1.4, 0.35);
                }
                if is_cursor_here && self.cursor_visible.get() {
                    let cursor_x =
                        x + self.text_advance_for_offset(s, font_size, true, cursor_offset);
                    self.draw_cursor(cursor_x, y, font_size);
                }
            }
            MathBox::Operator(op) => {
                self.draw_operator(*op, x, y, font_size);
            }
            MathBox::Slot => {
                self.draw_slot(x, y, font_size, is_cursor_here);
            }
            MathBox::Fraction { num, den } => {
                self.render_fraction(num, den, x, y, depth, cursor_path, cursor_offset);
            }
            MathBox::Power { base, exp } => {
                self.render_power(base, exp, x, y, depth, cursor_path, cursor_offset);
            }
            MathBox::Subscript { base, sub } => {
                self.render_subscript(base, sub, x, y, depth, cursor_path, cursor_offset);
            }
            MathBox::Root { index, radicand } => {
                self.render_root(
                    index.as_deref(),
                    radicand,
                    x,
                    y,
                    depth,
                    cursor_path,
                    cursor_offset,
                );
            }
            MathBox::Func { name, args } => {
                self.render_func(name, args, x, y, depth, cursor_path, cursor_offset);
            }
            MathBox::Abs(inner) => {
                self.render_abs(inner, x, y, depth, cursor_path, cursor_offset);
            }
            MathBox::Parens(inner) => {
                self.render_parens(inner, x, y, depth, cursor_path, cursor_offset);
            }
            MathBox::Integral {
                lower,
                upper,
                body,
                var,
            } => {
                self.render_integral(
                    lower.as_deref(),
                    upper.as_deref(),
                    body,
                    var,
                    x,
                    y,
                    depth,
                    cursor_path,
                    cursor_offset,
                );
            }
            MathBox::Derivative { order, var, body } => {
                self.render_derivative(*order, var, body, x, y, depth, cursor_path, cursor_offset);
            }
            MathBox::Limit {
                var,
                to,
                direction,
                body,
            } => {
                self.render_limit(
                    var,
                    to,
                    *direction,
                    body,
                    x,
                    y,
                    depth,
                    cursor_path,
                    cursor_offset,
                );
            }
            MathBox::Sum {
                var,
                lower,
                upper,
                body,
            } => {
                self.render_bigop(
                    "∑",
                    var,
                    lower,
                    upper,
                    body,
                    x,
                    y,
                    depth,
                    cursor_path,
                    cursor_offset,
                );
            }
            MathBox::Product {
                var,
                lower,
                upper,
                body,
            } => {
                self.render_bigop(
                    "∏",
                    var,
                    lower,
                    upper,
                    body,
                    x,
                    y,
                    depth,
                    cursor_path,
                    cursor_offset,
                );
            }
            MathBox::Matrix { rows } => {
                self.render_matrix(rows, x, y, depth, cursor_path, cursor_offset);
            }
            MathBox::Row(items) => {
                self.render_row(items, x, y, depth, cursor_path, cursor_offset);
            }
        }

        if is_active_container {
            self.draw_container_focus_frame(mathbox, x, y, depth);
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

    /// Measure text advance using the same style as `draw_text`
    fn text_advance(&self, text: &str, font_size: f64, italic: bool) -> f64 {
        self.ctx.save().unwrap();
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
        let advance = self
            .ctx
            .text_extents(text)
            .map(|e| e.x_advance())
            .unwrap_or(0.0);
        self.ctx.restore().unwrap();
        advance
    }

    fn text_advance_for_offset(
        &self,
        text: &str,
        font_size: f64,
        italic: bool,
        char_offset: usize,
    ) -> f64 {
        let prefix: String = text
            .chars()
            .take(char_offset.min(text.chars().count()))
            .collect();
        self.text_advance(&prefix, font_size, italic)
    }

    /// Draw an operator
    fn draw_operator(&self, op: Operator, x: f64, y: f64, font_size: f64) {
        let padding = font_size * 0.15;
        self.draw_text(&op.as_char().to_string(), x + padding, y, font_size, false);
    }

    /// Draw an empty slot
    fn draw_slot(&self, x: f64, y: f64, font_size: f64, focused: bool) {
        let (width, height, ascent) = Self::slot_geometry(font_size);

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

        let (_, height, _) = Self::slot_geometry(font_size);
        self.ctx.move_to(x, y - height * 0.6);
        self.ctx.line_to(x, y + height * 0.4);
        self.ctx.stroke().unwrap();

        self.ctx.restore().unwrap();
    }

    /// Draw a focus frame around the active token
    fn draw_focus_frame(
        &self,
        x: f64,
        y: f64,
        width: f64,
        font_size: f64,
        line_width: f64,
        alpha: f64,
    ) {
        let ascent = font_size * 0.72;
        let descent = font_size * 0.28;
        let padding = (font_size * 0.12).max(1.2);
        self.draw_focus_bounds(x, y, width, ascent, descent, padding, line_width, alpha);
    }

    /// Draw a focus frame around the active container (e.g. power/fraction)
    fn draw_container_focus_frame(&self, mathbox: &MathBox, x: f64, y: f64, depth: u32) {
        let layout = self
            .layout_engine
            .layout_with_depth(mathbox, self.ctx, depth);
        let scale = self.scale_for_depth(depth);
        let padding = (self.layout_engine.base_font_size * scale * 0.12).max(1.4);
        self.draw_focus_bounds(
            x,
            y,
            layout.width,
            layout.ascent,
            layout.descent,
            padding,
            1.5,
            0.28,
        );
    }

    fn draw_focus_bounds(
        &self,
        x: f64,
        y: f64,
        width: f64,
        ascent: f64,
        descent: f64,
        padding: f64,
        line_width: f64,
        alpha: f64,
    ) {
        self.ctx.save().unwrap();
        self.set_color(&Color::new(
            self.slot_focus_color.r,
            self.slot_focus_color.g,
            self.slot_focus_color.b,
            alpha,
        ));
        self.ctx.set_line_width(line_width);
        self.ctx.rectangle(
            x - padding,
            y - ascent - padding,
            width + padding * 2.0,
            ascent + descent + padding * 2.0,
        );
        self.ctx.stroke().unwrap();
        self.ctx.restore().unwrap();
    }

    fn is_focusable_container(mathbox: &MathBox) -> bool {
        matches!(
            mathbox,
            MathBox::Fraction { .. }
                | MathBox::Power { .. }
                | MathBox::Subscript { .. }
                | MathBox::Root { .. }
                | MathBox::Func { .. }
                | MathBox::Abs(_)
                | MathBox::Parens(_)
                | MathBox::Integral { .. }
                | MathBox::Derivative { .. }
                | MathBox::Limit { .. }
                | MathBox::Sum { .. }
                | MathBox::Product { .. }
                | MathBox::Matrix { .. }
        )
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
        cursor_offset: usize,
    ) {
        let layout = self.layout_engine.layout(
            &MathBox::Fraction {
                num: Box::new(num.clone()),
                den: Box::new(den.clone()),
            },
            self.ctx,
        );

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
            if !p.is_empty() && p[0] == 0 {
                Some(&p[1..])
            } else {
                None
            }
        });
        let den_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 1 {
                Some(&p[1..])
            } else {
                None
            }
        });

        self.render_at_depth(num, num_x, num_y, depth + 1, num_cursor, cursor_offset);
        self.render_at_depth(den, den_x, den_y, depth + 1, den_cursor, cursor_offset);
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
        cursor_offset: usize,
    ) {
        let base_layout = self.layout_engine.layout_with_depth(base, self.ctx, depth);
        let exp_layout = self
            .layout_engine
            .layout_with_depth(exp, self.ctx, depth + 1);
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;

        let base_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 {
                Some(&p[1..])
            } else {
                None
            }
        });
        let exp_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 1 {
                Some(&p[1..])
            } else {
                None
            }
        });

        self.render_at_depth(base, x, y, depth, base_cursor, cursor_offset);

        let exp_raise = base_layout.ascent * 0.58 + exp_layout.descent * 0.1;
        let exp_kern = (font_size * 0.06).max(0.8);
        self.render_at_depth(
            exp,
            x + base_layout.width + exp_kern,
            y - exp_raise,
            depth + 1,
            exp_cursor,
            cursor_offset,
        );
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
        cursor_offset: usize,
    ) {
        let base_layout = self.layout_engine.layout_with_depth(base, self.ctx, depth);
        let sub_layout = self
            .layout_engine
            .layout_with_depth(sub, self.ctx, depth + 1);
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;

        let base_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 {
                Some(&p[1..])
            } else {
                None
            }
        });
        let sub_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 1 {
                Some(&p[1..])
            } else {
                None
            }
        });

        self.render_at_depth(base, x, y, depth, base_cursor, cursor_offset);

        let sub_lower =
            (base_layout.descent * 0.6 + sub_layout.ascent * 0.9).max(sub_layout.ascent * 0.75);
        let sub_kern = (font_size * 0.05).max(0.6);
        self.render_at_depth(
            sub,
            x + base_layout.width + sub_kern,
            y + sub_lower,
            depth + 1,
            sub_cursor,
            cursor_offset,
        );
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
        cursor_offset: usize,
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
                if !p.is_empty() && p[0] == 0 {
                    Some(&p[1..])
                } else {
                    None
                }
            });
            self.render_at_depth(
                idx,
                x,
                y - radicand_layout.ascent * 0.5 - idx_layout.descent,
                depth + 2,
                idx_cursor,
                cursor_offset,
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
        self.ctx.line_to(
            radicand_x - radical_width + check_width * 0.3,
            y + radicand_layout.descent * 0.3,
        );
        self.ctx.line_to(
            radicand_x - radical_width + check_width,
            y + radicand_layout.descent,
        );
        self.ctx
            .line_to(radicand_x, y - radicand_layout.ascent - gap);

        // Overbar
        self.ctx.line_to(
            radicand_x + radicand_layout.width + bar_overhang,
            y - radicand_layout.ascent - gap,
        );
        self.ctx.stroke().unwrap();
        self.ctx.restore().unwrap();

        // Draw radicand
        let rad_cursor = cursor_path.and_then(|p| {
            let idx = if index.is_some() { 1 } else { 0 };
            if !p.is_empty() && p[0] == idx {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(radicand, radicand_x, y, depth, rad_cursor, cursor_offset);
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
        cursor_offset: usize,
    ) {
        if name == "factorial" && args.len() == 1 {
            self.render_factorial(&args[0], x, y, depth, cursor_path, cursor_offset);
            return;
        }

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
                if !p.is_empty() && p[0] == i {
                    Some(&p[1..])
                } else {
                    None
                }
            });
            self.render_at_depth(arg, current_x, y, depth, arg_cursor, cursor_offset);

            let arg_layout = self.layout_engine.layout(arg, self.ctx);
            current_x += arg_layout.width;
        }

        // Draw closing paren
        self.draw_text(")", current_x, y, font_size, false);
    }

    fn render_factorial(
        &self,
        arg: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
        cursor_offset: usize,
    ) {
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;
        let gap = (font_size * 0.06).max(0.6);

        let arg_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(arg, x, y, depth, arg_cursor, cursor_offset);

        let arg_layout = self.layout_engine.layout_with_depth(arg, self.ctx, depth);
        self.draw_text("!", x + arg_layout.width + gap, y, font_size, false);
    }

    /// Render absolute value
    fn render_abs(
        &self,
        inner: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
        cursor_offset: usize,
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
        self.ctx
            .move_to(x + bar_width / 2.0, y - inner_layout.ascent);
        self.ctx
            .line_to(x + bar_width / 2.0, y + inner_layout.descent);
        self.ctx.stroke().unwrap();

        // Right bar
        let right_x = x + bar_width + inner_layout.width + bar_width / 2.0;
        self.ctx.move_to(right_x, y - inner_layout.ascent);
        self.ctx.line_to(right_x, y + inner_layout.descent);
        self.ctx.stroke().unwrap();

        self.ctx.restore().unwrap();

        // Draw inner expression
        let inner_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(inner, x + bar_width, y, depth, inner_cursor, cursor_offset);
    }

    /// Render parenthesized expression
    fn render_parens(
        &self,
        inner: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
        cursor_offset: usize,
    ) {
        let inner_layout = self.layout_engine.layout(inner, self.ctx);
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;
        let paren_width = font_size * 0.25;

        // For now, draw text parentheses (could be replaced with curved paths)
        self.draw_text("(", x, y, font_size * 1.2, false);
        self.draw_text(
            ")",
            x + paren_width + inner_layout.width,
            y,
            font_size * 1.2,
            false,
        );

        let inner_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(
            inner,
            x + paren_width,
            y,
            depth,
            inner_cursor,
            cursor_offset,
        );
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
        cursor_offset: usize,
    ) {
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;
        let body_layout = self.layout_engine.layout_with_depth(body, self.ctx, depth);

        let symbol_size = body_layout.height().max(font_size * 1.8);
        self.ctx.set_font_size(symbol_size);
        let int_extents = self.ctx.text_extents("∫").unwrap();
        let int_font_extents = self.ctx.font_extents().unwrap();
        let symbol_width = int_extents.x_advance();
        let symbol_ascent = int_font_extents.ascent();
        let symbol_descent = int_font_extents.descent();

        let lo_layout = lower.map(|lo| {
            self.layout_engine
                .layout_with_depth(lo, self.ctx, depth + 1)
        });
        let hi_layout = upper.map(|hi| {
            self.layout_engine
                .layout_with_depth(hi, self.ctx, depth + 1)
        });

        let bound_gap = font_size * 0.14;
        let bounds_width = symbol_width
            .max(lo_layout.as_ref().map(|l| l.width).unwrap_or(0.0))
            .max(hi_layout.as_ref().map(|l| l.width).unwrap_or(0.0));
        let body_gap = font_size * 0.22;
        let dx_gap = font_size * 0.14;

        // Draw integral symbol centered in bounds column
        self.ctx.save().unwrap();
        self.set_color(&self.fg_color);
        self.ctx.set_font_size(symbol_size);
        let symbol_x = x + (bounds_width - symbol_width) / 2.0;
        let symbol_baseline = y + (symbol_ascent - symbol_descent) * 0.5;
        self.ctx.move_to(symbol_x, symbol_baseline);
        self.ctx.show_text("∫").unwrap();
        self.ctx.restore().unwrap();

        let mut child_idx = 0;

        // Draw lower bound
        if let (Some(lo), Some(lo_layout)) = (lower, lo_layout.as_ref()) {
            let lo_cursor = cursor_path.and_then(|p| {
                if !p.is_empty() && p[0] == child_idx {
                    Some(&p[1..])
                } else {
                    None
                }
            });
            let lo_x = x + (bounds_width - lo_layout.width) / 2.0;
            let lo_baseline = y + symbol_descent + bound_gap + lo_layout.ascent;
            self.render_at_depth(lo, lo_x, lo_baseline, depth + 1, lo_cursor, cursor_offset);
            child_idx += 1;
        }

        // Draw upper bound
        if let (Some(hi), Some(hi_layout)) = (upper, hi_layout.as_ref()) {
            let hi_cursor = cursor_path.and_then(|p| {
                if !p.is_empty() && p[0] == child_idx {
                    Some(&p[1..])
                } else {
                    None
                }
            });
            let hi_x = x + (bounds_width - hi_layout.width) / 2.0;
            let hi_baseline = y - symbol_ascent - bound_gap - hi_layout.descent;
            self.render_at_depth(hi, hi_x, hi_baseline, depth + 1, hi_cursor, cursor_offset);
            child_idx += 1;
        }

        // Draw body
        let body_x = x + bounds_width + body_gap;
        let body_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == child_idx {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(body, body_x, y, depth, body_cursor, cursor_offset);
        child_idx += 1;

        // Draw "dx"
        let d_width = self.text_advance("d", font_size, false);
        let d_x = body_x + body_layout.width + dx_gap;
        self.draw_text("d", d_x, y, font_size, false);

        let var_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == child_idx {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(var, d_x + d_width, y, depth, var_cursor, cursor_offset);
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
        cursor_offset: usize,
    ) {
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;
        let frac_font_size = font_size * 0.8;

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

        // Measure text and guard spacing
        let num_width = self.text_advance(&num_str, frac_font_size, false);
        let den_prefix_width = self.text_advance(&den_prefix, frac_font_size, false);
        self.ctx.set_font_size(frac_font_size);
        let frac_font_extents = self.ctx.font_extents().unwrap();
        let frac_ascent = frac_font_extents.ascent();
        let frac_descent = frac_font_extents.descent();

        let var_layout = self
            .layout_engine
            .layout_with_depth(var, self.ctx, depth + 1);
        let den_sep = (frac_font_size * 0.08).max(0.6);
        let denom_width = den_prefix_width + den_sep + var_layout.width;
        let frac_width = num_width.max(denom_width) + font_size * 0.18;
        let bar_gap = font_size * 0.14;
        let body_gap = font_size * 0.3;

        // Draw numerator
        self.draw_text(
            &num_str,
            x + (frac_width - num_width) / 2.0,
            y - bar_gap - frac_descent,
            frac_font_size,
            false,
        );

        // Draw fraction bar
        self.ctx.save().unwrap();
        self.set_color(&self.fg_color);
        self.ctx.set_line_width(1.0 * scale);
        self.ctx.move_to(x, y);
        self.ctx.line_to(x + frac_width, y);
        self.ctx.stroke().unwrap();
        self.ctx.restore().unwrap();

        // Draw denominator
        let den_x = x + (frac_width - denom_width) / 2.0;
        let den_baseline = y + bar_gap + frac_ascent.max(var_layout.ascent);
        self.draw_text(&den_prefix, den_x, den_baseline, frac_font_size, false);

        let var_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(
            var,
            den_x + den_prefix_width + den_sep,
            den_baseline,
            depth + 1,
            var_cursor,
            cursor_offset,
        );

        // Draw body
        let body_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 1 {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(
            body,
            x + frac_width + body_gap,
            y,
            depth,
            body_cursor,
            cursor_offset,
        );
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
        cursor_offset: usize,
    ) {
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;
        let sub_font_size = font_size * 0.7;
        let body_gap = font_size * 0.3;

        let lim_width = self.text_advance("lim", font_size, false);
        let var_layout = self
            .layout_engine
            .layout_with_depth(var, self.ctx, depth + 1);
        let to_layout = self
            .layout_engine
            .layout_with_depth(to, self.ctx, depth + 1);
        let arrow_width = self.text_advance("→", sub_font_size, false);
        let dir_width = if direction.is_some() {
            self.text_advance("⁺", sub_font_size * 0.6, false)
        } else {
            0.0
        };
        let sub_sep = (sub_font_size * 0.08).max(0.5);
        let subscript_width =
            var_layout.width + sub_sep + arrow_width + sub_sep + to_layout.width + dir_width;
        let lim_col_width = lim_width.max(subscript_width);
        let lim_x = x + (lim_col_width - lim_width) / 2.0;
        let sub_start_x = x + (lim_col_width - subscript_width) / 2.0;
        let subscript_y = y + font_size * 0.2 + var_layout.ascent.max(to_layout.ascent);

        // Draw "lim"
        self.draw_text("lim", lim_x, y, font_size, false);

        let var_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(
            var,
            sub_start_x,
            subscript_y,
            depth + 1,
            var_cursor,
            cursor_offset,
        );

        let arrow_x = sub_start_x + var_layout.width + sub_sep;
        self.draw_text("→", arrow_x, subscript_y, sub_font_size, false);

        let to_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 1 {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(
            to,
            arrow_x + arrow_width + sub_sep,
            subscript_y,
            depth + 1,
            to_cursor,
            cursor_offset,
        );

        // Draw direction indicator if present
        if let Some(dir) = direction {
            let dir_str = match dir {
                LimitDirection::FromRight => "⁺",
                LimitDirection::FromLeft => "⁻",
            };
            self.draw_text(
                dir_str,
                arrow_x + arrow_width + sub_sep + to_layout.width,
                subscript_y - sub_font_size * 0.3,
                sub_font_size * 0.6,
                false,
            );
        }

        // Draw body
        let body_x = x + lim_col_width + body_gap;
        let body_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 2 {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(body, body_x, y, depth, body_cursor, cursor_offset);
    }

    /// Render a big operator (sum, product)
    fn render_bigop(
        &self,
        symbol: &str,
        var: &MathBox,
        lower: &MathBox,
        upper: &MathBox,
        body: &MathBox,
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
        cursor_offset: usize,
    ) {
        let scale = self.scale_for_depth(depth);
        let font_size = self.layout_engine.base_font_size * scale;
        let symbol_size = font_size * 1.5;
        let bound_scale = self.scale_for_depth(depth + 1);
        let bound_font_size = self.layout_engine.base_font_size * bound_scale;

        let var_layout = self
            .layout_engine
            .layout_with_depth(var, self.ctx, depth + 1);
        let lower_layout = self
            .layout_engine
            .layout_with_depth(lower, self.ctx, depth + 1);
        let upper_layout = self
            .layout_engine
            .layout_with_depth(upper, self.ctx, depth + 1);
        let eq_width = self.text_advance("=", bound_font_size, false);
        let lower_sep = (bound_font_size * 0.08).max(0.6);
        let lower_block_width =
            var_layout.width + lower_sep + eq_width + lower_sep + lower_layout.width;

        // Draw the big symbol centered in the operator column.
        self.ctx.save().unwrap();
        self.set_color(&self.fg_color);
        self.ctx.set_font_size(symbol_size);
        let symbol_extents = self.ctx.text_extents(symbol).unwrap();
        let symbol_width = symbol_extents.x_advance();
        let symbol_font_extents = self.ctx.font_extents().unwrap();
        let symbol_ascent = symbol_font_extents.ascent();
        let symbol_descent = symbol_font_extents.descent();
        let op_width = symbol_width.max(lower_block_width).max(upper_layout.width);
        let symbol_x = x + (op_width - symbol_width) / 2.0;
        let symbol_baseline = y + (symbol_ascent - symbol_descent) * 0.5;
        self.ctx.move_to(symbol_x, symbol_baseline);
        self.ctx.show_text(symbol).unwrap();
        self.ctx.restore().unwrap();

        let bounds_gap = font_size * 0.16;
        let body_gap = font_size * 0.32;
        let symbol_top = symbol_baseline - symbol_ascent;
        let symbol_bottom = symbol_baseline + symbol_descent;
        let upper_baseline = symbol_top - bounds_gap - upper_layout.descent;
        let lower_baseline =
            symbol_bottom + bounds_gap + lower_layout.ascent.max(var_layout.ascent);
        let upper_x = x + (op_width - upper_layout.width) / 2.0;
        let lower_start_x = x + (op_width - lower_block_width) / 2.0;

        // Draw upper bound
        let upper_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 2 {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(
            upper,
            upper_x,
            upper_baseline,
            depth + 1,
            upper_cursor,
            cursor_offset,
        );

        // Draw lower bound as "var = lower"
        let var_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 0 {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(
            var,
            lower_start_x,
            lower_baseline,
            depth + 1,
            var_cursor,
            cursor_offset,
        );

        let eq_x = lower_start_x + var_layout.width + lower_sep;
        self.draw_text("=", eq_x, lower_baseline, bound_font_size, false);

        let lower_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 1 {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(
            lower,
            eq_x + eq_width + lower_sep,
            lower_baseline,
            depth + 1,
            lower_cursor,
            cursor_offset,
        );

        // Draw body
        let body_x = x + op_width + body_gap;
        let body_cursor = cursor_path.and_then(|p| {
            if !p.is_empty() && p[0] == 3 {
                Some(&p[1..])
            } else {
                None
            }
        });
        self.render_at_depth(body, body_x, y, depth, body_cursor, cursor_offset);
    }

    /// Render a matrix
    fn render_matrix(
        &self,
        rows: &[Vec<MathBox>],
        x: f64,
        y: f64,
        depth: u32,
        cursor_path: Option<&[usize]>,
        cursor_offset: usize,
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
        let total_height: f64 =
            row_heights.iter().sum::<f64>() + cell_padding * (rows.len() as f64 - 1.0);

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
        self.ctx
            .move_to(right_x - bracket_gap, y - total_height / 2.0);
        self.ctx.line_to(right_x, y - total_height / 2.0);
        self.ctx.line_to(right_x, y + total_height / 2.0);
        self.ctx
            .line_to(right_x - bracket_gap, y + total_height / 2.0);
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
                    if !p.is_empty() && p[0] == cell_idx {
                        Some(&p[1..])
                    } else {
                        None
                    }
                });
                self.render_at_depth(cell, cx, cy, depth, cell_cursor, cursor_offset);

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
        cursor_offset: usize,
    ) {
        let mut current_x = x;

        for (i, item) in items.iter().enumerate() {
            let item_cursor = cursor_path.and_then(|p| {
                if !p.is_empty() && p[0] == i {
                    Some(&p[1..])
                } else {
                    None
                }
            });
            self.render_at_depth(item, current_x, y, depth, item_cursor, cursor_offset);

            let layout = self.layout_engine.layout(item, self.ctx);
            current_x += layout.width;
        }
    }

    /// Set the current drawing color
    fn set_color(&self, color: &Color) {
        self.ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    }

    fn slot_geometry(font_size: f64) -> (f64, f64, f64) {
        let width = (font_size * 0.86).max(10.0);
        let height = (font_size * 0.9).max(11.0);
        let ascent = height * 0.62;
        (width, height, ascent)
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
