use serde::{Deserialize, Serialize};
use std::fmt;

/// A symbol (variable name)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol(pub String);

impl Symbol {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// A rational number (numerator/denominator)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rational {
    pub num: i64,
    pub den: i64,
}

impl Rational {
    pub fn new(num: i64, den: i64) -> Self {
        // Handle division by zero - keep as-is for limit detection
        if den == 0 {
            return Self { num, den: 0 };
        }
        let g = gcd(num.abs(), den.abs());
        if g == 0 {
            return Self { num: 0, den: 1 };
        }
        let sign = if den < 0 { -1 } else { 1 };
        Self {
            num: sign * num / g,
            den: den.abs() / g,
        }
    }

    pub fn integer(n: i64) -> Self {
        Self { num: n, den: 1 }
    }

    pub fn is_integer(&self) -> bool {
        self.den == 1
    }

    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Mathematical expression AST
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    // Atoms
    /// Integer value
    Integer(i64),
    /// Rational number
    Rational(Rational),
    /// Floating point
    Float(f64),
    /// Complex number (real, imaginary)
    Complex(f64, f64),
    /// Symbolic variable
    Symbol(Symbol),

    // Arithmetic operations
    /// Negation: -x
    Neg(Box<Expr>),
    /// Addition: a + b + c + ...
    Add(Vec<Expr>),
    /// Multiplication: a * b * c * ...
    Mul(Vec<Expr>),
    /// Power: base^exponent
    Pow(Box<Expr>, Box<Expr>),

    // Function application
    /// Function call: f(args...)
    Func(String, Vec<Expr>),

    // Calculus (symbolic)
    /// Derivative: d/dx(expr) with order
    Derivative {
        expr: Box<Expr>,
        var: Symbol,
        order: u32,
    },
    /// Integral: ∫ expr dx, optionally with bounds
    Integral {
        expr: Box<Expr>,
        var: Symbol,
        lower: Option<Box<Expr>>,
        upper: Option<Box<Expr>>,
    },
    /// Limit: lim_{x→point} expr
    Limit {
        expr: Box<Expr>,
        var: Symbol,
        point: Box<Expr>,
        direction: Option<LimitDirection>,
    },
    /// Summation: Σ_{var=lower}^{upper} expr
    Sum {
        expr: Box<Expr>,
        var: Symbol,
        lower: Box<Expr>,
        upper: Box<Expr>,
    },
    /// Product: Π_{var=lower}^{upper} expr
    Product {
        expr: Box<Expr>,
        var: Symbol,
        lower: Box<Expr>,
        upper: Box<Expr>,
    },

    // Algebra
    /// Equation: lhs = rhs
    Equation(Box<Expr>, Box<Expr>),
    /// Inequality: lhs < rhs (or >, <=, >=, !=)
    Inequality {
        lhs: Box<Expr>,
        op: InequalityOp,
        rhs: Box<Expr>,
    },

    // Linear algebra
    /// Matrix: rows of expressions
    Matrix(Vec<Vec<Expr>>),
    /// Vector: list of expressions
    Vector(Vec<Expr>),

    // Special values
    /// Undefined result
    Undefined,
    /// Positive or negative infinity
    Infinity(Sign),
}

/// Direction for limits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LimitDirection {
    Left,
    Right,
}

/// Sign for infinity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sign {
    Positive,
    Negative,
}

/// Inequality operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InequalityOp {
    Lt,
    Le,
    Gt,
    Ge,
    Ne,
}

impl Expr {
    // Constructors for common expressions

    pub fn integer(n: i64) -> Self {
        Self::Integer(n)
    }

    pub fn float(x: f64) -> Self {
        Self::Float(x)
    }

    pub fn symbol(name: impl Into<String>) -> Self {
        Self::Symbol(Symbol::new(name))
    }

    pub fn neg(expr: Expr) -> Self {
        Self::Neg(Box::new(expr))
    }

    pub fn add(terms: Vec<Expr>) -> Self {
        if terms.len() == 1 {
            terms.into_iter().next().unwrap()
        } else {
            Self::Add(terms)
        }
    }

