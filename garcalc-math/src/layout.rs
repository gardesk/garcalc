//! Math layout engine
//!
//! Computes bounding boxes and positions for mathematical typesetting.
//! Uses baseline-aligned layout with proper ascent/descent metrics.

use crate::mathbox::{LimitDirection, MathBox, Operator};
use cairo::Context;

/// Layout metrics for a rendered element
#[derive(Debug, Clone, Default)]
pub struct LayoutBox {
    /// Total width of the element
    pub width: f64,
    /// Height above the baseline
    pub ascent: f64,
    /// Depth below the baseline
    pub descent: f64,
    /// Positioned children: (x_offset, y_offset, child_layout)
    pub children: Vec<(f64, f64, LayoutBox)>,
}

impl LayoutBox {
    /// Total height (ascent + descent)
    pub fn height(&self) -> f64 {
        self.ascent + self.descent
    }

    /// Create an empty layout box
    pub fn empty() -> Self {
        Self::default()
    }
}

/// Layout engine for mathematical typesetting
pub struct MathLayoutEngine {
    /// Base font size in points
    pub base_font_size: f64,
    /// Font family name
    pub font_family: String,
}

impl Default for MathLayoutEngine {
    fn default() -> Self {
        Self {
            base_font_size: 16.0,
            font_family: "serif".to_string(),
        }
    }
}

impl MathLayoutEngine {
    /// Create a new layout engine with given font settings
    pub fn new(font_family: &str, font_size: f64) -> Self {
        Self {
            base_font_size: font_size,
            font_family: font_family.to_string(),
        }
    }

    /// Compute layout for a MathBox tree
    pub fn layout(&self, mathbox: &MathBox, ctx: &Context) -> LayoutBox {
        self.layout_at_depth(mathbox, ctx, 0)
    }

    /// Compute layout from an explicit starting depth
    pub fn layout_with_depth(&self, mathbox: &MathBox, ctx: &Context, depth: u32) -> LayoutBox {
        self.layout_at_depth(mathbox, ctx, depth)
    }

    /// Compute layout at a specific nesting depth
    fn layout_at_depth(&self, mathbox: &MathBox, ctx: &Context, depth: u32) -> LayoutBox {
        let scale = self.scale_for_depth(depth);
        let font_size = self.base_font_size * scale;

        match mathbox {
            MathBox::Number(s) => self.layout_text(s, ctx, font_size),
            MathBox::Symbol(s) => self.layout_symbol(s, ctx, font_size),
            MathBox::Operator(op) => self.layout_operator(*op, ctx, font_size),
            MathBox::Slot => self.layout_slot(ctx, font_size),
            MathBox::Fraction { num, den } => self.layout_fraction(num, den, ctx, depth),
            MathBox::Power { base, exp } => self.layout_power(base, exp, ctx, depth),
            MathBox::Subscript { base, sub } => self.layout_subscript(base, sub, ctx, depth),
            MathBox::Root { index, radicand } => {
                self.layout_root(index.as_deref(), radicand, ctx, depth)
            }
            MathBox::Func { name, args } => self.layout_func(name, args, ctx, depth),
            MathBox::Abs(inner) => self.layout_abs(inner, ctx, depth),
            MathBox::Parens(inner) => self.layout_parens(inner, ctx, depth),
            MathBox::Integral {
                lower,
                upper,
                body,
                var,
            } => self.layout_integral(lower.as_deref(), upper.as_deref(), body, var, ctx, depth),
            MathBox::Derivative { order, var, body } => {
                self.layout_derivative(*order, var, body, ctx, depth)
            }
            MathBox::Limit {
                var,
                to,
                direction,
                body,
            } => self.layout_limit(var, to, *direction, body, ctx, depth),
            MathBox::Sum {
                var,
                lower,
                upper,
                body,
            } => self.layout_bigop("∑", var, lower, upper, body, ctx, depth),
            MathBox::Product {
                var,
                lower,
                upper,
                body,
            } => self.layout_bigop("∏", var, lower, upper, body, ctx, depth),
            MathBox::Matrix { rows } => self.layout_matrix(rows, ctx, depth),
            MathBox::Row(items) => self.layout_row(items, ctx, depth),
        }
    }

