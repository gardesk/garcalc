//! Expression evaluation
//!
//! Supports both numeric and symbolic evaluation modes.

use std::collections::HashMap;
use std::f64::consts::{E, PI};

use crate::error::{CasError, Result};
use crate::expr::{Expr, Rational, Sign};
use crate::symbolic::{Differentiator, Integrator, Limits, Simplifier, Solver};

/// Variable bindings for evaluation
pub type Environment = HashMap<String, Expr>;

/// Expression evaluator
pub struct Evaluator {
    env: Environment,
    /// If true, try to keep results symbolic when possible
    exact_mode: bool,
    /// Angle mode for trig functions
    angle_mode: AngleMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleMode {
    Radians,
    Degrees,
}

impl Default for AngleMode {
    fn default() -> Self {
        Self::Radians
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluator {
    pub fn new() -> Self {
        Self {
            env: Environment::new(),
            exact_mode: false,
            angle_mode: AngleMode::Radians,
        }
    }

    pub fn with_exact_mode(mut self, exact: bool) -> Self {
        self.exact_mode = exact;
        self
    }

    pub fn with_angle_mode(mut self, mode: AngleMode) -> Self {
        self.angle_mode = mode;
        self
    }

    pub fn set_var(&mut self, name: impl Into<String>, value: Expr) {
        self.env.insert(name.into(), value);
    }

    pub fn get_var(&self, name: &str) -> Option<&Expr> {
        self.env.get(name)
    }

    pub fn clear_vars(&mut self) {
        self.env.clear();
    }

    /// Evaluate an expression to a numeric result
    pub fn eval(&self, expr: &Expr) -> Result<Expr> {
        match expr {
            Expr::Integer(n) => Ok(Expr::Integer(*n)),
            Expr::Rational(r) => Ok(Expr::Rational(*r)),
            Expr::Float(x) => Ok(Expr::Float(*x)),
            Expr::Complex(re, im) => Ok(Expr::Complex(*re, *im)),

            Expr::Symbol(sym) => {
                // Check for constants
                match sym.as_str() {
                    "pi" => Ok(Expr::Float(PI)),
                    "e" => Ok(Expr::Float(E)),
                    _ => {
                        // Check environment
                        if let Some(value) = self.env.get(sym.as_str()) {
                            self.eval(value)
                        } else if self.exact_mode {
                            // In exact mode, keep undefined symbols
                            Ok(expr.clone())
                        } else {
                            Err(CasError::UndefinedVariable(sym.to_string()))
                        }
                    }
                }
            }

            Expr::Neg(e) => {
                let val = self.eval(e)?;
                self.negate(&val)
            }

            Expr::Add(terms) => {
                let mut sum = Expr::integer(0);
                for term in terms {
                    let val = self.eval(term)?;
                    sum = self.add(&sum, &val)?;
                }
                Ok(sum)
            }

            Expr::Mul(factors) => {
                let mut product = Expr::integer(1);
                for factor in factors {
                    let val = self.eval(factor)?;
                    product = self.multiply(&product, &val)?;
                }
                Ok(product)
            }

            Expr::Pow(base, exp) => {
                let base_val = self.eval(base)?;
                let exp_val = self.eval(exp)?;
                self.power(&base_val, &exp_val)
            }

            Expr::Func(name, args) => {
                // Symbolic functions operate on unevaluated expressions
                match name.as_str() {
                    "diff" | "derivative" | "integrate" | "integral" |
                    "limit" | "lim" |
                    "solve" | "simplify" | "expand" | "factor" |
                    "substitute" | "subs" => {
                        self.call_function(name, args)
                    }
                    _ => {
                        let evaluated_args: Result<Vec<_>> =
                            args.iter().map(|a| self.eval(a)).collect();
                        self.call_function(name, &evaluated_args?)
                    }
                }
            }

            Expr::Equation(lhs, rhs) => {
                let lhs_val = self.eval(lhs)?;
                let rhs_val = self.eval(rhs)?;
                Ok(Expr::Equation(Box::new(lhs_val), Box::new(rhs_val)))
            }

            Expr::Vector(elems) => {
                let evaluated: Result<Vec<_>> = elems.iter().map(|e| self.eval(e)).collect();
                Ok(Expr::Vector(evaluated?))
            }

            Expr::Matrix(rows) => {
                let evaluated: Result<Vec<Vec<_>>> = rows
                    .iter()
                    .map(|row| row.iter().map(|e| self.eval(e)).collect())
                    .collect();
                Ok(Expr::Matrix(evaluated?))
            }

            // Symbolic operations - perform symbolic computation then try to evaluate
            Expr::Derivative { expr: inner, var, order } => {
                let result = Differentiator::diff_n(inner, var, *order)?;
                let simplified = Simplifier::simplify(&result);
                // Try to evaluate the result
                if simplified.contains_var(var) {
                    // Still has variable - return symbolic result
                    Ok(simplified)
                } else {
                    self.eval(&simplified)
                }
            }

            Expr::Integral { expr: inner, var, lower, upper } => {
                if let (Some(l), Some(u)) = (lower, upper) {
                    // Definite integral - evaluate to number
                    let result = Integrator::integrate_definite(inner, var, l, u)?;
                    let simplified = Simplifier::simplify(&result);
                    self.eval(&simplified)
                } else {
                    // Indefinite integral - return symbolic result
                    let result = Integrator::integrate(inner, var)?;
                    Ok(Simplifier::simplify(&result))
                }
            }

            Expr::Limit { expr: inner, var, point, direction } => {
                let result = Limits::limit(inner, var, point, *direction)?;
                let simplified = Simplifier::simplify(&result);
                self.eval(&simplified)
            }

            Expr::Sum { .. }
            | Expr::Product { .. } => {
                if self.exact_mode {
                    Ok(expr.clone())
                } else {
                    Err(CasError::NotImplemented(
                        "sums/products not yet implemented".to_string(),
                    ))
                }
            }

            Expr::Inequality { lhs, op, rhs } => {
                let lhs_val = self.eval(lhs)?;
                let rhs_val = self.eval(rhs)?;
                Ok(Expr::Inequality {
                    lhs: Box::new(lhs_val),
                    op: *op,
                    rhs: Box::new(rhs_val),
                })
            }

            Expr::Undefined => Ok(Expr::Undefined),
            Expr::Infinity(sign) => Ok(Expr::Infinity(*sign)),
        }
    }

    fn to_f64(&self, expr: &Expr) -> Result<f64> {
        match expr {
            Expr::Integer(n) => Ok(*n as f64),
            Expr::Rational(r) => Ok(r.to_f64()),
            Expr::Float(x) => Ok(*x),
            _ => Err(CasError::Type(format!("expected number, got {expr}"))),
        }
    }

    fn negate(&self, expr: &Expr) -> Result<Expr> {
        match expr {
            Expr::Integer(n) => Ok(Expr::Integer(-n)),
            Expr::Rational(r) => Ok(Expr::Rational(Rational::new(-r.num, r.den))),
            Expr::Float(x) => Ok(Expr::Float(-x)),
            Expr::Complex(re, im) => Ok(Expr::Complex(-re, -im)),
            Expr::Infinity(Sign::Positive) => Ok(Expr::Infinity(Sign::Negative)),
            Expr::Infinity(Sign::Negative) => Ok(Expr::Infinity(Sign::Positive)),
            _ => Ok(Expr::neg(expr.clone())),
        }
    }

    fn add(&self, a: &Expr, b: &Expr) -> Result<Expr> {
        match (a, b) {
            (Expr::Integer(x), Expr::Integer(y)) => Ok(Expr::Integer(x + y)),
            (Expr::Float(x), Expr::Float(y)) => Ok(Expr::Float(x + y)),
            (Expr::Integer(x), Expr::Float(y)) | (Expr::Float(y), Expr::Integer(x)) => {
                Ok(Expr::Float(*x as f64 + y))
            }
            (Expr::Complex(r1, i1), Expr::Complex(r2, i2)) => Ok(Expr::Complex(r1 + r2, i1 + i2)),
            (Expr::Complex(re, im), Expr::Float(x)) | (Expr::Float(x), Expr::Complex(re, im)) => {
                Ok(Expr::Complex(re + x, *im))
            }
            (Expr::Complex(re, im), Expr::Integer(n))
            | (Expr::Integer(n), Expr::Complex(re, im)) => {
                Ok(Expr::Complex(re + *n as f64, *im))
            }
            (Expr::Rational(r1), Expr::Rational(r2)) => {
                let num = r1.num * r2.den + r2.num * r1.den;
                let den = r1.den * r2.den;
                Ok(Expr::Rational(Rational::new(num, den)))
            }
            (Expr::Rational(r), Expr::Integer(n)) | (Expr::Integer(n), Expr::Rational(r)) => {
                let num = r.num + n * r.den;
                Ok(Expr::Rational(Rational::new(num, r.den)))
            }
            _ => {
                // Try converting to floats
                if let (Ok(x), Ok(y)) = (self.to_f64(a), self.to_f64(b)) {
                    Ok(Expr::Float(x + y))
                } else if self.exact_mode {
                    Ok(Expr::add(vec![a.clone(), b.clone()]))
                } else {
                    Err(CasError::Type(format!("cannot add {a} and {b}")))
                }
            }
        }
    }

    fn multiply(&self, a: &Expr, b: &Expr) -> Result<Expr> {
        match (a, b) {
            (Expr::Integer(x), Expr::Integer(y)) => Ok(Expr::Integer(x * y)),
            (Expr::Float(x), Expr::Float(y)) => Ok(Expr::Float(x * y)),
            (Expr::Integer(x), Expr::Float(y)) | (Expr::Float(y), Expr::Integer(x)) => {
                Ok(Expr::Float(*x as f64 * y))
            }
            (Expr::Complex(r1, i1), Expr::Complex(r2, i2)) => {
                // (a+bi)(c+di) = (ac-bd) + (ad+bc)i
                Ok(Expr::Complex(r1 * r2 - i1 * i2, r1 * i2 + i1 * r2))
            }
            (Expr::Complex(re, im), Expr::Float(x)) | (Expr::Float(x), Expr::Complex(re, im)) => {
                Ok(Expr::Complex(re * x, im * x))
            }
            (Expr::Complex(re, im), Expr::Integer(n))
            | (Expr::Integer(n), Expr::Complex(re, im)) => {
                let n = *n as f64;
                Ok(Expr::Complex(re * n, im * n))
            }
            (Expr::Rational(r1), Expr::Rational(r2)) => {
                Ok(Expr::Rational(Rational::new(r1.num * r2.num, r1.den * r2.den)))
            }
            (Expr::Rational(r), Expr::Integer(n)) | (Expr::Integer(n), Expr::Rational(r)) => {
                Ok(Expr::Rational(Rational::new(r.num * n, r.den)))
            }
            _ => {
                if let (Ok(x), Ok(y)) = (self.to_f64(a), self.to_f64(b)) {
                    Ok(Expr::Float(x * y))
                } else if self.exact_mode {
                    Ok(Expr::mul(vec![a.clone(), b.clone()]))
                } else {
                    Err(CasError::Type(format!("cannot multiply {a} and {b}")))
                }
            }
        }
    }

    fn power(&self, base: &Expr, exp: &Expr) -> Result<Expr> {
        // Special cases
        if exp.is_zero() {
            return Ok(Expr::integer(1));
        }
        if exp.is_one() {
            return Ok(base.clone());
        }
        if base.is_zero() {
            return Ok(Expr::integer(0));
        }
        if base.is_one() {
            return Ok(Expr::integer(1));
        }

        match (base, exp) {
            (Expr::Integer(b), Expr::Integer(e)) => {
                if *e >= 0 {
                    Ok(Expr::Integer(b.pow(*e as u32)))
                } else {
                    // Negative exponent -> rational or float
                    let denom = b.pow((-e) as u32);
                    if self.exact_mode {
                        Ok(Expr::Rational(Rational::new(1, denom)))
                    } else {
                        Ok(Expr::Float(1.0 / denom as f64))
                    }
                }
            }
            (Expr::Float(b), Expr::Integer(e)) => Ok(Expr::Float(b.powi(*e as i32))),
            (Expr::Float(b), Expr::Float(e)) => Ok(Expr::Float(b.powf(*e))),
            (Expr::Integer(b), Expr::Float(e)) => Ok(Expr::Float((*b as f64).powf(*e))),
            _ => {
                if let (Ok(b), Ok(e)) = (self.to_f64(base), self.to_f64(exp)) {
                    Ok(Expr::Float(b.powf(e)))
                } else if self.exact_mode {
                    Ok(Expr::pow(base.clone(), exp.clone()))
                } else {
                    Err(CasError::Type(format!("cannot compute {base}^{exp}")))
                }
            }
        }
    }

    fn call_function(&self, name: &str, args: &[Expr]) -> Result<Expr> {
        // Get numeric argument if single-arg function
        let arg = if args.len() == 1 {
            self.to_f64(&args[0]).ok()
        } else {
            None
        };

        // Angle conversion for trig functions
        let angle = |x: f64| match self.angle_mode {
            AngleMode::Radians => x,
            AngleMode::Degrees => x.to_radians(),
        };

        let from_angle = |x: f64| match self.angle_mode {
            AngleMode::Radians => x,
            AngleMode::Degrees => x.to_degrees(),
        };

        match (name, args.len(), arg) {
            // Trigonometric
            ("sin", 1, Some(x)) => Ok(Expr::Float(angle(x).sin())),
            ("cos", 1, Some(x)) => Ok(Expr::Float(angle(x).cos())),
            ("tan", 1, Some(x)) => Ok(Expr::Float(angle(x).tan())),
            ("asin", 1, Some(x)) => Ok(Expr::Float(from_angle(x.asin()))),
            ("acos", 1, Some(x)) => Ok(Expr::Float(from_angle(x.acos()))),
            ("atan", 1, Some(x)) => Ok(Expr::Float(from_angle(x.atan()))),
            ("sinh", 1, Some(x)) => Ok(Expr::Float(x.sinh())),
            ("cosh", 1, Some(x)) => Ok(Expr::Float(x.cosh())),
            ("tanh", 1, Some(x)) => Ok(Expr::Float(x.tanh())),
            ("asinh", 1, Some(x)) => Ok(Expr::Float(x.asinh())),
            ("acosh", 1, Some(x)) => Ok(Expr::Float(x.acosh())),
            ("atanh", 1, Some(x)) => Ok(Expr::Float(x.atanh())),

            // Exponential/logarithmic
            ("exp", 1, Some(x)) => Ok(Expr::Float(x.exp())),
            ("ln", 1, Some(x)) => Ok(Expr::Float(x.ln())),
            ("log", 1, Some(x)) => Ok(Expr::Float(x.log10())),
            ("log10", 1, Some(x)) => Ok(Expr::Float(x.log10())),
            ("log2", 1, Some(x)) => Ok(Expr::Float(x.log2())),

            // Roots
            ("sqrt", 1, Some(x)) => {
                if x >= 0.0 {
                    Ok(Expr::Float(x.sqrt()))
                } else {
                    Ok(Expr::Complex(0.0, (-x).sqrt()))
                }
            }
            ("cbrt", 1, Some(x)) => Ok(Expr::Float(x.cbrt())),

            // Other
            ("abs", 1, Some(x)) => Ok(Expr::Float(x.abs())),
            ("floor", 1, Some(x)) => Ok(Expr::Integer(x.floor() as i64)),
            ("ceil", 1, Some(x)) => Ok(Expr::Integer(x.ceil() as i64)),
            ("round", 1, Some(x)) => Ok(Expr::Integer(x.round() as i64)),
            ("sign", 1, Some(x)) => Ok(Expr::Integer(if x > 0.0 {
                1
            } else if x < 0.0 {
                -1
            } else {
                0
            })),

            // Factorial
            ("factorial", 1, _) => {
                if let Expr::Integer(n) = &args[0] {
                    if *n < 0 {
                        Err(CasError::Domain("factorial of negative number".to_string()))
                    } else if *n > 20 {
                        // Use Stirling's approximation for large n
                        Ok(Expr::Float(gamma(*n as f64 + 1.0)))
                    } else {
                        Ok(Expr::Integer(factorial(*n as u64) as i64))
                    }
                } else if let Some(x) = arg {
                    Ok(Expr::Float(gamma(x + 1.0)))
                } else {
                    Err(CasError::Type("factorial requires a number".to_string()))
                }
            }

            // Two-argument functions
            ("atan2", 2, _) => {
                let y = self.to_f64(&args[0])?;
                let x = self.to_f64(&args[1])?;
                Ok(Expr::Float(from_angle(y.atan2(x))))
            }
            ("pow", 2, _) => self.power(&args[0], &args[1]),
            ("mod", 2, _) => {
                let a = self.to_f64(&args[0])?;
                let b = self.to_f64(&args[1])?;
                Ok(Expr::Float(a % b))
            }
            ("min", _, _) if !args.is_empty() => {
                let values: Result<Vec<f64>> = args.iter().map(|a| self.to_f64(a)).collect();
                let min = values?
                    .into_iter()
                    .fold(f64::INFINITY, |a, b| a.min(b));
                Ok(Expr::Float(min))
            }
            ("max", _, _) if !args.is_empty() => {
                let values: Result<Vec<f64>> = args.iter().map(|a| self.to_f64(a)).collect();
                let max = values?
                    .into_iter()
                    .fold(f64::NEG_INFINITY, |a, b| a.max(b));
                Ok(Expr::Float(max))
            }
            ("gcd", 2, _) => {
                if let (Expr::Integer(a), Expr::Integer(b)) = (&args[0], &args[1]) {
                    Ok(Expr::Integer(gcd(*a, *b)))
                } else {
                    Err(CasError::Type("gcd requires integers".to_string()))
                }
            }
            ("lcm", 2, _) => {
                if let (Expr::Integer(a), Expr::Integer(b)) = (&args[0], &args[1]) {
                    Ok(Expr::Integer(lcm(*a, *b)))
                } else {
                    Err(CasError::Type("lcm requires integers".to_string()))
                }
            }

            // Symbolic operations
            ("diff", 2, _) | ("derivative", 2, _) => {
                // diff(expr, var)
                if let Expr::Symbol(var) = &args[1] {
                    let result = Differentiator::diff(&args[0], var)?;
                    Ok(Simplifier::simplify(&result))
                } else {
                    Err(CasError::Type("diff requires variable as second argument".to_string()))
                }
            }
            ("diff", 3, _) | ("derivative", 3, _) => {
                // diff(expr, var, order)
                if let (Expr::Symbol(var), Expr::Integer(n)) = (&args[1], &args[2]) {
                    let result = Differentiator::diff_n(&args[0], var, *n as u32)?;
                    Ok(Simplifier::simplify(&result))
                } else {
                    Err(CasError::Type("diff requires variable and integer order".to_string()))
                }
            }

            ("integrate", 2, _) | ("integral", 2, _) => {
                // integrate(expr, var)
                if let Expr::Symbol(var) = &args[1] {
                    let result = Integrator::integrate(&args[0], var)?;
                    Ok(Simplifier::simplify(&result))
                } else {
                    Err(CasError::Type("integrate requires variable as second argument".to_string()))
                }
            }
            ("integrate", 4, _) | ("integral", 4, _) => {
                // integrate(expr, var, lower, upper)
                if let Expr::Symbol(var) = &args[1] {
                    let result = Integrator::integrate_definite(&args[0], var, &args[2], &args[3])?;
                    // Try to evaluate the result numerically
                    self.eval(&Simplifier::simplify(&result))
                } else {
                    Err(CasError::Type("integrate requires variable as second argument".to_string()))
                }
            }

            ("solve", 2, _) => {
                // solve(expr, var) or solve(equation, var)
                if let Expr::Symbol(var) = &args[1] {
                    let solutions = Solver::solve(&args[0], var)?;
                    if solutions.len() == 1 {
                        Ok(solutions.into_iter().next().unwrap())
                    } else {
                        Ok(Expr::Vector(solutions))
                    }
                } else {
                    Err(CasError::Type("solve requires variable as second argument".to_string()))
                }
            }

            ("simplify", 1, _) => {
                Ok(Simplifier::simplify(&args[0]))
            }

            ("expand", 1, _) => {
                Ok(Simplifier::simplify(&Simplifier::expand(&args[0])))
            }

            ("factor", 1, _) => {
                // Basic factoring - just return simplified for now
                // Full factoring is complex, can add later
                Ok(Simplifier::simplify(&args[0]))
            }

            ("substitute", 3, _) | ("subs", 3, _) => {
                // substitute(expr, var, replacement)
                if let Expr::Symbol(var) = &args[1] {
                    Ok(Simplifier::simplify(&Simplifier::substitute(&args[0], var, &args[2])))
                } else {
                    Err(CasError::Type("substitute requires variable as second argument".to_string()))
                }
            }

            ("limit", 3, _) | ("lim", 3, _) => {
                // limit(expr, var, point)
                if let Expr::Symbol(var) = &args[1] {
                    let result = Limits::limit(&args[0], var, &args[2], None)?;
                    self.eval(&Simplifier::simplify(&result))
                } else {
                    Err(CasError::Type("limit requires variable as second argument".to_string()))
                }
            }
            ("limit", 4, _) | ("lim", 4, _) => {
                // limit(expr, var, point, direction) where direction is "left", "right", "-", "+"
                if let Expr::Symbol(var) = &args[1] {
                    let direction = match &args[3] {
                        Expr::Symbol(s) if s.as_str() == "left" || s.as_str() == "-" => {
                            Some(crate::expr::LimitDirection::Left)
                        }
                        Expr::Symbol(s) if s.as_str() == "right" || s.as_str() == "+" => {
                            Some(crate::expr::LimitDirection::Right)
                        }
                        _ => None,
                    };
                    let result = Limits::limit(&args[0], var, &args[2], direction)?;
                    self.eval(&Simplifier::simplify(&result))
                } else {
                    Err(CasError::Type("limit requires variable as second argument".to_string()))
                }
            }

            // Matrix operations
            ("det", 1, _) | ("determinant", 1, _) => {
                if let Expr::Matrix(rows) = &args[0] {
                    self.matrix_det(rows)
                } else {
                    Err(CasError::Type("det requires a matrix argument".to_string()))
                }
            }

            ("inv", 1, _) | ("inverse", 1, _) => {
                if let Expr::Matrix(rows) = &args[0] {
                    self.matrix_inv(rows)
                } else {
                    Err(CasError::Type("inv requires a matrix argument".to_string()))
                }
            }

            ("transpose", 1, _) | ("T", 1, _) => {
                if let Expr::Matrix(rows) = &args[0] {
                    self.matrix_transpose(rows)
                } else {
                    Err(CasError::Type("transpose requires a matrix argument".to_string()))
                }
            }

            ("trace", 1, _) | ("tr", 1, _) => {
                if let Expr::Matrix(rows) = &args[0] {
                    self.matrix_trace(rows)
                } else {
                    Err(CasError::Type("trace requires a matrix argument".to_string()))
                }
            }

            ("matmul", 2, _) => {
                if let (Expr::Matrix(a), Expr::Matrix(b)) = (&args[0], &args[1]) {
                    self.matrix_mul(a, b)
                } else {
                    Err(CasError::Type("matmul requires two matrix arguments".to_string()))
                }
            }

            ("identity", 1, Some(n)) => {
                let n = n as usize;
                if n == 0 || n > 100 {
                    return Err(CasError::EvaluationError("identity matrix size must be 1-100".to_string()));
                }
                let mut rows = Vec::with_capacity(n);
                for i in 0..n {
                    let mut row = vec![Expr::Integer(0); n];
                    row[i] = Expr::Integer(1);
                    rows.push(row);
                }
                Ok(Expr::Matrix(rows))
            }

            _ => {
                if self.exact_mode {
                    Ok(Expr::func(name, args.to_vec()))
                } else {
                    Err(CasError::UndefinedFunction(name.to_string()))
                }
            }
        }
    }

    /// Compute matrix determinant
    fn matrix_det(&self, rows: &[Vec<Expr>]) -> Result<Expr> {
        let n = rows.len();
        if n == 0 {
            return Err(CasError::EvaluationError("empty matrix".to_string()));
        }
        if rows.iter().any(|r| r.len() != n) {
            return Err(CasError::EvaluationError("det requires square matrix".to_string()));
        }

        // Convert to f64 for numerical computation
        let mut matrix: Vec<Vec<f64>> = Vec::with_capacity(n);
        for row in rows {
            let mut num_row = Vec::with_capacity(n);
            for elem in row {
                num_row.push(self.to_f64(elem)?);
            }
            matrix.push(num_row);
        }

        // LU decomposition for determinant
        let det = self.det_lu(&mut matrix, n);

        // Return as integer if close to integer
        if det.fract().abs() < 1e-10 {
            Ok(Expr::Integer(det.round() as i64))
        } else {
            Ok(Expr::Float(det))
        }
    }

    /// LU decomposition determinant
    fn det_lu(&self, matrix: &mut [Vec<f64>], n: usize) -> f64 {
        let mut det = 1.0;

        for col in 0..n {
            // Find pivot
            let mut max_row = col;
            for row in (col + 1)..n {
                if matrix[row][col].abs() > matrix[max_row][col].abs() {
                    max_row = row;
                }
            }

            if max_row != col {
                matrix.swap(col, max_row);
                det = -det; // Swap changes sign
            }

            if matrix[col][col].abs() < 1e-15 {
                return 0.0; // Singular matrix
            }

            det *= matrix[col][col];

            for row in (col + 1)..n {
                let factor = matrix[row][col] / matrix[col][col];
                for j in col..n {
                    matrix[row][j] -= factor * matrix[col][j];
                }
            }
        }

        det
    }

    /// Compute matrix inverse using Gauss-Jordan elimination
    fn matrix_inv(&self, rows: &[Vec<Expr>]) -> Result<Expr> {
        let n = rows.len();
        if n == 0 {
            return Err(CasError::EvaluationError("empty matrix".to_string()));
        }
        if rows.iter().any(|r| r.len() != n) {
            return Err(CasError::EvaluationError("inv requires square matrix".to_string()));
        }

        // Convert to f64
        let mut aug: Vec<Vec<f64>> = Vec::with_capacity(n);
        for (i, row) in rows.iter().enumerate() {
            let mut aug_row = Vec::with_capacity(2 * n);
            for elem in row {
                aug_row.push(self.to_f64(elem)?);
            }
            // Append identity matrix
            for j in 0..n {
                aug_row.push(if i == j { 1.0 } else { 0.0 });
            }
            aug.push(aug_row);
        }

        // Gauss-Jordan elimination
        for col in 0..n {
            // Find pivot
            let mut max_row = col;
            for row in (col + 1)..n {
                if aug[row][col].abs() > aug[max_row][col].abs() {
                    max_row = row;
                }
            }
            aug.swap(col, max_row);

            if aug[col][col].abs() < 1e-15 {
                return Err(CasError::EvaluationError("matrix is singular".to_string()));
            }

            // Scale pivot row
            let pivot = aug[col][col];
            for j in 0..(2 * n) {
                aug[col][j] /= pivot;
            }

            // Eliminate column
            for row in 0..n {
                if row != col {
                    let factor = aug[row][col];
                    for j in 0..(2 * n) {
                        aug[row][j] -= factor * aug[col][j];
                    }
                }
            }
        }

        // Extract inverse from right half
        let mut result = Vec::with_capacity(n);
        for row in &aug {
            let mut result_row = Vec::with_capacity(n);
            for j in n..(2 * n) {
                let val = row[j];
                if val.fract().abs() < 1e-10 {
                    result_row.push(Expr::Integer(val.round() as i64));
                } else {
                    result_row.push(Expr::Float(val));
                }
            }
            result.push(result_row);
        }

        Ok(Expr::Matrix(result))
    }

    /// Transpose a matrix
    fn matrix_transpose(&self, rows: &[Vec<Expr>]) -> Result<Expr> {
        if rows.is_empty() {
            return Ok(Expr::Matrix(vec![]));
        }
        let n_rows = rows.len();
        let n_cols = rows[0].len();

        let mut result = Vec::with_capacity(n_cols);
        for j in 0..n_cols {
            let mut new_row = Vec::with_capacity(n_rows);
            for row in rows {
                if j < row.len() {
                    new_row.push(row[j].clone());
                } else {
                    new_row.push(Expr::Integer(0));
                }
            }
            result.push(new_row);
        }

        Ok(Expr::Matrix(result))
    }

    /// Compute trace (sum of diagonal)
    fn matrix_trace(&self, rows: &[Vec<Expr>]) -> Result<Expr> {
        let n = rows.len();
        if n == 0 {
            return Err(CasError::EvaluationError("empty matrix".to_string()));
        }
        if rows.iter().any(|r| r.len() != n) {
            return Err(CasError::EvaluationError("trace requires square matrix".to_string()));
        }

        let mut sum = 0.0;
        for i in 0..n {
            sum += self.to_f64(&rows[i][i])?;
        }

        if sum.fract().abs() < 1e-10 {
            Ok(Expr::Integer(sum.round() as i64))
        } else {
            Ok(Expr::Float(sum))
        }
    }

    /// Matrix multiplication
    fn matrix_mul(&self, a: &[Vec<Expr>], b: &[Vec<Expr>]) -> Result<Expr> {
        if a.is_empty() || b.is_empty() {
            return Err(CasError::EvaluationError("empty matrix".to_string()));
        }

        let m = a.len();
        let n = a[0].len();
        let p = b[0].len();

        if b.len() != n {
            return Err(CasError::EvaluationError(format!(
                "matrix dimensions don't match for multiplication: {}x{} * {}x{}",
                m, n, b.len(), p
            )));
        }

        // Convert to f64
        let a_num: Vec<Vec<f64>> = a.iter()
            .map(|row| row.iter().map(|e| self.to_f64(e)).collect::<Result<Vec<_>>>())
            .collect::<Result<Vec<_>>>()?;
        let b_num: Vec<Vec<f64>> = b.iter()
            .map(|row| row.iter().map(|e| self.to_f64(e)).collect::<Result<Vec<_>>>())
            .collect::<Result<Vec<_>>>()?;

        let mut result = Vec::with_capacity(m);
        for i in 0..m {
            let mut row = Vec::with_capacity(p);
            for j in 0..p {
                let mut sum = 0.0;
                for k in 0..n {
                    sum += a_num[i][k] * b_num[k][j];
                }
                if sum.fract().abs() < 1e-10 {
                    row.push(Expr::Integer(sum.round() as i64));
                } else {
                    row.push(Expr::Float(sum));
                }
            }
            result.push(row);
        }

        Ok(Expr::Matrix(result))
    }
}

// Helper functions

fn factorial(n: u64) -> u64 {
    (1..=n).product()
}

fn gcd(a: i64, b: i64) -> i64 {
    let (a, b) = (a.abs(), b.abs());
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

fn lcm(a: i64, b: i64) -> i64 {
    (a * b).abs() / gcd(a, b)
}

/// Gamma function approximation (Lanczos)
fn gamma(x: f64) -> f64 {
    if x < 0.5 {
        PI / (PI * x).sin() / gamma(1.0 - x)
    } else {
        let x = x - 1.0;
        let g = 7.0;
        let c = [
            0.99999999999980993,
            676.5203681218851,
            -1259.1392167224028,
            771.32342877765313,
            -176.61502916214059,
            12.507343278686905,
            -0.13857109526572012,
            9.9843695780195716e-6,
            1.5056327351493116e-7,
        ];

        let mut sum = c[0];
        for (i, &ci) in c.iter().enumerate().skip(1) {
            sum += ci / (x + i as f64);
        }

        (2.0 * PI).sqrt() * (x + g + 0.5).powf(x + 0.5) * (-(x + g + 0.5)).exp() * sum
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn eval(input: &str) -> Result<Expr> {
        let expr = parse(input)?;
        Evaluator::new().eval(&expr)
    }

    fn eval_to_f64(input: &str) -> f64 {
        match eval(input).unwrap() {
            Expr::Integer(n) => n as f64,
            Expr::Float(x) => x,
            other => panic!("expected number, got {other}"),
        }
    }

    #[test]
    fn test_arithmetic() {
        assert_eq!(eval("2 + 3").unwrap(), Expr::Integer(5));
        assert_eq!(eval("10 - 4").unwrap(), Expr::Integer(6));
        assert_eq!(eval("3 * 4").unwrap(), Expr::Integer(12));
        assert_eq!(eval("2^10").unwrap(), Expr::Integer(1024));
    }

    #[test]
    fn test_float() {
        let result = eval_to_f64("3.14 * 2");
        assert!((result - 6.28).abs() < 1e-10);
    }

    #[test]
    fn test_functions() {
        let result = eval_to_f64("sin(0)");
        assert!(result.abs() < 1e-10);

        let result = eval_to_f64("cos(0)");
        assert!((result - 1.0).abs() < 1e-10);

        let result = eval_to_f64("sqrt(4)");
        assert!((result - 2.0).abs() < 1e-10);

        let result = eval_to_f64("ln(e)");
        assert!((result - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_constants() {
        let result = eval_to_f64("pi");
        assert!((result - PI).abs() < 1e-10);

        let result = eval_to_f64("e");
        assert!((result - E).abs() < 1e-10);
    }

    #[test]
    fn test_factorial() {
        assert_eq!(eval("5!").unwrap(), Expr::Integer(120));
    }

    #[test]
    fn test_complex_expr() {
        let result = eval_to_f64("2 * sin(pi/2) + 1");
        assert!((result - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_variables() {
        let expr = parse("x + 1").unwrap();
        let mut evaluator = Evaluator::new();
        evaluator.set_var("x", Expr::integer(5));
        let result = evaluator.eval(&expr).unwrap();
        assert_eq!(result, Expr::Integer(6));
    }
}
