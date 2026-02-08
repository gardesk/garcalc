//! Structured math input model
//!
//! MathBox represents mathematical expressions as a tree structure
//! with navigable slots for user input.

use serde::{Deserialize, Serialize};

/// Structured math input with navigation slots
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MathBox {
    /// Numeric literal (digits and decimal point)
    Number(String),

    /// Variable or constant symbol
    Symbol(String),

    /// Operator symbol (+, -, ×, ÷, =)
    Operator(Operator),

    /// Fraction with numerator and denominator
    Fraction {
        num: Box<MathBox>,
        den: Box<MathBox>,
    },

    /// Power/exponent
    Power {
        base: Box<MathBox>,
        exp: Box<MathBox>,
    },

    /// Subscript (for indexed variables like x_1)
    Subscript {
        base: Box<MathBox>,
        sub: Box<MathBox>,
    },

    /// Square root or nth root
    Root {
        /// None for square root, Some for nth root
        index: Option<Box<MathBox>>,
        radicand: Box<MathBox>,
    },

    /// Function call with arguments
    Func {
        name: String,
        args: Vec<MathBox>,
    },

    /// Absolute value
    Abs(Box<MathBox>),

    /// Parenthesized expression
    Parens(Box<MathBox>),

    /// Integral
    Integral {
        /// Lower bound (for definite integral)
        lower: Option<Box<MathBox>>,
        /// Upper bound (for definite integral)
        upper: Option<Box<MathBox>>,
        /// The integrand
        body: Box<MathBox>,
        /// The variable of integration
        var: Box<MathBox>,
    },

    /// Derivative
    Derivative {
        /// Order of derivative (1 = first, 2 = second, etc.)
        order: u32,
        /// Variable to differentiate with respect to
        var: Box<MathBox>,
        /// Expression to differentiate
        body: Box<MathBox>,
    },

    /// Limit
    Limit {
        /// Variable approaching
        var: Box<MathBox>,
        /// Value being approached
        to: Box<MathBox>,
        /// Direction (optional: +, -, or none for two-sided)
        direction: Option<LimitDirection>,
        /// Expression to take limit of
        body: Box<MathBox>,
    },

    /// Summation
    Sum {
        /// Index variable
        var: Box<MathBox>,
        /// Lower bound (e.g., i=0)
        lower: Box<MathBox>,
        /// Upper bound (e.g., n)
        upper: Box<MathBox>,
        /// Expression to sum
        body: Box<MathBox>,
    },

    /// Product (capital Pi)
    Product {
        /// Index variable
        var: Box<MathBox>,
        /// Lower bound
        lower: Box<MathBox>,
        /// Upper bound
        upper: Box<MathBox>,
        /// Expression to multiply
        body: Box<MathBox>,
    },

    /// Matrix with rows of cells
    Matrix {
        rows: Vec<Vec<MathBox>>,
    },

    /// Horizontal sequence of elements (e.g., 2 + 3 × x)
    Row(Vec<MathBox>),

    /// Empty slot waiting for input (cursor can enter)
    Slot,
}

/// Mathematical operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operator {
    Add,      // +
    Sub,      // -
    Mul,      // × or *
    Div,      // ÷ (for inline division, not fraction)
    Eq,       // =
    Lt,       // <
    Gt,       // >
    Le,       // ≤
    Ge,       // ≥
    Ne,       // ≠
    Comma,    // ,
}

impl Operator {
    /// Get the display character for this operator
    pub fn as_char(&self) -> char {
        match self {
            Operator::Add => '+',
            Operator::Sub => '−',
            Operator::Mul => '×',
            Operator::Div => '÷',
            Operator::Eq => '=',
            Operator::Lt => '<',
            Operator::Gt => '>',
            Operator::Le => '≤',
            Operator::Ge => '≥',
            Operator::Ne => '≠',
            Operator::Comma => ',',
        }
    }
}

/// Direction for one-sided limits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LimitDirection {
    FromLeft,  // x → a⁻
    FromRight, // x → a⁺
}

/// Cursor position in the MathBox tree
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cursor {
    /// Path through the tree (indices at each level)
    pub path: Vec<usize>,
    /// Position within current element (for Number, Symbol, Row)
    pub offset: usize,
}

impl Cursor {
    /// Create a new cursor at the root
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the depth of the cursor in the tree
    pub fn depth(&self) -> usize {
        self.path.len()
    }

    /// Check if cursor is at the root level
    pub fn is_at_root(&self) -> bool {
        self.path.is_empty()
    }

    /// Move cursor into a child slot
    pub fn enter(&mut self, index: usize) {
        self.path.push(index);
        self.offset = 0;
    }

    /// Move cursor out of current slot to parent
    pub fn exit(&mut self) -> Option<usize> {
        self.offset = 0;
        self.path.pop()
    }
}

impl MathBox {
    /// Create an empty slot
    pub fn slot() -> Self {
        MathBox::Slot
    }

    /// Create a fraction template with empty slots
    pub fn fraction_template() -> Self {
        MathBox::Fraction {
            num: Box::new(MathBox::Slot),
            den: Box::new(MathBox::Slot),
        }
    }