    /// Scale factor for nested elements
    pub fn scale_for_depth(&self, depth: u32) -> f64 {
        match depth {
            0 => 1.0,
            1 => 0.8,
            _ => 0.65,
        }
    }

    /// Layout plain text
    fn layout_text(&self, text: &str, ctx: &Context, font_size: f64) -> LayoutBox {
        ctx.set_font_size(font_size);
        let extents = ctx.text_extents(text).unwrap();
        let font_extents = ctx.font_extents().unwrap();

        LayoutBox {
            width: extents.x_advance(),
            ascent: font_extents.ascent(),
            descent: font_extents.descent(),
            children: vec![],
        }
    }

    /// Layout a symbol (may use italic)
    fn layout_symbol(&self, symbol: &str, ctx: &Context, font_size: f64) -> LayoutBox {
        ctx.select_font_face(
            &self.font_family,
            cairo::FontSlant::Italic,
            cairo::FontWeight::Normal,
        );
        let layout = self.layout_text(symbol, ctx, font_size);
        ctx.select_font_face(
            &self.font_family,
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
        );
        layout
    }

    /// Layout an operator
    fn layout_operator(&self, op: Operator, ctx: &Context, font_size: f64) -> LayoutBox {
        let ch = op.as_char().to_string();
        let mut layout = self.layout_text(&ch, ctx, font_size);
        // Add padding around operators
        layout.width += font_size * 0.3;
        layout
    }

    /// Layout an empty slot (placeholder box)
    fn layout_slot(&self, _ctx: &Context, font_size: f64) -> LayoutBox {
        // Keep slots readable at nested depths by enforcing a minimum visual size.
        let slot_width = (font_size * 0.86).max(10.0);
        let slot_height = (font_size * 0.9).max(11.0);

        LayoutBox {
            width: slot_width,
            ascent: slot_height * 0.62,
            descent: slot_height * 0.38,
            children: vec![],
        }
    }

    /// Layout a fraction
    fn layout_fraction(
        &self,
        num: &MathBox,
        den: &MathBox,
        ctx: &Context,
        depth: u32,
    ) -> LayoutBox {
        let num_layout = self.layout_at_depth(num, ctx, depth + 1);
        let den_layout = self.layout_at_depth(den, ctx, depth + 1);

        let scale = self.scale_for_depth(depth);
        let bar_thickness = 1.0 * scale;
        let gap = self.base_font_size * 0.1 * scale;

        let width = num_layout.width.max(den_layout.width) + self.base_font_size * 0.2;
        let num_x = (width - num_layout.width) / 2.0;
        let den_x = (width - den_layout.width) / 2.0;

        // Position numerator above bar, denominator below
        let num_y = -(gap + bar_thickness / 2.0 + num_layout.descent);
        let den_y = gap + bar_thickness / 2.0 + den_layout.ascent;

        LayoutBox {
            width,
            ascent: gap + bar_thickness / 2.0 + num_layout.height(),
            descent: gap + bar_thickness / 2.0 + den_layout.height(),
            children: vec![(num_x, num_y, num_layout), (den_x, den_y, den_layout)],
        }
    }

    /// Layout a power (superscript)
    fn layout_power(&self, base: &MathBox, exp: &MathBox, ctx: &Context, depth: u32) -> LayoutBox {
        let base_layout = self.layout_at_depth(base, ctx, depth);
        let exp_layout = self.layout_at_depth(exp, ctx, depth + 1);
        let scale = self.scale_for_depth(depth);
        let font_size = self.base_font_size * scale;

        // Exponent is raised above the baseline and slightly kerned to the right.
        let exp_raise = base_layout.ascent * 0.58 + exp_layout.descent * 0.1;
        let exp_kern = (font_size * 0.06).max(0.8);

        LayoutBox {
            width: base_layout.width + exp_kern + exp_layout.width,
            ascent: base_layout.ascent.max(exp_raise + exp_layout.ascent),
            descent: base_layout.descent,
            children: vec![
                (0.0, 0.0, base_layout.clone()),
                (base_layout.width + exp_kern, -exp_raise, exp_layout),
            ],
        }
    }