    pub fn sub(a: Expr, b: Expr) -> Self {
        Self::add(vec![a, Self::neg(b)])
    }

    pub fn mul(factors: Vec<Expr>) -> Self {
        if factors.len() == 1 {
            factors.into_iter().next().unwrap()
        } else {
            Self::Mul(factors)
        }
    }

    pub fn div(a: Expr, b: Expr) -> Self {
        Self::mul(vec![a, Self::pow(b, Self::integer(-1))])
    }

    pub fn pow(base: Expr, exp: Expr) -> Self {
        Self::Pow(Box::new(base), Box::new(exp))
    }

    pub fn func(name: impl Into<String>, args: Vec<Expr>) -> Self {
        Self::Func(name.into(), args)
    }

    // Common mathematical constants
    pub fn pi() -> Self {
        Self::symbol("pi")
    }

    pub fn e() -> Self {
        Self::symbol("e")
    }

    pub fn i() -> Self {
        Self::Complex(0.0, 1.0)
    }

    // Predicate methods

    pub fn is_zero(&self) -> bool {
        match self {
            Self::Integer(0) => true,
            Self::Float(x) if *x == 0.0 => true,
            _ => false,
        }
    }

    pub fn is_one(&self) -> bool {
        match self {
            Self::Integer(1) => true,
            Self::Float(x) if *x == 1.0 => true,
            _ => false,
        }
    }

    pub fn is_negative_one(&self) -> bool {
        match self {
            Self::Integer(-1) => true,
            Self::Float(x) if *x == -1.0 => true,
            _ => false,
        }
    }

    pub fn is_number(&self) -> bool {
        matches!(
            self,
            Self::Integer(_) | Self::Rational(_) | Self::Float(_) | Self::Complex(_, _)
        )
    }

    pub fn is_symbol(&self) -> bool {
        matches!(self, Self::Symbol(_))
    }