    /// Create a power template with the given base
    pub fn power_template(base: MathBox) -> Self {
        MathBox::Power {
            base: Box::new(base),
            exp: Box::new(MathBox::Slot),
        }
    }

    /// Create a square root template
    pub fn sqrt_template() -> Self {
        MathBox::Root {
            index: None,
            radicand: Box::new(MathBox::Slot),
        }
    }

    /// Create an nth root template
    pub fn nthroot_template() -> Self {
        MathBox::Root {
            index: Some(Box::new(MathBox::Slot)),
            radicand: Box::new(MathBox::Slot),
        }
    }

    /// Create an indefinite integral template
    pub fn integral_template() -> Self {
        MathBox::Integral {
            lower: None,
            upper: None,
            body: Box::new(MathBox::Slot),
            var: Box::new(MathBox::Symbol("x".to_string())),
        }
    }

    /// Create a definite integral template
    pub fn definite_integral_template() -> Self {
        MathBox::Integral {
            lower: Some(Box::new(MathBox::Slot)),
            upper: Some(Box::new(MathBox::Slot)),
            body: Box::new(MathBox::Slot),
            var: Box::new(MathBox::Symbol("x".to_string())),
        }
    }

    /// Create a derivative template
    pub fn derivative_template() -> Self {
        MathBox::Derivative {
            order: 1,
            var: Box::new(MathBox::Slot),
            body: Box::new(MathBox::Slot),
        }
    }

    /// Create a limit template
    pub fn limit_template() -> Self {
        MathBox::Limit {
            var: Box::new(MathBox::Symbol("x".to_string())),
            to: Box::new(MathBox::Slot),
            direction: None,
            body: Box::new(MathBox::Slot),
        }
    }

    /// Create a summation template
    pub fn sum_template() -> Self {
        MathBox::Sum {
            var: Box::new(MathBox::Symbol("i".to_string())),
            lower: Box::new(MathBox::Slot),
            upper: Box::new(MathBox::Slot),
            body: Box::new(MathBox::Slot),
        }
    }

    /// Create a product template
    pub fn product_template() -> Self {
        MathBox::Product {
            var: Box::new(MathBox::Symbol("i".to_string())),
            lower: Box::new(MathBox::Slot),
            upper: Box::new(MathBox::Slot),
            body: Box::new(MathBox::Slot),
        }
    }

    /// Create a matrix template with given dimensions
    pub fn matrix_template(rows: usize, cols: usize) -> Self {
        MathBox::Matrix {
            rows: (0..rows)
                .map(|_| (0..cols).map(|_| MathBox::Slot).collect())
                .collect(),
        }
    }

    /// Check if this is an empty slot
    pub fn is_slot(&self) -> bool {
        matches!(self, MathBox::Slot)
    }

    /// Check if this element is empty (slot or empty row/number)
    pub fn is_empty(&self) -> bool {
        match self {
            MathBox::Slot => true,
            MathBox::Number(s) | MathBox::Symbol(s) => s.is_empty(),
            MathBox::Row(items) => items.is_empty(),
            _ => false,
        }
    }

    /// Get the number of child slots this element has
    pub fn child_count(&self) -> usize {
        match self {
            MathBox::Number(_) | MathBox::Symbol(_) | MathBox::Operator(_) | MathBox::Slot => 0,
            MathBox::Fraction { .. } | MathBox::Power { .. } | MathBox::Subscript { .. } => 2,
            MathBox::Root { index: Some(_), .. } => 2,
            MathBox::Root { index: None, .. } => 1,
            MathBox::Func { args, .. } => args.len(),
            MathBox::Abs(_) | MathBox::Parens(_) => 1,
            MathBox::Integral { lower, upper, .. } => {
                2 + if lower.is_some() { 1 } else { 0 } + if upper.is_some() { 1 } else { 0 }
            }
            MathBox::Derivative { .. } => 2,
            MathBox::Limit { .. } => 3,
            MathBox::Sum { .. } | MathBox::Product { .. } => 4,
            MathBox::Matrix { rows } => rows.iter().map(|r| r.len()).sum(),
            MathBox::Row(items) => items.len(),
        }
    }