    /// Layout a subscript
    fn layout_subscript(
        &self,
        base: &MathBox,
        sub: &MathBox,
        ctx: &Context,
        depth: u32,
    ) -> LayoutBox {
        let base_layout = self.layout_at_depth(base, ctx, depth);
        let sub_layout = self.layout_at_depth(sub, ctx, depth + 1);
        let scale = self.scale_for_depth(depth);
        let font_size = self.base_font_size * scale;

        // Subscript is lowered with a small right kern to avoid touching the base.
        let sub_lower =
            (base_layout.descent * 0.6 + sub_layout.ascent * 0.9).max(sub_layout.ascent * 0.75);
        let sub_kern = (font_size * 0.05).max(0.6);

        LayoutBox {
            width: base_layout.width + sub_kern + sub_layout.width,
            ascent: base_layout
                .ascent
                .max((sub_layout.ascent - sub_lower).max(0.0)),
            descent: base_layout.descent.max(sub_lower + sub_layout.descent),
            children: vec![
                (0.0, 0.0, base_layout.clone()),
                (base_layout.width + sub_kern, sub_lower, sub_layout),
            ],
        }
    }

    /// Layout a root (square or nth)
    fn layout_root(
        &self,
        index: Option<&MathBox>,
        radicand: &MathBox,
        ctx: &Context,
        depth: u32,
    ) -> LayoutBox {
        let radicand_layout = self.layout_at_depth(radicand, ctx, depth);
        let scale = self.scale_for_depth(depth);
        let font_size = self.base_font_size * scale;

        // Radical symbol width
        let radical_width = font_size * 0.6;
        let bar_overhang = font_size * 0.1;
        let gap = font_size * 0.1;

        let mut total_width = radical_width + radicand_layout.width + bar_overhang;
        let mut children = vec![];

        // Handle nth root index
        let index_width = if let Some(idx) = index {
            let idx_layout = self.layout_at_depth(idx, ctx, depth + 2);
            let idx_x = 0.0;
            let idx_y = -(radicand_layout.ascent * 0.5);
            children.push((idx_x, idx_y, idx_layout.clone()));
            idx_layout.width
        } else {
            0.0
        };

        total_width += index_width;

        // Radicand position
        let radicand_x = index_width + radical_width;
        children.push((radicand_x, 0.0, radicand_layout.clone()));

        LayoutBox {
            width: total_width,
            ascent: radicand_layout.ascent + gap + 1.0, // +1 for bar
            descent: radicand_layout.descent,
            children,
        }
    }

    /// Layout a function call
    fn layout_func(&self, name: &str, args: &[MathBox], ctx: &Context, depth: u32) -> LayoutBox {
        if name == "factorial" && args.len() == 1 {
            return self.layout_factorial(&args[0], ctx, depth);
        }

        let scale = self.scale_for_depth(depth);
        let font_size = self.base_font_size * scale;

        // Function name
        let name_layout = self.layout_text(name, ctx, font_size);
        let paren_width = font_size * 0.3;

        let mut width = name_layout.width + paren_width; // opening paren
        let mut max_ascent = name_layout.ascent;
        let mut max_descent = name_layout.descent;
        let mut children = vec![(0.0, 0.0, name_layout.clone())];

        let mut x = name_layout.width + paren_width;

        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                // Comma separator
                let comma_layout = self.layout_text(",", ctx, font_size);
                x += comma_layout.width;
                width += comma_layout.width;
            }