    /// Check if expression contains a variable
    pub fn contains_var(&self, var: &Symbol) -> bool {
        match self {
            Self::Symbol(s) => s == var,
            Self::Neg(e) => e.contains_var(var),
            Self::Add(terms) => terms.iter().any(|t| t.contains_var(var)),
            Self::Mul(factors) => factors.iter().any(|f| f.contains_var(var)),
            Self::Pow(base, exp) => base.contains_var(var) || exp.contains_var(var),
            Self::Func(_, args) => args.iter().any(|a| a.contains_var(var)),
            Self::Derivative { expr, .. } => expr.contains_var(var),
            Self::Integral { expr, .. } => expr.contains_var(var),
            Self::Limit { expr, point, .. } => expr.contains_var(var) || point.contains_var(var),
            Self::Sum { expr, lower, upper, .. } | Self::Product { expr, lower, upper, .. } => {
                expr.contains_var(var) || lower.contains_var(var) || upper.contains_var(var)
            }
            Self::Equation(lhs, rhs) => lhs.contains_var(var) || rhs.contains_var(var),
            Self::Inequality { lhs, rhs, .. } => lhs.contains_var(var) || rhs.contains_var(var),
            Self::Matrix(rows) => rows.iter().any(|row| row.iter().any(|e| e.contains_var(var))),
            Self::Vector(elems) => elems.iter().any(|e| e.contains_var(var)),
            _ => false,
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(n) => write!(f, "{n}"),
            Self::Rational(r) => write!(f, "{r}"),
            Self::Float(x) => {
                if x.fract() == 0.0 && x.abs() < 1e15 {
                    write!(f, "{}", *x as i64)
                } else {
                    write!(f, "{x}")
                }
            }
            Self::Complex(re, im) => {
                if *re == 0.0 {
                    if *im == 1.0 {
                        write!(f, "i")
                    } else if *im == -1.0 {
                        write!(f, "-i")
                    } else {
                        write!(f, "{im}i")
                    }
                } else if *im >= 0.0 {
                    write!(f, "{re}+{im}i")
                } else {
                    write!(f, "{re}{im}i")
                }
            }
            Self::Symbol(s) => write!(f, "{s}"),
            Self::Neg(e) => write!(f, "-{e}"),
            Self::Add(terms) => {
                if terms.is_empty() {
                    write!(f, "0")
                } else {
                    write!(f, "(")?;
                    for (i, term) in terms.iter().enumerate() {
                        if i > 0 {
                            write!(f, "+")?;
                        }
                        write!(f, "{term}")?;
                    }
                    write!(f, ")")
                }
            }
            Self::Mul(factors) => {
                if factors.is_empty() {
                    write!(f, "1")
                } else {
                    write!(f, "(")?;
                    for (i, factor) in factors.iter().enumerate() {
                        if i > 0 {
                            write!(f, "*")?;
                        }
                        write!(f, "{factor}")?;
                    }
                    write!(f, ")")
                }
            }
            Self::Pow(base, exp) => write!(f, "{base}^{exp}"),
            Self::Func(name, args) => {
                write!(f, "{name}(")?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                write!(f, ")")
            }
            Self::Derivative { expr, var, order } => {
                if *order == 1 {
                    write!(f, "d/d{var}({expr})")
                } else {
                    write!(f, "d^{order}/d{var}^{order}({expr})")
                }
            }
            Self::Integral {
                expr,
                var,
                lower,
                upper,
            } => {
                if let (Some(l), Some(u)) = (lower, upper) {
                    write!(f, "integrate({expr}, {var}, {l}, {u})")
                } else {
                    write!(f, "integrate({expr}, {var})")
                }
            }
            Self::Limit {
                expr,
                var,
                point,
                direction,
            } => {
                let dir = match direction {
                    Some(LimitDirection::Left) => "-",
                    Some(LimitDirection::Right) => "+",
                    None => "",
                };
                write!(f, "lim({expr}, {var}, {point}{dir})")
            }
            Self::Sum {
                expr,
                var,
                lower,
                upper,
            } => write!(f, "sum({expr}, {var}, {lower}, {upper})"),
            Self::Product {
                expr,
                var,
                lower,
                upper,
            } => write!(f, "product({expr}, {var}, {lower}, {upper})"),
            Self::Equation(lhs, rhs) => write!(f, "{lhs} = {rhs}"),
            Self::Inequality { lhs, op, rhs } => {
                let op_str = match op {
                    InequalityOp::Lt => "<",
                    InequalityOp::Le => "<=",
                    InequalityOp::Gt => ">",
                    InequalityOp::Ge => ">=",
                    InequalityOp::Ne => "!=",
                };
                write!(f, "{lhs} {op_str} {rhs}")
            }
            Self::Matrix(rows) => {
                write!(f, "[")?;
                for (i, row) in rows.iter().enumerate() {
                    if i > 0 {
                        write!(f, "; ")?;
                    }
                    for (j, elem) in row.iter().enumerate() {
                        if j > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{elem}")?;
                    }
                }
                write!(f, "]")
            }
            Self::Vector(elems) => {
                write!(f, "[")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{elem}")?;
                }
                write!(f, "]")
            }
            Self::Undefined => write!(f, "undefined"),
            Self::Infinity(Sign::Positive) => write!(f, "infinity"),
            Self::Infinity(Sign::Negative) => write!(f, "-infinity"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rational() {
        let r = Rational::new(4, 6);
        assert_eq!(r.num, 2);
        assert_eq!(r.den, 3);
        assert_eq!(r.to_string(), "2/3");
    }

    #[test]
    fn test_expr_display() {
        let expr = Expr::add(vec![
            Expr::symbol("x"),
            Expr::mul(vec![Expr::integer(2), Expr::symbol("y")]),
        ]);
        assert_eq!(expr.to_string(), "(x+(2*y))");
    }

    #[test]
    fn test_contains_var() {
        let expr = Expr::add(vec![Expr::symbol("x"), Expr::integer(1)]);
        assert!(expr.contains_var(&Symbol::new("x")));
        assert!(!expr.contains_var(&Symbol::new("y")));
    }
}
