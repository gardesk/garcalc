//! Math layout engine
//!
//! Computes bounding boxes and positions for mathematical typesetting.
//! Uses baseline-aligned layout with proper ascent/descent metrics.

use crate::mathbox::{MathBox, Operator, LimitDirection};
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

    /// Compute layout at a specific nesting depth
    fn layout_at_depth(&self, mathbox: &MathBox, ctx: &Context, depth: u32) -> LayoutBox {
        let scale = self.scale_for_depth(depth);
        let font_size = self.base_font_size * scale;

        match mathbox {
            MathBox::Number(s) => self.layout_text(s, ctx, font_size),
            MathBox::Symbol(s) => self.layout_symbol(s, ctx, font_size),
            MathBox::Operator(op) => self.layout_operator(*op, ctx, font_size),
            MathBox::Slot => self.layout_slot(ctx, font_size),
            MathBox::Fraction { num, den } => {
                self.layout_fraction(num, den, ctx, depth)
            }
            MathBox::Power { base, exp } => {
                self.layout_power(base, exp, ctx, depth)
            }
            MathBox::Subscript { base, sub } => {
                self.layout_subscript(base, sub, ctx, depth)
            }
            MathBox::Root { index, radicand } => {
                self.layout_root(index.as_deref(), radicand, ctx, depth)
            }
            MathBox::Func { name, args } => {
                self.layout_func(name, args, ctx, depth)
            }
            MathBox::Abs(inner) => self.layout_abs(inner, ctx, depth),
            MathBox::Parens(inner) => self.layout_parens(inner, ctx, depth),
            MathBox::Integral { lower, upper, body, var } => {
                self.layout_integral(
                    lower.as_deref(),
                    upper.as_deref(),
                    body,
                    var,
                    ctx,
                    depth,
                )
            }
            MathBox::Derivative { order, var, body } => {
                self.layout_derivative(*order, var, body, ctx, depth)
            }
            MathBox::Limit { var, to, direction, body } => {
                self.layout_limit(var, to, *direction, body, ctx, depth)
            }
            MathBox::Sum { var, lower, upper, body } => {
                self.layout_bigop("∑", var, lower, upper, body, ctx, depth)
            }
            MathBox::Product { var, lower, upper, body } => {
                self.layout_bigop("∏", var, lower, upper, body, ctx, depth)
            }
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
        ctx.select_font_face(&self.font_family, cairo::FontSlant::Italic, cairo::FontWeight::Normal);
        let layout = self.layout_text(symbol, ctx, font_size);
        ctx.select_font_face(&self.font_family, cairo::FontSlant::Normal, cairo::FontWeight::Normal);
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
        let slot_width = font_size * 0.8;
        let slot_height = font_size * 0.8;

        LayoutBox {
            width: slot_width,
            ascent: slot_height * 0.6,
            descent: slot_height * 0.4,
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
            children: vec![
                (num_x, num_y, num_layout),
                (den_x, den_y, den_layout),
            ],
        }
    }

    /// Layout a power (superscript)
    fn layout_power(
        &self,
        base: &MathBox,
        exp: &MathBox,
        ctx: &Context,
        depth: u32,
    ) -> LayoutBox {
        let base_layout = self.layout_at_depth(base, ctx, depth);
        let exp_layout = self.layout_at_depth(exp, ctx, depth + 1);

        // Exponent is raised above the baseline
        let exp_raise = base_layout.ascent * 0.5;

        LayoutBox {
            width: base_layout.width + exp_layout.width,
            ascent: (base_layout.ascent).max(exp_raise + exp_layout.height()),
            descent: base_layout.descent,
            children: vec![
                (0.0, 0.0, base_layout.clone()),
                (base_layout.width, -exp_raise, exp_layout),
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

        // Subscript is lowered below the baseline
        let sub_lower = base_layout.descent + sub_layout.ascent * 0.3;

        LayoutBox {
            width: base_layout.width + sub_layout.width,
            ascent: base_layout.ascent,
            descent: (base_layout.descent).max(sub_lower + sub_layout.height()),
            children: vec![
                (0.0, 0.0, base_layout.clone()),
                (base_layout.width, sub_lower, sub_layout),
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
    fn layout_func(
        &self,
        name: &str,
        args: &[MathBox],
        ctx: &Context,
        depth: u32,
    ) -> LayoutBox {
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

        // Integral symbol sizing
        let int_height = (body_layout.height()).max(font_size * 1.5);
        let int_width = font_size * 0.5;

        let mut width = int_width;
        let mut ascent = int_height / 2.0;
        let mut descent = int_height / 2.0;
        let mut children = vec![];

        // Lower bound
        if let Some(lo) = lower {
            let lo_layout = self.layout_at_depth(lo, ctx, depth + 1);
            children.push((0.0, int_height / 2.0 + lo_layout.ascent, lo_layout.clone()));
            descent = descent.max(int_height / 2.0 + lo_layout.height());
            width = width.max(lo_layout.width);
        }

        // Upper bound
        if let Some(hi) = upper {
            let hi_layout = self.layout_at_depth(hi, ctx, depth + 1);
            children.push((0.0, -(int_height / 2.0 + hi_layout.descent), hi_layout.clone()));
            ascent = ascent.max(int_height / 2.0 + hi_layout.height());
            width = width.max(hi_layout.width);
        }

        // Body
        let body_x = width + font_size * 0.2;
        children.push((body_x, 0.0, body_layout.clone()));
        width = body_x + body_layout.width;

        // "dx" part
        let d_layout = self.layout_text("d", ctx, font_size);
        children.push((width + font_size * 0.1, 0.0, d_layout.clone()));
        width += font_size * 0.1 + d_layout.width;
        children.push((width, 0.0, var_layout.clone()));
        width += var_layout.width;

        LayoutBox {
            width,
            ascent,
            descent,
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

        let d_layout = self.layout_text(&d_str, ctx, font_size);
        let dx_layout = self.layout_text(&dx_str, ctx, font_size);

        let frac_width = d_layout.width.max(dx_layout.width + var_layout.width) + font_size * 0.2;
        let bar_gap = font_size * 0.1;

        let width = frac_width + font_size * 0.2 + body_layout.width;
        let frac_height = d_layout.height() + dx_layout.height() + var_layout.height() + bar_gap * 2.0;

        LayoutBox {
            width,
            ascent: frac_height / 2.0 + bar_gap,
            descent: frac_height / 2.0 + bar_gap,
            children: vec![
                (frac_width + font_size * 0.2, 0.0, body_layout),
            ],
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

        // Build subscript: "x→a" or "x→a⁺" or "x→a⁻"
        let arrow_layout = self.layout_text("→", ctx, font_size * 0.7);
        let dir_str = match direction {
            Some(LimitDirection::FromRight) => "⁺",
            Some(LimitDirection::FromLeft) => "⁻",
            None => "",
        };
        let dir_layout = if !dir_str.is_empty() {
            Some(self.layout_text(dir_str, ctx, font_size * 0.5))
        } else {
            None
        };

        let subscript_width = var_layout.width + arrow_layout.width + to_layout.width
            + dir_layout.as_ref().map(|l| l.width).unwrap_or(0.0);
        let subscript_height = var_layout.height().max(arrow_layout.height()).max(to_layout.height());

        let lim_width = lim_layout.width.max(subscript_width);

        LayoutBox {
            width: lim_width + font_size * 0.3 + body_layout.width,
            ascent: lim_layout.ascent.max(body_layout.ascent),
            descent: (lim_layout.descent + subscript_height + font_size * 0.1).max(body_layout.descent),
            children: vec![
                (lim_width + font_size * 0.3, 0.0, body_layout),
            ],
        }
    }

    /// Layout a big operator (sum, product)
    fn layout_bigop(
        &self,
        symbol: &str,
        _var: &MathBox,
        lower: &MathBox,
        upper: &MathBox,
        body: &MathBox,
        ctx: &Context,
        depth: u32,
    ) -> LayoutBox {
        let scale = self.scale_for_depth(depth);
        let font_size = self.base_font_size * scale;

        let body_layout = self.layout_at_depth(body, ctx, depth);
        let lower_layout = self.layout_at_depth(lower, ctx, depth + 1);
        let upper_layout = self.layout_at_depth(upper, ctx, depth + 1);

        // Big operator symbol
        let symbol_size = font_size * 1.5;
        ctx.set_font_size(symbol_size);
        let symbol_extents = ctx.text_extents(symbol).unwrap();
        let symbol_width = symbol_extents.x_advance();
        let symbol_height = symbol_size;

        let op_width = symbol_width.max(lower_layout.width).max(upper_layout.width);
        let gap = font_size * 0.1;

        let ascent = symbol_height / 2.0 + gap + upper_layout.height();
        let descent = symbol_height / 2.0 + gap + lower_layout.height();

        LayoutBox {
            width: op_width + font_size * 0.3 + body_layout.width,
            ascent: ascent.max(body_layout.ascent),
            descent: descent.max(body_layout.descent),
            children: vec![
                (op_width + font_size * 0.3, 0.0, body_layout),
            ],
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
        let total_height: f64 = row_heights.iter().sum::<f64>()
            + cell_padding * (rows.len() as f64 - 1.0);

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
}