            let arg_layout = self.layout_at_depth(arg, ctx, depth);
            max_ascent = max_ascent.max(arg_layout.ascent);
            max_descent = max_descent.max(arg_layout.descent);
            children.push((x, 0.0, arg_layout.clone()));
            x += arg_layout.width;
            width += arg_layout.width;
        }

        width += paren_width; // closing paren

        LayoutBox {
            width,
            ascent: max_ascent,
            descent: max_descent,
            children,
        }
    }

    fn layout_factorial(&self, arg: &MathBox, ctx: &Context, depth: u32) -> LayoutBox {
        let scale = self.scale_for_depth(depth);
        let font_size = self.base_font_size * scale;
        let gap = (font_size * 0.06).max(0.6);

        let arg_layout = self.layout_at_depth(arg, ctx, depth);
        let arg_width = arg_layout.width;
        let bang_layout = self.layout_text("!", ctx, font_size);

        LayoutBox {
            width: arg_width + gap + bang_layout.width,
            ascent: arg_layout.ascent.max(bang_layout.ascent),
            descent: arg_layout.descent.max(bang_layout.descent),
            children: vec![(0.0, 0.0, arg_layout), (arg_width + gap, 0.0, bang_layout)],
        }
    }

    /// Layout absolute value
    fn layout_abs(&self, inner: &MathBox, ctx: &Context, depth: u32) -> LayoutBox {
        let inner_layout = self.layout_at_depth(inner, ctx, depth);
        let scale = self.scale_for_depth(depth);
        let bar_width = self.base_font_size * 0.15 * scale;

        LayoutBox {
            width: inner_layout.width + 2.0 * bar_width,
            ascent: inner_layout.ascent,
            descent: inner_layout.descent,
            children: vec![(bar_width, 0.0, inner_layout)],
        }
    }

    /// Layout parenthesized expression
    fn layout_parens(&self, inner: &MathBox, ctx: &Context, depth: u32) -> LayoutBox {
        let inner_layout = self.layout_at_depth(inner, ctx, depth);
        let scale = self.scale_for_depth(depth);
        let paren_width = self.base_font_size * 0.25 * scale;

        LayoutBox {
            width: inner_layout.width + 2.0 * paren_width,
            ascent: inner_layout.ascent + 2.0,
            descent: inner_layout.descent + 2.0,
            children: vec![(paren_width, 0.0, inner_layout)],
        }
    }

    /// Layout an integral
    fn layout_integral(
        &self,
        lower: Option<&MathBox>,
        upper: Option<&MathBox>,
        body: &MathBox,
        var: &MathBox,
        ctx: &Context,
        depth: u32,
    ) -> LayoutBox {
        let scale = self.scale_for_depth(depth);
        let font_size = self.base_font_size * scale;

        let body_layout = self.layout_at_depth(body, ctx, depth);
        let var_layout = self.layout_at_depth(var, ctx, depth);
        let d_layout = self.layout_text("d", ctx, font_size);

        // Integral symbol sizing and metrics
        let symbol_size = body_layout.height().max(font_size * 1.8);
        ctx.set_font_size(symbol_size);
        let int_extents = ctx.text_extents("∫").unwrap();
        let int_font_extents = ctx.font_extents().unwrap();
        let symbol_width = int_extents.x_advance();
        let symbol_ascent = int_font_extents.ascent();
        let symbol_descent = int_font_extents.descent();

        let lo_layout = lower.map(|lo| self.layout_at_depth(lo, ctx, depth + 1));
        let hi_layout = upper.map(|hi| self.layout_at_depth(hi, ctx, depth + 1));

        let bound_gap = font_size * 0.14;
        let bounds_width = symbol_width
            .max(lo_layout.as_ref().map(|l| l.width).unwrap_or(0.0))
            .max(hi_layout.as_ref().map(|l| l.width).unwrap_or(0.0));
        let body_gap = font_size * 0.22;
        let dx_gap = font_size * 0.14;

        let mut children = vec![];
        let body_x = bounds_width + body_gap;
        children.push((body_x, 0.0, body_layout.clone()));

        let d_x = body_x + body_layout.width + dx_gap;
        children.push((d_x, 0.0, d_layout.clone()));
        let var_x = d_x + d_layout.width;
        children.push((var_x, 0.0, var_layout.clone()));

        let ascent =
            symbol_ascent + bound_gap + hi_layout.as_ref().map(|l| l.height()).unwrap_or(0.0);
        let descent =
            symbol_descent + bound_gap + lo_layout.as_ref().map(|l| l.height()).unwrap_or(0.0);

        LayoutBox {
            width: var_x + var_layout.width,
            ascent: ascent.max(body_layout.ascent),
            descent: descent.max(body_layout.descent),
            children,
        }
    }

    /// Layout a derivative
    fn layout_derivative(
        &self,
        order: u32,
        var: &MathBox,
        body: &MathBox,
        ctx: &Context,
        depth: u32,
    ) -> LayoutBox {
        let scale = self.scale_for_depth(depth);
        let font_size = self.base_font_size * scale;
        let frac_font_size = font_size * 0.8;

        let var_layout = self.layout_at_depth(var, ctx, depth + 1);
        let body_layout = self.layout_at_depth(body, ctx, depth);

        // Build "d/dx" or "d²/dx²"
        let d_str = if order > 1 {
            format!("d{}", superscript_digits(order))
        } else {
            "d".to_string()
        };

        let dx_str = if order > 1 {
            format!("d{}", superscript_digits(order))
        } else {
            "d".to_string()
        };

        let d_layout = self.layout_text(&d_str, ctx, frac_font_size);
        let dx_layout = self.layout_text(&dx_str, ctx, frac_font_size);
        let den_sep = (frac_font_size * 0.08).max(0.6);
        let denom_width = dx_layout.width + den_sep + var_layout.width;
        let denom_height = dx_layout.height().max(var_layout.height());
        let frac_width = d_layout.width.max(denom_width) + font_size * 0.18;
        let bar_gap = font_size * 0.14;
        let body_gap = font_size * 0.3;

        LayoutBox {
            width: frac_width + body_gap + body_layout.width,
            ascent: (bar_gap + d_layout.height()).max(body_layout.ascent),
            descent: (bar_gap + denom_height).max(body_layout.descent),
            children: vec![(frac_width + body_gap, 0.0, body_layout)],
        }
    }

    /// Layout a limit
    fn layout_limit(
        &self,
        var: &MathBox,
        to: &MathBox,
        direction: Option<LimitDirection>,
        body: &MathBox,
        ctx: &Context,
        depth: u32,
    ) -> LayoutBox {
        let scale = self.scale_for_depth(depth);
        let font_size = self.base_font_size * scale;

        let body_layout = self.layout_at_depth(body, ctx, depth);
        let var_layout = self.layout_at_depth(var, ctx, depth + 1);
        let to_layout = self.layout_at_depth(to, ctx, depth + 1);

        // "lim" text
        let lim_layout = self.layout_text("lim", ctx, font_size);
        let lim_width = lim_layout.width;

        // Build subscript: "x→a" or "x→a⁺" or "x→a⁻"
        let sub_font_size = font_size * 0.7;
        let arrow_layout = self.layout_text("→", ctx, sub_font_size);
        let dir_str = match direction {
            Some(LimitDirection::FromRight) => "⁺",
            Some(LimitDirection::FromLeft) => "⁻",
            None => "",
        };
        let dir_layout = if !dir_str.is_empty() {
            Some(self.layout_text(dir_str, ctx, sub_font_size * 0.6))
        } else {
            None
        };

        let sub_sep = (sub_font_size * 0.08).max(0.5);
        let subscript_width = var_layout.width
            + sub_sep
            + arrow_layout.width
            + sub_sep
            + to_layout.width
            + dir_layout.as_ref().map(|l| l.width).unwrap_or(0.0);
        let subscript_height = var_layout
            .height()
            .max(arrow_layout.height())
            .max(to_layout.height());

        let lim_col_width = lim_width.max(subscript_width);
        let body_gap = font_size * 0.3;
        let subscript_drop = font_size * 0.2 + subscript_height;

        LayoutBox {
            width: lim_col_width + body_gap + body_layout.width,
            ascent: lim_layout.ascent.max(body_layout.ascent),
            descent: subscript_drop.max(body_layout.descent),
            children: vec![(lim_col_width + body_gap, 0.0, body_layout)],
        }
    }

    /// Layout a big operator (sum, product)
    fn layout_bigop(
        &self,
        symbol: &str,
        var: &MathBox,
        lower: &MathBox,
        upper: &MathBox,
        body: &MathBox,
        ctx: &Context,
        depth: u32,
    ) -> LayoutBox {
        let scale = self.scale_for_depth(depth);
        let font_size = self.base_font_size * scale;

        let body_layout = self.layout_at_depth(body, ctx, depth);
        let var_layout = self.layout_at_depth(var, ctx, depth + 1);
        let lower_layout = self.layout_at_depth(lower, ctx, depth + 1);
        let upper_layout = self.layout_at_depth(upper, ctx, depth + 1);

        // Big operator symbol
        let symbol_size = font_size * 1.5;
        ctx.set_font_size(symbol_size);
        let symbol_extents = ctx.text_extents(symbol).unwrap();
        let symbol_width = symbol_extents.x_advance();
        let symbol_font_extents = ctx.font_extents().unwrap();
        let symbol_ascent = symbol_font_extents.ascent();
        let symbol_descent = symbol_font_extents.descent();

        let bound_font_size = self.base_font_size * self.scale_for_depth(depth + 1);
        let eq_layout = self.layout_text("=", ctx, bound_font_size);
        let lower_sep = (bound_font_size * 0.08).max(0.6);
        let lower_block_width =
            var_layout.width + lower_sep + eq_layout.width + lower_sep + lower_layout.width;

        let op_width = symbol_width.max(lower_block_width).max(upper_layout.width);
        let bounds_gap = font_size * 0.16;
        let body_gap = font_size * 0.32;
        // Rendering centers big-op symbols around the expression baseline.
        // Reserve half of total glyph height above and below for consistent placement.
        let symbol_half_height = (symbol_ascent + symbol_descent) * 0.5;

        let ascent =
            (symbol_half_height + bounds_gap + upper_layout.height()).max(body_layout.ascent);
        let descent =
            (symbol_half_height + bounds_gap + lower_layout.height().max(var_layout.height()))
                .max(body_layout.descent);

        LayoutBox {
            width: op_width + body_gap + body_layout.width,
            ascent,
            descent,
            children: vec![(op_width + body_gap, 0.0, body_layout)],
        }
    }

    /// Layout a matrix
    fn layout_matrix(&self, rows: &[Vec<MathBox>], ctx: &Context, depth: u32) -> LayoutBox {
        let scale = self.scale_for_depth(depth);
        let font_size = self.base_font_size * scale;
        let cell_padding = font_size * 0.3;

        if rows.is_empty() {
            return LayoutBox::empty();
        }

        let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        if num_cols == 0 {
            return LayoutBox::empty();
        }

        // Compute layouts for all cells
        let cell_layouts: Vec<Vec<LayoutBox>> = rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| self.layout_at_depth(cell, ctx, depth))
                    .collect()
            })
            .collect();

        // Find max width per column and max height per row
        let mut col_widths = vec![0.0f64; num_cols];
        let mut row_heights = vec![0.0f64; rows.len()];

        for (r, row) in cell_layouts.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                col_widths[c] = col_widths[c].max(cell.width);
                row_heights[r] = row_heights[r].max(cell.height());
            }
        }

        // Total dimensions
        let bracket_width = font_size * 0.2;
        let total_width: f64 = col_widths.iter().sum::<f64>()
            + cell_padding * (num_cols as f64 - 1.0)
            + 2.0 * bracket_width;
        let total_height: f64 =
            row_heights.iter().sum::<f64>() + cell_padding * (rows.len() as f64 - 1.0);

        // Position cells
        let mut children = vec![];
        let mut y = -total_height / 2.0;

        for (r, row) in cell_layouts.iter().enumerate() {
            let mut x = bracket_width;
            let row_h = row_heights[r];

            for (c, cell) in row.iter().enumerate() {
                let col_w = col_widths[c];
                // Center cell in its column
                let cell_x = x + (col_w - cell.width) / 2.0;
                let cell_y = y + row_h / 2.0;
                children.push((cell_x, cell_y, cell.clone()));
                x += col_w + cell_padding;
            }

            y += row_h + cell_padding;
        }

        LayoutBox {
            width: total_width,
            ascent: total_height / 2.0 + cell_padding,
            descent: total_height / 2.0 + cell_padding,
            children,
        }
    }

    /// Layout a horizontal row of elements
    fn layout_row(&self, items: &[MathBox], ctx: &Context, depth: u32) -> LayoutBox {
        let scale = self.scale_for_depth(depth);
        let _font_size = self.base_font_size * scale;

        let mut width: f64 = 0.0;
        let mut max_ascent: f64 = 0.0;
        let mut max_descent: f64 = 0.0;
        let mut children = vec![];

        for item in items {
            let item_layout = self.layout_at_depth(item, ctx, depth);
            children.push((width, 0.0, item_layout.clone()));
            width += item_layout.width;
            max_ascent = max_ascent.max(item_layout.ascent);
            max_descent = max_descent.max(item_layout.descent);
        }

        LayoutBox {
            width,
            ascent: max_ascent,
            descent: max_descent,
            children,
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use cairo::{Context, Format, ImageSurface};

    fn test_context() -> Context {
        let surface = ImageSurface::create(Format::ARgb32, 256, 256).unwrap();
        Context::new(&surface).unwrap()
    }

    #[test]
    fn test_scale_for_depth() {
        let engine = MathLayoutEngine::default();
        assert_eq!(engine.scale_for_depth(0), 1.0);
        assert_eq!(engine.scale_for_depth(1), 0.8);
        assert_eq!(engine.scale_for_depth(2), 0.65);
        assert_eq!(engine.scale_for_depth(5), 0.65);
    }

    #[test]
    fn test_superscript_digits() {
        assert_eq!(superscript_digits(2), "²");
        assert_eq!(superscript_digits(123), "¹²³");
    }

    #[test]
    fn test_slot_min_size_at_nested_depth() {
        let engine = MathLayoutEngine::default();
        let ctx = test_context();
        let slot = engine.layout_with_depth(&MathBox::Slot, &ctx, 2);
        assert!(slot.width >= 10.0);
        assert!(slot.height() >= 11.0);
    }

    #[test]
    fn test_power_layout_raises_exponent() {
        let engine = MathLayoutEngine::default();
        let ctx = test_context();
        let power = MathBox::Power {
            base: Box::new(MathBox::Number("2".to_string())),
            exp: Box::new(MathBox::Slot),
        };
        let layout = engine.layout(&power, &ctx);
        let base_layout = engine.layout(&MathBox::Number("2".to_string()), &ctx);

        assert!(layout.width > base_layout.width);
        assert!(layout.ascent > base_layout.ascent);
        assert_eq!(layout.children.len(), 2);
        assert!(layout.children[1].1 < 0.0);
    }

    #[test]
    fn test_subscript_layout_lowers_and_extends_descent() {
        let engine = MathLayoutEngine::default();
        let ctx = test_context();
        let sub = MathBox::Subscript {
            base: Box::new(MathBox::Number("x".to_string())),
            sub: Box::new(MathBox::Slot),
        };
        let layout = engine.layout(&sub, &ctx);
        let base_layout = engine.layout(&MathBox::Number("x".to_string()), &ctx);

        assert!(layout.descent > base_layout.descent);
        assert_eq!(layout.children.len(), 2);
        assert!(layout.children[1].1 > 0.0);
    }

    #[test]
    fn test_bigop_layout_reserves_space_for_var_equals_lower() {
        let engine = MathLayoutEngine::default();
        let ctx = test_context();

        let short = MathBox::Sum {
            var: Box::new(MathBox::Symbol("i".to_string())),
            lower: Box::new(MathBox::Number("1".to_string())),
            upper: Box::new(MathBox::Number("5".to_string())),
            body: Box::new(MathBox::Slot),
        };
        let wide = MathBox::Sum {
            var: Box::new(MathBox::Symbol("index".to_string())),
            lower: Box::new(MathBox::Number("123456".to_string())),
            upper: Box::new(MathBox::Number("5".to_string())),
            body: Box::new(MathBox::Slot),
        };

        let short_layout = engine.layout(&short, &ctx);
        let wide_layout = engine.layout(&wide, &ctx);
        assert!(wide_layout.width > short_layout.width);
    }

    #[test]
    fn test_bigop_layout_expands_ascent_descent_for_bounds() {
        let engine = MathLayoutEngine::default();
        let ctx = test_context();

        let sum = MathBox::Sum {
            var: Box::new(MathBox::Symbol("i".to_string())),
            lower: Box::new(MathBox::Slot),
            upper: Box::new(MathBox::Slot),
            body: Box::new(MathBox::Number("x".to_string())),
        };
        let body_layout = engine.layout(&MathBox::Number("x".to_string()), &ctx);
        let sum_layout = engine.layout(&sum, &ctx);

        assert!(sum_layout.ascent > body_layout.ascent);
        assert!(sum_layout.descent > body_layout.descent);
    }

    #[test]
    fn test_derivative_layout_widens_for_long_variable() {
        let engine = MathLayoutEngine::default();
        let ctx = test_context();

        let short = MathBox::Derivative {
            order: 1,
            var: Box::new(MathBox::Symbol("x".to_string())),
            body: Box::new(MathBox::Slot),
        };
        let long = MathBox::Derivative {
            order: 1,
            var: Box::new(MathBox::Symbol("variable".to_string())),
            body: Box::new(MathBox::Slot),
        };

        let short_layout = engine.layout(&short, &ctx);
        let long_layout = engine.layout(&long, &ctx);
        assert!(long_layout.width > short_layout.width);
    }

    #[test]
    fn test_integral_layout_bounds_expand_vertical_space() {
        let engine = MathLayoutEngine::default();
        let ctx = test_context();

        let plain = MathBox::Integral {
            lower: None,
            upper: None,
            body: Box::new(MathBox::Slot),
            var: Box::new(MathBox::Symbol("x".to_string())),
        };
        let bounded = MathBox::Integral {
            lower: Some(Box::new(MathBox::Slot)),
            upper: Some(Box::new(MathBox::Slot)),
            body: Box::new(MathBox::Slot),
            var: Box::new(MathBox::Symbol("x".to_string())),
        };

        let plain_layout = engine.layout(&plain, &ctx);
        let bounded_layout = engine.layout(&bounded, &ctx);
        assert!(bounded_layout.ascent > plain_layout.ascent);
        assert!(bounded_layout.descent > plain_layout.descent);
    }

    #[test]
    fn test_limit_layout_widens_for_wide_subscript_and_extends_descent() {
        let engine = MathLayoutEngine::default();
        let ctx = test_context();

        let narrow = MathBox::Limit {
            var: Box::new(MathBox::Symbol("x".to_string())),
            to: Box::new(MathBox::Number("1".to_string())),
            direction: None,
            body: Box::new(MathBox::Slot),
        };
        let wide = MathBox::Limit {
            var: Box::new(MathBox::Symbol("veryLongVariable".to_string())),
            to: Box::new(MathBox::Number("123456".to_string())),
            direction: Some(LimitDirection::FromRight),
            body: Box::new(MathBox::Slot),
        };

        let narrow_layout = engine.layout(&narrow, &ctx);
        let wide_layout = engine.layout(&wide, &ctx);
        let body_layout = engine.layout(&MathBox::Slot, &ctx);

        assert!(wide_layout.width > narrow_layout.width);
        assert!(wide_layout.descent > body_layout.descent);
    }
}