    /// Get a mutable reference to a child by index
    pub fn child_mut(&mut self, index: usize) -> Option<&mut MathBox> {
        match self {
            MathBox::Fraction { num, den } => match index {
                0 => Some(num),
                1 => Some(den),
                _ => None,
            },
            MathBox::Power { base, exp } => match index {
                0 => Some(base),
                1 => Some(exp),
                _ => None,
            },
            MathBox::Subscript { base, sub } => match index {
                0 => Some(base),
                1 => Some(sub),
                _ => None,
            },
            MathBox::Root { index: idx, radicand } => {
                if let Some(i) = idx {
                    match index {
                        0 => Some(i),
                        1 => Some(radicand),
                        _ => None,
                    }
                } else {
                    match index {
                        0 => Some(radicand),
                        _ => None,
                    }
                }
            },
            MathBox::Func { args, .. } => args.get_mut(index),
            MathBox::Abs(inner) | MathBox::Parens(inner) => match index {
                0 => Some(inner),
                _ => None,
            },
            MathBox::Integral { lower, upper, body, var } => {
                let mut i = 0;
                if let Some(l) = lower {
                    if index == i { return Some(l); }
                    i += 1;
                }
                if let Some(u) = upper {
                    if index == i { return Some(u); }
                    i += 1;
                }
                if index == i { return Some(body); }
                if index == i + 1 { return Some(var); }
                None
            },
            MathBox::Derivative { var, body, .. } => match index {
                0 => Some(var),
                1 => Some(body),
                _ => None,
            },
            MathBox::Limit { var, to, body, .. } => match index {
                0 => Some(var),
                1 => Some(to),
                2 => Some(body),
                _ => None,
            },
            MathBox::Sum { var, lower, upper, body }
            | MathBox::Product { var, lower, upper, body } => match index {
                0 => Some(var),
                1 => Some(lower),
                2 => Some(upper),
                3 => Some(body),
                _ => None,
            },
            MathBox::Matrix { rows } => {
                let mut idx = 0;
                for row in rows.iter_mut() {
                    for cell in row.iter_mut() {
                        if idx == index {
                            return Some(cell);
                        }
                        idx += 1;
                    }
                }
                None
            },
            MathBox::Row(items) => items.get_mut(index),
            _ => None,
        }
    }

    /// Get an immutable reference to a child by index
    pub fn child(&self, index: usize) -> Option<&MathBox> {
        match self {
            MathBox::Fraction { num, den } => match index {
                0 => Some(num),
                1 => Some(den),
                _ => None,
            },
            MathBox::Power { base, exp } => match index {
                0 => Some(base),
                1 => Some(exp),
                _ => None,
            },
            MathBox::Subscript { base, sub } => match index {
                0 => Some(base),
                1 => Some(sub),
                _ => None,
            },
            MathBox::Root { index: idx, radicand } => {
                if let Some(i) = idx {
                    match index {
                        0 => Some(i),
                        1 => Some(radicand),
                        _ => None,
                    }
                } else {
                    match index {
                        0 => Some(radicand),
                        _ => None,
                    }
                }
            },
            MathBox::Func { args, .. } => args.get(index),
            MathBox::Abs(inner) | MathBox::Parens(inner) => match index {
                0 => Some(inner),
                _ => None,
            },
            MathBox::Integral { lower, upper, body, var } => {
                let mut i = 0;
                if let Some(l) = lower {
                    if index == i { return Some(l); }
                    i += 1;
                }
                if let Some(u) = upper {
                    if index == i { return Some(u); }
                    i += 1;
                }
                if index == i { return Some(body); }
                if index == i + 1 { return Some(var); }
                None
            },
            MathBox::Derivative { var, body, .. } => match index {
                0 => Some(var),
                1 => Some(body),
                _ => None,
            },
            MathBox::Limit { var, to, body, .. } => match index {
                0 => Some(var),
                1 => Some(to),
                2 => Some(body),
                _ => None,
            },
            MathBox::Sum { var, lower, upper, body }
            | MathBox::Product { var, lower, upper, body } => match index {
                0 => Some(var),
                1 => Some(lower),
                2 => Some(upper),
                3 => Some(body),
                _ => None,
            },
            MathBox::Matrix { rows } => {
                let mut idx = 0;
                for row in rows.iter() {
                    for cell in row.iter() {
                        if idx == index {
                            return Some(cell);
                        }
                        idx += 1;
                    }
                }
                None
            },
            MathBox::Row(items) => items.get(index),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fraction_template() {
        let frac = MathBox::fraction_template();
        if let MathBox::Fraction { num, den } = frac {
            assert!(num.is_slot());
            assert!(den.is_slot());
        } else {
            panic!("Expected Fraction");
        }
    }

    #[test]
    fn test_cursor_navigation() {
        let mut cursor = Cursor::new();
        assert!(cursor.is_at_root());
        assert_eq!(cursor.depth(), 0);

        cursor.enter(0);
        assert!(!cursor.is_at_root());
        assert_eq!(cursor.depth(), 1);

        cursor.enter(1);
        assert_eq!(cursor.depth(), 2);

        assert_eq!(cursor.exit(), Some(1));
        assert_eq!(cursor.depth(), 1);
    }

    #[test]
    fn test_child_access() {
        let mut frac = MathBox::Fraction {
            num: Box::new(MathBox::Number("1".to_string())),
            den: Box::new(MathBox::Number("2".to_string())),
        };

        assert_eq!(frac.child_count(), 2);

        if let Some(MathBox::Number(s)) = frac.child(0) {
            assert_eq!(s, "1");
        }

        if let Some(num) = frac.child_mut(0) {
            *num = MathBox::Number("42".to_string());
        }

        if let Some(MathBox::Number(s)) = frac.child(0) {
            assert_eq!(s, "42");
        }
    }
}
