//! Expression evaluation
//!
//! Supports both numeric and symbolic evaluation modes.

use std::collections::BTreeSet;
use std::collections::HashMap;
use std::f64::consts::{E, PI};

use crate::error::{CasError, Result};
use crate::expr::{Expr, Rational, Sign, Symbol};
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
                    "diff" | "derivative" | "integrate" | "integral" | "limit" | "lim"
                    | "solve" | "simplify" | "expand" | "factor" | "substitute" | "subs" => {
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
            Expr::Derivative {
                expr: inner,
                var,
                order,
            } => {
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

            Expr::Integral {
                expr: inner,
                var,
                lower,
                upper,
            } => {
                if let (Some(l), Some(u)) = (lower, upper) {
                    // Definite integral - try symbolic antiderivative first.
                    let result = Integrator::integrate_definite(inner, var, l, u)?;
                    let simplified = Simplifier::simplify(&result);
                    if Self::is_unevaluated_definite_integral(&simplified) {
                        // If no closed form is available, fall back to numerical quadrature.
                        match self.eval_definite_integral_numeric(inner, var, l, u) {
                            Ok(value) => Ok(value),
                            Err(_) => Ok(simplified),
                        }
                    } else {
                        self.eval(&simplified)
                    }
                } else {
                    // Indefinite integral - return symbolic result
                    let result = Integrator::integrate(inner, var)?;
                    Ok(Simplifier::simplify(&result))
                }
            }

            Expr::Limit {
                expr: inner,
                var,
                point,
                direction,
            } => {
                let result = Limits::limit(inner, var, point, *direction)?;
                let simplified = Simplifier::simplify(&result);
                self.eval(&simplified)
            }

            Expr::Sum {
                expr: inner,
                var,
                lower,
                upper,
            } => self.eval_sum(inner, var, lower, upper),

            Expr::Product {
                expr: inner,
                var,
                lower,
                upper,
            } => self.eval_product(inner, var, lower, upper),

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
            | (Expr::Integer(n), Expr::Complex(re, im)) => Ok(Expr::Complex(re + *n as f64, *im)),
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
        if a.is_one() {
            return Ok(b.clone());
        }
        if b.is_one() {
            return Ok(a.clone());
        }
        if a.is_negative_one() {
            return self.negate(b);
        }
        if b.is_negative_one() {
            return self.negate(a);
        }

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
            (Expr::Rational(r1), Expr::Rational(r2)) => Ok(Expr::Rational(Rational::new(
                r1.num * r2.num,
                r1.den * r2.den,
            ))),
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
                let min = values?.into_iter().fold(f64::INFINITY, |a, b| a.min(b));
                Ok(Expr::Float(min))
            }
            ("max", _, _) if !args.is_empty() => {
                let values: Result<Vec<f64>> = args.iter().map(|a| self.to_f64(a)).collect();
                let max = values?.into_iter().fold(f64::NEG_INFINITY, |a, b| a.max(b));
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
                    Err(CasError::Type(
                        "diff requires variable as second argument".to_string(),
                    ))
                }
            }
            ("diff", 3, _) | ("derivative", 3, _) => {
                // diff(expr, var, order)
                if let (Expr::Symbol(var), Expr::Integer(n)) = (&args[1], &args[2]) {
                    let result = Differentiator::diff_n(&args[0], var, *n as u32)?;
                    Ok(Simplifier::simplify(&result))
                } else {
                    Err(CasError::Type(
                        "diff requires variable and integer order".to_string(),
                    ))
                }
            }

            ("integrate", 2, _) | ("integral", 2, _) => {
                // integrate(expr, var)
                if let Expr::Symbol(var) = &args[1] {
                    let result = Integrator::integrate(&args[0], var)?;
                    Ok(Simplifier::simplify(&result))
                } else {
                    Err(CasError::Type(
                        "integrate requires variable as second argument".to_string(),
                    ))
                }
            }
            ("integrate", 4, _) | ("integral", 4, _) => {
                // integrate(expr, var, lower, upper)
                if let Expr::Symbol(var) = &args[1] {
                    let result = Integrator::integrate_definite(&args[0], var, &args[2], &args[3])?;
                    let simplified = Simplifier::simplify(&result);
                    if Self::is_unevaluated_definite_integral(&simplified) {
                        match self.eval_definite_integral_numeric(&args[0], var, &args[2], &args[3])
                        {
                            Ok(value) => Ok(value),
                            Err(_) => Ok(simplified),
                        }
                    } else {
                        self.eval(&simplified)
                    }
                } else {
                    Err(CasError::Type(
                        "integrate requires variable as second argument".to_string(),
                    ))
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
                    Err(CasError::Type(
                        "solve requires variable as second argument".to_string(),
                    ))
                }
            }

            ("sum", 4, _) => {
                if let Expr::Symbol(var) = &args[1] {
                    self.eval_sum(&args[0], var, &args[2], &args[3])
                } else {
                    Err(CasError::Type(
                        "sum requires variable as second argument".to_string(),
                    ))
                }
            }

            ("product", 4, _) | ("prod", 4, _) => {
                if let Expr::Symbol(var) = &args[1] {
                    self.eval_product(&args[0], var, &args[2], &args[3])
                } else {
                    Err(CasError::Type(
                        "product requires variable as second argument".to_string(),
                    ))
                }
            }

            ("simplify", 1, _) => Ok(Simplifier::simplify(&args[0])),

            ("expand", 1, _) => Ok(Simplifier::simplify(&Simplifier::expand(&args[0]))),

            ("factor", 1, _) => {
                // Basic factoring - just return simplified for now
                // Full factoring is complex, can add later
                Ok(Simplifier::simplify(&args[0]))
            }

            ("substitute", 3, _) | ("subs", 3, _) => {
                // substitute(expr, var, replacement)
                if let Expr::Symbol(var) = &args[1] {
                    Ok(Simplifier::simplify(&Simplifier::substitute(
                        &args[0], var, &args[2],
                    )))
                } else {
                    Err(CasError::Type(
                        "substitute requires variable as second argument".to_string(),
                    ))
                }
            }

            ("limit", 3, _) | ("lim", 3, _) => {
                // limit(expr, var, point)
                if let Expr::Symbol(var) = &args[1] {
                    let result = Limits::limit(&args[0], var, &args[2], None)?;
                    self.eval(&Simplifier::simplify(&result))
                } else {
                    Err(CasError::Type(
                        "limit requires variable as second argument".to_string(),
                    ))
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
                    Err(CasError::Type(
                        "limit requires variable as second argument".to_string(),
                    ))
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
                    Err(CasError::Type(
                        "transpose requires a matrix argument".to_string(),
                    ))
                }
            }

            ("trace", 1, _) | ("tr", 1, _) => {
                if let Expr::Matrix(rows) = &args[0] {
                    self.matrix_trace(rows)
                } else {
                    Err(CasError::Type(
                        "trace requires a matrix argument".to_string(),
                    ))
                }
            }

            ("matmul", 2, _) => {
                if let (Expr::Matrix(a), Expr::Matrix(b)) = (&args[0], &args[1]) {
                    self.matrix_mul(a, b)
                } else {
                    Err(CasError::Type(
                        "matmul requires two matrix arguments".to_string(),
                    ))
                }
            }

            ("identity", 1, Some(n)) => {
                let n = n as usize;
                if n == 0 || n > 100 {
                    return Err(CasError::EvaluationError(
                        "identity matrix size must be 1-100".to_string(),
                    ));
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
            return Err(CasError::EvaluationError(
                "det requires square matrix".to_string(),
            ));
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
            return Err(CasError::EvaluationError(
                "inv requires square matrix".to_string(),
            ));
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
            return Err(CasError::EvaluationError(
                "trace requires square matrix".to_string(),
            ));
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
                m,
                n,
                b.len(),
                p
            )));
        }

        // Convert to f64
        let a_num: Vec<Vec<f64>> = a
            .iter()
            .map(|row| {
                row.iter()
                    .map(|e| self.to_f64(e))
                    .collect::<Result<Vec<_>>>()
            })
            .collect::<Result<Vec<_>>>()?;
        let b_num: Vec<Vec<f64>> = b
            .iter()
            .map(|row| {
                row.iter()
                    .map(|e| self.to_f64(e))
                    .collect::<Result<Vec<_>>>()
            })
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

    fn eval_integer_bound(&self, bound: &Expr) -> Result<i64> {
        let value = self.eval(bound)?;
        match value {
            Expr::Integer(n) => Ok(n),
            Expr::Rational(r) if r.den == 1 => Ok(r.num),
            Expr::Float(x) => {
                if !x.is_finite() {
                    return Err(CasError::Type("bound must be a finite number".to_string()));
                }
                let rounded = x.round();
                if (x - rounded).abs() < 1e-10
                    && rounded >= i64::MIN as f64
                    && rounded <= i64::MAX as f64
                {
                    Ok(rounded as i64)
                } else {
                    Err(CasError::Type(format!(
                        "bound must be an integer, got {}",
                        Expr::Float(x)
                    )))
                }
            }
            _ => Err(CasError::Type(format!(
                "bound must be an integer, got {value}"
            ))),
        }
    }

    fn is_unevaluated_definite_integral(expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::Integral {
                lower: Some(_),
                upper: Some(_),
                ..
            }
        )
    }

    fn eval_definite_integral_numeric(
        &self,
        body: &Expr,
        var: &Symbol,
        lower: &Expr,
        upper: &Expr,
    ) -> Result<Expr> {
        let lower_eval = self.eval(lower)?;
        let upper_eval = self.eval(upper)?;
        let mut a = self.to_f64(&lower_eval)?;
        let mut b = self.to_f64(&upper_eval)?;

        if !a.is_finite() || !b.is_finite() {
            return Err(CasError::Type(
                "integral bounds must be finite numbers".to_string(),
            ));
        }

        if (a - b).abs() < 1e-14 {
            return Ok(Expr::Integer(0));
        }

        let mut sign = 1.0;
        if a > b {
            std::mem::swap(&mut a, &mut b);
            sign = -1.0;
        }

        let mut slices = 64usize;
        let mut estimate = self.simpson_integral(body, var, a, b, slices)?;
        for _ in 0..8 {
            slices *= 2;
            let refined = self.simpson_integral(body, var, a, b, slices)?;
            if (refined - estimate).abs() <= 1e-10 * (1.0 + refined.abs()) {
                return Ok(Self::float_to_expr(sign * refined));
            }
            estimate = refined;
        }

        Ok(Self::float_to_expr(sign * estimate))
    }

    fn simpson_integral(
        &self,
        body: &Expr,
        var: &Symbol,
        a: f64,
        b: f64,
        slices: usize,
    ) -> Result<f64> {
        if slices == 0 || slices % 2 != 0 {
            return Err(CasError::EvaluationError(
                "simpson integration requires a positive even number of slices".to_string(),
            ));
        }

        let h = (b - a) / slices as f64;
        let mut acc =
            self.eval_integrand_point(body, var, a)? + self.eval_integrand_point(body, var, b)?;

        for i in 1..slices {
            let x = a + i as f64 * h;
            let fx = self.eval_integrand_point(body, var, x)?;
            if i % 2 == 0 {
                acc += 2.0 * fx;
            } else {
                acc += 4.0 * fx;
            }
        }

        Ok(acc * h / 3.0)
    }

    fn eval_integrand_point(&self, body: &Expr, var: &Symbol, x: f64) -> Result<f64> {
        let substituted = Simplifier::substitute(body, var, &Expr::Float(x));
        let evaluated = self.eval(&substituted)?;
        let value = self.to_f64(&evaluated)?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(CasError::EvaluationError(format!(
                "integrand is not finite at {x}"
            )))
        }
    }

    fn float_to_expr(x: f64) -> Expr {
        if !x.is_finite() {
            return Expr::Float(x);
        }
        let rounded = x.round();
        if (x - rounded).abs() < 1e-10 && rounded >= i64::MIN as f64 && rounded <= i64::MAX as f64 {
            Expr::Integer(rounded as i64)
        } else {
            Expr::Float(x)
        }
    }

    fn eval_sum(
        &self,
        body: &Expr,
        var: &crate::expr::Symbol,
        lower: &Expr,
        upper: &Expr,
    ) -> Result<Expr> {
        let inferred_var;
        let active_var = if body.contains_var(var) {
            var
        } else {
            inferred_var = Self::infer_iteration_var(body);
            inferred_var.as_ref().unwrap_or(var)
        };

        match self.eval_discrete_series(body, active_var, lower, upper, false) {
            Ok(value) => Ok(value),
            Err(_err) => {
                if let Some(symbolic) = Self::symbolic_sum(body, active_var, lower, upper) {
                    return Ok(Simplifier::simplify(&symbolic));
                }

                Ok(Expr::Sum {
                    expr: Box::new(body.clone()),
                    var: active_var.clone(),
                    lower: Box::new(lower.clone()),
                    upper: Box::new(upper.clone()),
                })
            }
        }
    }

    fn eval_product(
        &self,
        body: &Expr,
        var: &crate::expr::Symbol,
        lower: &Expr,
        upper: &Expr,
    ) -> Result<Expr> {
        let inferred_var;
        let active_var = if body.contains_var(var) {
            var
        } else {
            inferred_var = Self::infer_iteration_var(body);
            inferred_var.as_ref().unwrap_or(var)
        };

        match self.eval_discrete_series(body, active_var, lower, upper, true) {
            Ok(value) => Ok(value),
            Err(_err) => {
                if let Some(symbolic) = Self::symbolic_product(body, active_var, lower, upper) {
                    return Ok(Simplifier::simplify(&symbolic));
                }

                Ok(Expr::Product {
                    expr: Box::new(body.clone()),
                    var: active_var.clone(),
                    lower: Box::new(lower.clone()),
                    upper: Box::new(upper.clone()),
                })
            }
        }
    }

    fn infer_iteration_var(body: &Expr) -> Option<crate::expr::Symbol> {
        let mut vars = BTreeSet::new();
        Self::collect_symbols(body, &mut vars);
        if vars.len() == 1 {
            vars.into_iter().next().map(crate::expr::Symbol::new)
        } else {
            None
        }
    }

    fn collect_symbols(expr: &Expr, out: &mut BTreeSet<String>) {
        match expr {
            Expr::Symbol(s) => {
                out.insert(s.as_str().to_string());
            }
            Expr::Neg(inner) => Self::collect_symbols(inner, out),
            Expr::Add(terms) | Expr::Mul(terms) | Expr::Vector(terms) => {
                for term in terms {
                    Self::collect_symbols(term, out);
                }
            }
            Expr::Pow(base, exp) => {
                Self::collect_symbols(base, out);
                Self::collect_symbols(exp, out);
            }
            Expr::Func(_, args) => {
                for arg in args {
                    Self::collect_symbols(arg, out);
                }
            }
            Expr::Derivative { expr, .. } => Self::collect_symbols(expr, out),
            Expr::Integral {
                expr, lower, upper, ..
            } => {
                Self::collect_symbols(expr, out);
                if let Some(lo) = lower {
                    Self::collect_symbols(lo, out);
                }
                if let Some(hi) = upper {
                    Self::collect_symbols(hi, out);
                }
            }
            Expr::Limit { expr, point, .. } => {
                Self::collect_symbols(expr, out);
                Self::collect_symbols(point, out);
            }
            Expr::Sum {
                expr, lower, upper, ..
            }
            | Expr::Product {
                expr, lower, upper, ..
            } => {
                Self::collect_symbols(expr, out);
                Self::collect_symbols(lower, out);
                Self::collect_symbols(upper, out);
            }
            Expr::Equation(lhs, rhs) => {
                Self::collect_symbols(lhs, out);
                Self::collect_symbols(rhs, out);
            }
            Expr::Inequality { lhs, rhs, .. } => {
                Self::collect_symbols(lhs, out);
                Self::collect_symbols(rhs, out);
            }
            Expr::Matrix(rows) => {
                for row in rows {
                    for elem in row {
                        Self::collect_symbols(elem, out);
                    }
                }
            }
            Expr::Integer(_)
            | Expr::Rational(_)
            | Expr::Float(_)
            | Expr::Complex(_, _)
            | Expr::Undefined
            | Expr::Infinity(_) => {}
        }
    }

    fn symbolic_sum(
        body: &Expr,
        var: &crate::expr::Symbol,
        lower: &Expr,
        upper: &Expr,
    ) -> Option<Expr> {
        // Finite count for symbolic bounds: upper - lower + 1.
        let count = Expr::add(vec![
            Expr::sub(upper.clone(), lower.clone()),
            Expr::Integer(1),
        ]);

        if !body.contains_var(var) {
            return Some(Expr::mul(vec![body.clone(), count]));
        }

        match body {
            Expr::Symbol(s) if s == var => Some(Self::sum_linear(lower, upper)),

            Expr::Pow(base, exp) if matches!(base.as_ref(), Expr::Symbol(s) if s == var) => {
                match exp.as_ref() {
                    Expr::Integer(0) => Some(count),
                    Expr::Integer(1) => Some(Self::sum_linear(lower, upper)),
                    Expr::Integer(2) => Some(Self::sum_square(lower, upper)),
                    Expr::Integer(3) => Some(Self::sum_cube(lower, upper)),
                    _ => None,
                }
            }

            Expr::Neg(inner) => Self::symbolic_sum(inner, var, lower, upper).map(Expr::neg),

            Expr::Add(terms) => {
                let mut summed_terms = Vec::with_capacity(terms.len());
                for term in terms {
                    summed_terms.push(Self::symbolic_sum(term, var, lower, upper)?);
                }
                Some(Expr::add(summed_terms))
            }

            Expr::Mul(factors) => {
                let (mut independent, dependent): (Vec<Expr>, Vec<Expr>) =
                    factors.iter().cloned().partition(|f| !f.contains_var(var));

                if dependent.is_empty() {
                    independent.push(count);
                    return Some(Expr::mul(independent));
                }

                if dependent.len() == 1 {
                    let dep_sum = Self::symbolic_sum(&dependent[0], var, lower, upper)?;
                    independent.push(dep_sum);
                    return Some(Expr::mul(independent));
                }

                None
            }

            _ => None,
        }
    }

    fn sum_linear(lower: &Expr, upper: &Expr) -> Expr {
        fn triangular(x: Expr) -> Expr {
            Expr::mul(vec![
                x.clone(),
                Expr::add(vec![x, Expr::Integer(1)]),
                Expr::Rational(Rational::new(1, 2)),
            ])
        }

        let lo_minus_one = Expr::sub(lower.clone(), Expr::Integer(1));
        Expr::sub(triangular(upper.clone()), triangular(lo_minus_one))
    }

    fn sum_square(lower: &Expr, upper: &Expr) -> Expr {
        fn square_sum_prefix(x: Expr) -> Expr {
            let two_x_plus_one = Expr::add(vec![
                Expr::mul(vec![Expr::Integer(2), x.clone()]),
                Expr::Integer(1),
            ]);
            Expr::mul(vec![
                x.clone(),
                Expr::add(vec![x, Expr::Integer(1)]),
                two_x_plus_one,
                Expr::Rational(Rational::new(1, 6)),
            ])
        }

        let lo_minus_one = Expr::sub(lower.clone(), Expr::Integer(1));
        Expr::sub(
            square_sum_prefix(upper.clone()),
            square_sum_prefix(lo_minus_one),
        )
    }

    fn sum_cube(lower: &Expr, upper: &Expr) -> Expr {
        fn cube_sum_prefix(x: Expr) -> Expr {
            let tri = Expr::mul(vec![
                x.clone(),
                Expr::add(vec![x, Expr::Integer(1)]),
                Expr::Rational(Rational::new(1, 2)),
            ]);
            Expr::pow(tri, Expr::Integer(2))
        }

        let lo_minus_one = Expr::sub(lower.clone(), Expr::Integer(1));
        Expr::sub(
            cube_sum_prefix(upper.clone()),
            cube_sum_prefix(lo_minus_one),
        )
    }

    fn symbolic_product(
        body: &Expr,
        var: &crate::expr::Symbol,
        lower: &Expr,
        upper: &Expr,
    ) -> Option<Expr> {
        let count = Expr::add(vec![
            Expr::sub(upper.clone(), lower.clone()),
            Expr::Integer(1),
        ]);

        if !body.contains_var(var) {
            return Some(Expr::pow(body.clone(), count));
        }

        if let Some(factored) = Self::factor_simple_product_body(body, var) {
            return Self::symbolic_product(&factored, var, lower, upper);
        }

        match body {
            Expr::Symbol(s) if s == var => Some(Self::factorial_range(lower, upper)),
            Expr::Neg(inner) => {
                let inner_product = Self::symbolic_product(inner, var, lower, upper)?;
                Some(Expr::mul(vec![
                    Expr::pow(Expr::Integer(-1), count),
                    inner_product,
                ]))
            }
            Expr::Mul(factors) => {
                let (independent, dependent): (Vec<Expr>, Vec<Expr>) =
                    factors.iter().cloned().partition(|f| !f.contains_var(var));

                let mut parts = Vec::new();
                if !independent.is_empty() {
                    parts.push(Expr::pow(Expr::mul(independent), count.clone()));
                }

                for dep in dependent {
                    parts.push(Self::symbolic_product(&dep, var, lower, upper)?);
                }

                Some(Expr::mul(parts))
            }
            Expr::Add(_) => Self::product_linear_term(body, var, lower, upper),
            Expr::Pow(base, exp) => {
                let Expr::Integer(power) = exp.as_ref() else {
                    return None;
                };

                if matches!(base.as_ref(), Expr::Symbol(s) if s == var) {
                    return Some(Expr::pow(
                        Self::factorial_range(lower, upper),
                        Expr::Integer(*power),
                    ));
                }

                if let Some(linear_base_product) =
                    Self::product_linear_term(base, var, lower, upper)
                {
                    return Some(Expr::pow(linear_base_product, Expr::Integer(*power)));
                }

                None
            }
            _ => None,
        }
    }

    fn factorial_range(lower: &Expr, upper: &Expr) -> Expr {
        Self::factorial_range_shifted(lower, upper, 0).unwrap_or_else(|| {
            let upper_fact = Expr::func("factorial", vec![upper.clone()]);
            if matches!(lower, Expr::Integer(1)) {
                upper_fact
            } else {
                let lower_minus_one = Expr::sub(lower.clone(), Expr::Integer(1));
                let lower_fact = Expr::func("factorial", vec![lower_minus_one]);
                Expr::mul(vec![upper_fact, Expr::pow(lower_fact, Expr::Integer(-1))])
            }
        })
    }

    fn product_linear_term(
        expr: &Expr,
        var: &crate::expr::Symbol,
        lower: &Expr,
        upper: &Expr,
    ) -> Option<Expr> {
        let shift = Self::extract_linear_shift(expr, var)?;
        Self::factorial_range_shifted(lower, upper, shift)
    }

    fn extract_linear_shift(expr: &Expr, var: &crate::expr::Symbol) -> Option<i64> {
        match expr {
            Expr::Symbol(s) if s == var => Some(0),
            Expr::Add(terms) => {
                let mut saw_var = false;
                let mut shift = 0_i64;
                for term in terms {
                    match term {
                        Expr::Symbol(s) if s == var => {
                            if saw_var {
                                return None;
                            }
                            saw_var = true;
                        }
                        Expr::Integer(n) => {
                            shift = shift.checked_add(*n)?;
                        }
                        Expr::Neg(inner) => {
                            if let Expr::Integer(n) = inner.as_ref() {
                                shift = shift.checked_sub(*n)?;
                            } else {
                                return None;
                            }
                        }
                        _ => return None,
                    }
                }
                if saw_var {
                    Some(shift)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn factorial_range_shifted(lower: &Expr, upper: &Expr, shift: i64) -> Option<Expr> {
        let shift_minus_one = shift.checked_sub(1)?;
        if let Expr::Integer(lo) = lower {
            if lo.checked_add(shift_minus_one)? < 0 {
                return None;
            }
        }
        if let Expr::Integer(hi) = upper {
            if hi.checked_add(shift)? < 0 {
                return None;
            }
        }

        let shifted_upper =
            Simplifier::simplify(&Expr::add(vec![upper.clone(), Expr::Integer(shift)]));
        let shifted_lower_minus_one = Simplifier::simplify(&Expr::add(vec![
            lower.clone(),
            Expr::Integer(shift_minus_one),
        ]));

        let upper_fact = Expr::func("factorial", vec![shifted_upper]);
        if matches!(shifted_lower_minus_one, Expr::Integer(0)) {
            Some(upper_fact)
        } else {
            let lower_fact = Expr::func("factorial", vec![shifted_lower_minus_one]);
            Some(Expr::mul(vec![
                upper_fact,
                Expr::pow(lower_fact, Expr::Integer(-1)),
            ]))
        }
    }

    fn factor_simple_product_body(body: &Expr, var: &crate::expr::Symbol) -> Option<Expr> {
        let Expr::Add(terms) = body else {
            return None;
        };
        if terms.len() != 2 {
            return None;
        }

        let mut has_square = false;
        let mut linear_sign = 0_i64;
        for term in terms {
            match term {
                Expr::Pow(base, exp)
                    if matches!(base.as_ref(), Expr::Symbol(s) if s == var)
                        && matches!(exp.as_ref(), Expr::Integer(2)) =>
                {
                    has_square = true;
                }
                Expr::Symbol(s) if s == var => linear_sign += 1,
                Expr::Neg(inner) if matches!(inner.as_ref(), Expr::Symbol(s) if s == var) => {
                    linear_sign -= 1;
                }
                _ => return None,
            }
        }

        if !has_square {
            return None;
        }

        let var_expr = Expr::Symbol(var.clone());
        match linear_sign {
            1 => Some(Expr::mul(vec![
                var_expr.clone(),
                Expr::add(vec![var_expr, Expr::Integer(1)]),
            ])),
            -1 => Some(Expr::mul(vec![
                var_expr.clone(),
                Expr::add(vec![var_expr, Expr::Integer(-1)]),
            ])),
            _ => None,
        }
    }

    fn eval_discrete_series(
        &self,
        body: &Expr,
        var: &crate::expr::Symbol,
        lower: &Expr,
        upper: &Expr,
        is_product: bool,
    ) -> Result<Expr> {
        const MAX_TERMS: i64 = 100_000;

        let lo = self.eval_integer_bound(lower)?;
        let hi = self.eval_integer_bound(upper)?;

        if lo > hi {
            return Ok(if is_product {
                Expr::Integer(1)
            } else {
                Expr::Integer(0)
            });
        }

        let count = hi.saturating_sub(lo).saturating_add(1);
        if count > MAX_TERMS {
            let kind = if is_product { "product" } else { "sum" };
            return Err(CasError::EvaluationError(format!(
                "{kind} has too many terms ({count}); limit is {MAX_TERMS}"
            )));
        }

        let mut acc = if is_product {
            Expr::Integer(1)
        } else {
            Expr::Integer(0)
        };

        for n in lo..=hi {
            let substituted = Simplifier::substitute(body, var, &Expr::Integer(n));
            let term = self.eval(&substituted)?;
            acc = if is_product {
                self.multiply(&acc, &term)?
            } else {
                self.add(&acc, &term)?
            };
        }

        Ok(Simplifier::simplify(&acc))
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

    fn expr_to_f64(expr: &Expr) -> f64 {
        match expr {
            Expr::Integer(n) => *n as f64,
            Expr::Rational(r) => r.to_f64(),
            Expr::Float(x) => *x,
            other => panic!("expected numeric expression, got {other}"),
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

    #[test]
    fn test_sum_evaluation() {
        assert_eq!(eval("sum(n, n, 1, 5)").unwrap(), Expr::Integer(15));
    }

    #[test]
    fn test_product_evaluation() {
        assert_eq!(eval("product(n, n, 1, 4)").unwrap(), Expr::Integer(24));
    }

    #[test]
    fn test_definite_integral_numeric_fallback_for_non_elementary_antiderivative() {
        let result = eval_to_f64("integrate(x^(x+2), x, 0, 1)");
        assert!((result - 0.2781176122).abs() < 1e-8);
    }

    #[test]
    fn test_definite_integral_with_symbolic_bounds_stays_symbolic() {
        let symbolic = eval("integrate(x^(x+2), x, 0, n)").unwrap();
        assert!(matches!(
            symbolic,
            Expr::Integral {
                lower: Some(_),
                upper: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn test_empty_discrete_range() {
        assert_eq!(eval("sum(n, n, 5, 1)").unwrap(), Expr::Integer(0));
        assert_eq!(eval("product(n, n, 5, 1)").unwrap(), Expr::Integer(1));
    }

    #[test]
    fn test_symbolic_solve_function() {
        let result = eval("solve(x^2 - 4, x)").unwrap();
        if let Expr::Vector(solutions) = result {
            assert_eq!(solutions.len(), 2);
            let values: Vec<f64> = solutions
                .iter()
                .map(|s| match s {
                    Expr::Integer(n) => *n as f64,
                    Expr::Rational(r) => r.to_f64(),
                    Expr::Float(x) => *x,
                    other => panic!("expected numeric solution, got {other}"),
                })
                .collect();
            assert!(values.iter().any(|v| (*v - 2.0).abs() < 1e-10));
            assert!(values.iter().any(|v| (*v + 2.0).abs() < 1e-10));
        } else {
            panic!("expected vector of solutions");
        }
    }

    #[test]
    fn test_symbolic_sum_linear_closed_form() {
        let symbolic = eval("sum(k, k, 1, n)").unwrap();
        let mut evaluator = Evaluator::new();
        evaluator.set_var("n", Expr::Integer(10));
        let value = evaluator.eval(&symbolic).unwrap();
        assert!((expr_to_f64(&value) - 55.0).abs() < 1e-10);
    }

    #[test]
    fn test_symbolic_sum_quadratic_closed_form() {
        let symbolic = eval("sum(k^2 + k, k, 1, n)").unwrap();
        let mut evaluator = Evaluator::new();
        evaluator.set_var("n", Expr::Integer(5));
        let value = evaluator.eval(&symbolic).unwrap();
        assert!((expr_to_f64(&value) - 70.0).abs() < 1e-10);
    }

    #[test]
    fn test_symbolic_sum_fallback_for_unreduced_form() {
        let symbolic = eval("sum(1/(k-1), k, 1, n)").unwrap();
        assert!(matches!(symbolic, Expr::Sum { .. }));
    }

    #[test]
    fn test_symbolic_product_constant_closed_form() {
        let symbolic = eval("product(2, k, 1, n)").unwrap();
        let mut evaluator = Evaluator::new();
        evaluator.set_var("n", Expr::Integer(5));
        let value = evaluator.eval(&symbolic).unwrap();
        assert!((expr_to_f64(&value) - 32.0).abs() < 1e-10);
    }

    #[test]
    fn test_symbolic_product_factorial_closed_form() {
        let symbolic = eval("product(k, k, 1, n)").unwrap();
        let mut evaluator = Evaluator::new();
        evaluator.set_var("n", Expr::Integer(5));
        let value = evaluator.eval(&symbolic).unwrap();
        assert!((expr_to_f64(&value) - 120.0).abs() < 1e-10);
    }

    #[test]
    fn test_symbolic_product_linear_shift_closed_form() {
        let symbolic = eval("product(k+1, k, 1, n)").unwrap();
        let mut evaluator = Evaluator::new();
        evaluator.set_var("n", Expr::Integer(5));
        let value = evaluator.eval(&symbolic).unwrap();
        assert!((expr_to_f64(&value) - 720.0).abs() < 1e-10);
    }

    #[test]
    fn test_symbolic_product_mul_decomposition_closed_form() {
        let symbolic = eval("product(2*k, k, 1, n)").unwrap();
        let mut evaluator = Evaluator::new();
        evaluator.set_var("n", Expr::Integer(5));
        let value = evaluator.eval(&symbolic).unwrap();
        assert!((expr_to_f64(&value) - 3840.0).abs() < 1e-10);
    }

    #[test]
    fn test_symbolic_product_simple_quadratic_factoring() {
        let symbolic = eval("product(k^2 + k, k, 1, n)").unwrap();
        let mut evaluator = Evaluator::new();
        evaluator.set_var("n", Expr::Integer(4));
        let value = evaluator.eval(&symbolic).unwrap();
        assert!((expr_to_f64(&value) - 2880.0).abs() < 1e-10);
    }

    #[test]
    fn test_symbolic_product_telescoping_ratio() {
        let symbolic = eval("product(k/(k-1), k, 2, n)").unwrap();
        let mut evaluator = Evaluator::new();
        evaluator.set_var("n", Expr::Integer(6));
        let value = evaluator.eval(&symbolic).unwrap();
        assert!((expr_to_f64(&value) - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_symbolic_product_fallback_for_unreduced_form() {
        let symbolic = eval("product(k^2 + 2, k, 1, n)").unwrap();
        assert!(matches!(symbolic, Expr::Product { .. }));
    }

    #[test]
    fn test_sum_infers_iteration_var_when_template_var_unused() {
        let expr = Expr::Sum {
            expr: Box::new(Expr::Symbol(crate::expr::Symbol::new("n"))),
            var: crate::expr::Symbol::new("i"),
            lower: Box::new(Expr::Integer(1)),
            upper: Box::new(Expr::Integer(5)),
        };
        let result = Evaluator::new().eval(&expr).unwrap();
        assert_eq!(result, Expr::Integer(15));
    }

    #[test]
    fn test_product_infers_iteration_var_when_template_var_unused() {
        let expr = Expr::Product {
            expr: Box::new(Expr::Symbol(crate::expr::Symbol::new("n"))),
            var: crate::expr::Symbol::new("i"),
            lower: Box::new(Expr::Integer(1)),
            upper: Box::new(Expr::Integer(5)),
        };
        let result = Evaluator::new().eval(&expr).unwrap();
        assert_eq!(result, Expr::Integer(120));
    }

    #[test]
    fn test_exact_multiply_rational_one_identity() {
        let mut evaluator = Evaluator::new();
        evaluator.exact_mode = true;

        let expr = Expr::mul(vec![
            Expr::Rational(crate::expr::Rational::new(1, 1)),
            Expr::symbol("n"),
        ]);
        let result = evaluator.eval(&expr).unwrap();

        assert_eq!(result, Expr::symbol("n"));
    }
}
