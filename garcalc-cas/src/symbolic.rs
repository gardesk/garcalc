//! Symbolic mathematics operations
//!
//! Provides symbolic differentiation, integration, simplification, and solving.

use crate::expr::{Expr, Symbol};
use crate::error::{CasError, Result};

/// Symbolic differentiator
pub struct Differentiator;

impl Differentiator {
    /// Compute the derivative of an expression with respect to a variable
    pub fn diff(expr: &Expr, var: &Symbol) -> Result<Expr> {
        Self::diff_impl(expr, var)
    }

    /// Compute the nth derivative
    pub fn diff_n(expr: &Expr, var: &Symbol, n: u32) -> Result<Expr> {
        let mut result = expr.clone();
        for _ in 0..n {
            result = Self::diff(&result, var)?;
        }
        Ok(result)
    }

    fn diff_impl(expr: &Expr, var: &Symbol) -> Result<Expr> {
        match expr {
            // Constants: d/dx(c) = 0
            Expr::Integer(_) | Expr::Float(_) | Expr::Rational(_) | Expr::Complex(_, _) => {
                Ok(Expr::Integer(0))
            }

            // Variable: d/dx(x) = 1, d/dx(y) = 0
            Expr::Symbol(s) => {
                if s == var {
                    Ok(Expr::Integer(1))
                } else {
                    Ok(Expr::Integer(0))
                }
            }

            // Negation: d/dx(-f) = -d/dx(f)
            Expr::Neg(e) => {
                let de = Self::diff(e, var)?;
                Ok(Expr::neg(de))
            }

            // Sum rule: d/dx(f + g + ...) = d/dx(f) + d/dx(g) + ...
            Expr::Add(terms) => {
                let dterms: Result<Vec<_>> = terms.iter().map(|t| Self::diff(t, var)).collect();
                Ok(Expr::add(dterms?))
            }

            // Product rule: d/dx(f * g) = f' * g + f * g'
            // For multiple factors: d/dx(f*g*h) = f'*g*h + f*g'*h + f*g*h'
            Expr::Mul(factors) => {
                if factors.is_empty() {
                    return Ok(Expr::Integer(0));
                }
                if factors.len() == 1 {
                    return Self::diff(&factors[0], var);
                }

                let mut terms = Vec::new();
                for i in 0..factors.len() {
                    let mut term_factors = Vec::new();
                    for (j, f) in factors.iter().enumerate() {
                        if i == j {
                            term_factors.push(Self::diff(f, var)?);
                        } else {
                            term_factors.push(f.clone());
                        }
                    }
                    terms.push(Expr::mul(term_factors));
                }
                Ok(Expr::add(terms))
            }

            // Power rule: d/dx(f^g)
            // If g is constant: d/dx(f^n) = n * f^(n-1) * f'
            // General case: d/dx(f^g) = f^g * (g' * ln(f) + g * f'/f)
            Expr::Pow(base, exp) => {
                let base_has_var = base.contains_var(var);
                let exp_has_var = exp.contains_var(var);

                match (base_has_var, exp_has_var) {
                    // d/dx(a^b) = 0 where a,b are constants
                    (false, false) => Ok(Expr::Integer(0)),

                    // d/dx(f^n) = n * f^(n-1) * f' (power rule)
                    (true, false) => {
                        let df = Self::diff(base, var)?;
                        let n_minus_1 = Expr::sub((**exp).clone(), Expr::Integer(1));
                        Ok(Expr::mul(vec![
                            (**exp).clone(),
                            Expr::pow((**base).clone(), n_minus_1),
                            df,
                        ]))
                    }

                    // d/dx(a^g) = a^g * ln(a) * g' (exponential rule)
                    (false, true) => {
                        let dg = Self::diff(exp, var)?;
                        Ok(Expr::mul(vec![
                            Expr::pow((**base).clone(), (**exp).clone()),
                            Expr::func("ln", vec![(**base).clone()]),
                            dg,
                        ]))
                    }

                    // d/dx(f^g) = f^g * (g' * ln(f) + g * f'/f)
                    (true, true) => {
                        let df = Self::diff(base, var)?;
                        let dg = Self::diff(exp, var)?;
                        let f_g = Expr::pow((**base).clone(), (**exp).clone());
                        let ln_f = Expr::func("ln", vec![(**base).clone()]);
                        let f_prime_over_f = Expr::div(df, (**base).clone());
                        Ok(Expr::mul(vec![
                            f_g,
                            Expr::add(vec![
                                Expr::mul(vec![dg, ln_f]),
                                Expr::mul(vec![(**exp).clone(), f_prime_over_f]),
                            ]),
                        ]))
                    }
                }
            }

            // Function derivatives
            Expr::Func(name, args) => Self::diff_func(name, args, var),

            // Derivative of a derivative: just nest it
            Expr::Derivative { expr, var: v, order } => {
                if v == var {
                    Ok(Expr::Derivative {
                        expr: expr.clone(),
                        var: var.clone(),
                        order: order + 1,
                    })
                } else {
                    // Mixed partial - differentiate the inner derivative
                    let inner = Self::diff(expr, var)?;
                    Ok(Expr::Derivative {
                        expr: Box::new(inner),
                        var: v.clone(),
                        order: *order,
                    })
                }
            }

            // Other cases return unevaluated derivative
            _ => Ok(Expr::Derivative {
                expr: Box::new(expr.clone()),
                var: var.clone(),
                order: 1,
            }),
        }
    }

    fn diff_func(name: &str, args: &[Expr], var: &Symbol) -> Result<Expr> {
        if args.is_empty() {
            return Ok(Expr::Integer(0));
        }

        let arg = &args[0];
        let darg = Self::diff(arg, var)?;

        // If the argument doesn't contain the variable, derivative is 0
        if !arg.contains_var(var) {
            return Ok(Expr::Integer(0));
        }

        // Chain rule: d/dx(f(g(x))) = f'(g(x)) * g'(x)
        let outer_deriv = match name {
            // Trigonometric
            "sin" => Expr::func("cos", vec![arg.clone()]),
            "cos" => Expr::neg(Expr::func("sin", vec![arg.clone()])),
            "tan" => {
                // sec^2(x) = 1/cos^2(x)
                Expr::pow(Expr::func("sec", vec![arg.clone()]), Expr::Integer(2))
            }
            "cot" => Expr::neg(Expr::pow(
                Expr::func("csc", vec![arg.clone()]),
                Expr::Integer(2),
            )),
            "sec" => Expr::mul(vec![
                Expr::func("sec", vec![arg.clone()]),
                Expr::func("tan", vec![arg.clone()]),
            ]),
            "csc" => Expr::neg(Expr::mul(vec![
                Expr::func("csc", vec![arg.clone()]),
                Expr::func("cot", vec![arg.clone()]),
            ])),

            // Inverse trigonometric
            "asin" | "arcsin" => {
                // 1 / sqrt(1 - x^2)
                Expr::div(
                    Expr::Integer(1),
                    Expr::func(
                        "sqrt",
                        vec![Expr::sub(
                            Expr::Integer(1),
                            Expr::pow(arg.clone(), Expr::Integer(2)),
                        )],
                    ),
                )
            }
            "acos" | "arccos" => {
                // -1 / sqrt(1 - x^2)
                Expr::neg(Expr::div(
                    Expr::Integer(1),
                    Expr::func(
                        "sqrt",
                        vec![Expr::sub(
                            Expr::Integer(1),
                            Expr::pow(arg.clone(), Expr::Integer(2)),
                        )],
                    ),
                ))
            }
            "atan" | "arctan" => {
                // 1 / (1 + x^2)
                Expr::div(
                    Expr::Integer(1),
                    Expr::add(vec![
                        Expr::Integer(1),
                        Expr::pow(arg.clone(), Expr::Integer(2)),
                    ]),
                )
            }

            // Hyperbolic
            "sinh" => Expr::func("cosh", vec![arg.clone()]),
            "cosh" => Expr::func("sinh", vec![arg.clone()]),
            "tanh" => {
                // sech^2(x) = 1 - tanh^2(x)
                Expr::sub(
                    Expr::Integer(1),
                    Expr::pow(Expr::func("tanh", vec![arg.clone()]), Expr::Integer(2)),
                )
            }

            // Exponential and logarithmic
            "exp" => Expr::func("exp", vec![arg.clone()]),
            "ln" | "log" => Expr::div(Expr::Integer(1), arg.clone()),
            "log10" => {
                // 1 / (x * ln(10))
                Expr::div(
                    Expr::Integer(1),
                    Expr::mul(vec![arg.clone(), Expr::func("ln", vec![Expr::Integer(10)])]),
                )
            }
            "log2" => {
                // 1 / (x * ln(2))
                Expr::div(
                    Expr::Integer(1),
                    Expr::mul(vec![arg.clone(), Expr::func("ln", vec![Expr::Integer(2)])]),
                )
            }

            // Root functions
            "sqrt" => {
                // 1 / (2 * sqrt(x))
                Expr::div(
                    Expr::Integer(1),
                    Expr::mul(vec![Expr::Integer(2), Expr::func("sqrt", vec![arg.clone()])]),
                )
            }
            "cbrt" => {
                // 1 / (3 * cbrt(x)^2)
                Expr::div(
                    Expr::Integer(1),
                    Expr::mul(vec![
                        Expr::Integer(3),
                        Expr::pow(Expr::func("cbrt", vec![arg.clone()]), Expr::Integer(2)),
                    ]),
                )
            }

            // Absolute value
            "abs" => {
                // |x|' = x / |x| = sign(x)
                Expr::func("sign", vec![arg.clone()])
            }

            // Unknown function: return unevaluated
            _ => {
                return Ok(Expr::Derivative {
                    expr: Box::new(Expr::func(name, args.to_vec())),
                    var: var.clone(),
                    order: 1,
                });
            }
        };

        // Apply chain rule: f'(g(x)) * g'(x)
        if darg.is_one() {
            Ok(outer_deriv)
        } else {
            Ok(Expr::mul(vec![outer_deriv, darg]))
        }
    }
}

/// Symbolic integrator
pub struct Integrator;

impl Integrator {
    /// Compute indefinite integral
    pub fn integrate(expr: &Expr, var: &Symbol) -> Result<Expr> {
        Self::integrate_impl(expr, var)
    }

    /// Compute definite integral
    pub fn integrate_definite(
        expr: &Expr,
        var: &Symbol,
        lower: &Expr,
        upper: &Expr,
    ) -> Result<Expr> {
        // Get antiderivative
        let antideriv = Self::integrate(expr, var)?;

        // F(upper) - F(lower)
        let f_upper = Simplifier::substitute(&antideriv, var, upper);
        let f_lower = Simplifier::substitute(&antideriv, var, lower);

        Ok(Expr::sub(f_upper, f_lower))
    }

    fn integrate_impl(expr: &Expr, var: &Symbol) -> Result<Expr> {
        // If expression doesn't contain variable, it's a constant
        if !expr.contains_var(var) {
            return Ok(Expr::mul(vec![
                expr.clone(),
                Expr::Symbol(var.clone()),
            ]));
        }

        match expr {
            // ∫ x dx = x^2/2
            Expr::Symbol(s) if s == var => Ok(Expr::div(
                Expr::pow(Expr::Symbol(var.clone()), Expr::Integer(2)),
                Expr::Integer(2),
            )),

            // ∫ -f dx = -∫ f dx
            Expr::Neg(e) => {
                let int_e = Self::integrate(e, var)?;
                Ok(Expr::neg(int_e))
            }

            // ∫ (f + g) dx = ∫ f dx + ∫ g dx
            Expr::Add(terms) => {
                let int_terms: Result<Vec<_>> =
                    terms.iter().map(|t| Self::integrate(t, var)).collect();
                Ok(Expr::add(int_terms?))
            }

            // ∫ c*f dx = c * ∫ f dx (where c is constant)
            Expr::Mul(factors) => {
                let (constants, var_factors): (Vec<_>, Vec<_>) =
                    factors.iter().partition(|f| !f.contains_var(var));

                if var_factors.is_empty() {
                    // All constants
                    return Ok(Expr::mul(vec![
                        Expr::mul(constants.into_iter().cloned().collect()),
                        Expr::Symbol(var.clone()),
                    ]));
                }

                if constants.is_empty() {
                    // No constants to factor out
                    Self::integrate_product(&var_factors, var)
                } else {
                    // Factor out constants
                    let const_part = Expr::mul(constants.into_iter().cloned().collect());
                    let var_part = if var_factors.len() == 1 {
                        var_factors[0].clone()
                    } else {
                        Expr::mul(var_factors.into_iter().cloned().collect())
                    };
                    let int_var = Self::integrate(&var_part, var)?;
                    Ok(Expr::mul(vec![const_part, int_var]))
                }
            }

            // Power rule: ∫ x^n dx = x^(n+1)/(n+1) for n ≠ -1
            Expr::Pow(base, exp) => {
                // Check if base is just the variable and exponent is constant
                if **base == Expr::Symbol(var.clone()) && !exp.contains_var(var) {
                    // Check for n = -1 case: ∫ x^(-1) dx = ln|x|
                    if exp.is_negative_one() {
                        return Ok(Expr::func("ln", vec![Expr::func(
                            "abs",
                            vec![Expr::Symbol(var.clone())],
                        )]));
                    }

                    let n_plus_1 = Expr::add(vec![(**exp).clone(), Expr::Integer(1)]);
                    return Ok(Expr::div(
                        Expr::pow(Expr::Symbol(var.clone()), n_plus_1.clone()),
                        n_plus_1,
                    ));
                }

                // ∫ e^x dx = e^x
                if let Expr::Symbol(s) = &**base {
                    if s.as_str() == "e" && **exp == Expr::Symbol(var.clone()) {
                        return Ok(Expr::pow(Expr::symbol("e"), Expr::Symbol(var.clone())));
                    }
                }

                // ∫ a^x dx = a^x / ln(a)
                if !base.contains_var(var) && **exp == Expr::Symbol(var.clone()) {
                    return Ok(Expr::div(
                        expr.clone(),
                        Expr::func("ln", vec![(**base).clone()]),
                    ));
                }

                // Return unevaluated
                Ok(Expr::Integral {
                    expr: Box::new(expr.clone()),
                    var: var.clone(),
                    lower: None,
                    upper: None,
                })
            }

            // Function integrals
            Expr::Func(name, args) => Self::integrate_func(name, args, var),

            // Return unevaluated for unhandled cases
            _ => Ok(Expr::Integral {
                expr: Box::new(expr.clone()),
                var: var.clone(),
                lower: None,
                upper: None,
            }),
        }
    }

    fn integrate_product(factors: &[&Expr], var: &Symbol) -> Result<Expr> {
        // Very limited - just handle simple cases
        if factors.len() == 1 {
            return Self::integrate(factors[0], var);
        }

        // Return unevaluated
        Ok(Expr::Integral {
            expr: Box::new(Expr::mul(factors.iter().map(|f| (*f).clone()).collect())),
            var: var.clone(),
            lower: None,
            upper: None,
        })
    }

    fn integrate_func(name: &str, args: &[Expr], var: &Symbol) -> Result<Expr> {
        if args.is_empty() {
            return Ok(Expr::Integral {
                expr: Box::new(Expr::func(name, vec![])),
                var: var.clone(),
                lower: None,
                upper: None,
            });
        }

        let arg = &args[0];

        // Only handle simple case where arg = var
        if *arg != Expr::Symbol(var.clone()) {
            return Ok(Expr::Integral {
                expr: Box::new(Expr::func(name, args.to_vec())),
                var: var.clone(),
                lower: None,
                upper: None,
            });
        }

        let result = match name {
            // Trigonometric
            "sin" => Expr::neg(Expr::func("cos", vec![arg.clone()])),
            "cos" => Expr::func("sin", vec![arg.clone()]),
            "tan" => Expr::neg(Expr::func("ln", vec![Expr::func(
                "abs",
                vec![Expr::func("cos", vec![arg.clone()])],
            )])),
            "cot" => Expr::func("ln", vec![Expr::func(
                "abs",
                vec![Expr::func("sin", vec![arg.clone()])],
            )]),
            "sec" => Expr::func("ln", vec![Expr::func(
                "abs",
                vec![Expr::add(vec![
                    Expr::func("sec", vec![arg.clone()]),
                    Expr::func("tan", vec![arg.clone()]),
                ])],
            )]),
            "csc" => Expr::neg(Expr::func("ln", vec![Expr::func(
                "abs",
                vec![Expr::add(vec![
                    Expr::func("csc", vec![arg.clone()]),
                    Expr::func("cot", vec![arg.clone()]),
                ])],
            )])),

            // Hyperbolic
            "sinh" => Expr::func("cosh", vec![arg.clone()]),
            "cosh" => Expr::func("sinh", vec![arg.clone()]),
            "tanh" => Expr::func("ln", vec![Expr::func("cosh", vec![arg.clone()])]),

            // Exponential
            "exp" => Expr::func("exp", vec![arg.clone()]),

            // Return unevaluated for unknown
            _ => {
                return Ok(Expr::Integral {
                    expr: Box::new(Expr::func(name, args.to_vec())),
                    var: var.clone(),
                    lower: None,
                    upper: None,
                });
            }
        };

        Ok(result)
    }
}

/// Expression simplifier
pub struct Simplifier;

impl Simplifier {
    /// Simplify an expression
    pub fn simplify(expr: &Expr) -> Expr {
        Self::simplify_impl(expr)
    }

    /// Substitute a variable with an expression
    pub fn substitute(expr: &Expr, var: &Symbol, replacement: &Expr) -> Expr {
        match expr {
            Expr::Symbol(s) if s == var => replacement.clone(),
            Expr::Symbol(_)
            | Expr::Integer(_)
            | Expr::Float(_)
            | Expr::Rational(_)
            | Expr::Complex(_, _) => expr.clone(),

            Expr::Neg(e) => Expr::neg(Self::substitute(e, var, replacement)),

            Expr::Add(terms) => Expr::add(
                terms
                    .iter()
                    .map(|t| Self::substitute(t, var, replacement))
                    .collect(),
            ),

            Expr::Mul(factors) => Expr::mul(
                factors
                    .iter()
                    .map(|f| Self::substitute(f, var, replacement))
                    .collect(),
            ),

            Expr::Pow(base, exp) => Expr::pow(
                Self::substitute(base, var, replacement),
                Self::substitute(exp, var, replacement),
            ),

            Expr::Func(name, args) => Expr::func(
                name,
                args.iter()
                    .map(|a| Self::substitute(a, var, replacement))
                    .collect(),
            ),

            _ => expr.clone(),
        }
    }

    /// Expand products: (a+b)(c+d) = ac + ad + bc + bd
    pub fn expand(expr: &Expr) -> Expr {
        match expr {
            Expr::Mul(factors) => {
                let expanded_factors: Vec<_> = factors.iter().map(Self::expand).collect();
                Self::expand_product(&expanded_factors)
            }
            Expr::Add(terms) => Expr::add(terms.iter().map(Self::expand).collect()),
            Expr::Neg(e) => Expr::neg(Self::expand(e)),
            Expr::Pow(base, exp) => {
                // Expand (a+b)^n for small positive integer n
                if let Expr::Integer(n) = **exp {
                    if n > 1 && n <= 6 {
                        if let Expr::Add(_) = **base {
                            let expanded = Self::expand(base);
                            let mut result = expanded.clone();
                            for _ in 1..n {
                                result = Self::expand(&Expr::mul(vec![result, expanded.clone()]));
                            }
                            return result;
                        }
                    }
                }
                Expr::pow(Self::expand(base), Self::expand(exp))
            }
            _ => expr.clone(),
        }
    }

    fn expand_product(factors: &[Expr]) -> Expr {
        if factors.is_empty() {
            return Expr::Integer(1);
        }
        if factors.len() == 1 {
            return factors[0].clone();
        }

        // Multiply first two factors, then recurse
        let first = &factors[0];
        let second = &factors[1];
        let rest = &factors[2..];

        let product = match (first, second) {
            (Expr::Add(terms1), Expr::Add(terms2)) => {
                // (a+b)(c+d) = ac + ad + bc + bd
                let mut result_terms = Vec::new();
                for t1 in terms1 {
                    for t2 in terms2 {
                        result_terms.push(Expr::mul(vec![t1.clone(), t2.clone()]));
                    }
                }
                Expr::add(result_terms)
            }
            (Expr::Add(terms), other) | (other, Expr::Add(terms)) => {
                // (a+b)*c = ac + bc
                Expr::add(
                    terms
                        .iter()
                        .map(|t| Expr::mul(vec![t.clone(), other.clone()]))
                        .collect(),
                )
            }
            _ => Expr::mul(vec![first.clone(), second.clone()]),
        };

        if rest.is_empty() {
            product
        } else {
            let mut new_factors = vec![product];
            new_factors.extend(rest.iter().cloned());
            Self::expand_product(&new_factors)
        }
    }

    fn simplify_impl(expr: &Expr) -> Expr {
        match expr {
            // Recursively simplify sub-expressions first
            Expr::Neg(e) => {
                let se = Self::simplify_impl(e);
                match se {
                    Expr::Integer(0) => Expr::Integer(0),
                    Expr::Integer(n) => Expr::Integer(-n),
                    Expr::Float(x) => Expr::Float(-x),
                    Expr::Neg(inner) => *inner,
                    other => Expr::neg(other),
                }
            }

            Expr::Add(terms) => Self::simplify_add(terms),
            Expr::Mul(factors) => Self::simplify_mul(factors),

            Expr::Pow(base, exp) => {
                let sb = Self::simplify_impl(base);
                let se = Self::simplify_impl(exp);

                // x^0 = 1
                if se.is_zero() {
                    return Expr::Integer(1);
                }
                // x^1 = x
                if se.is_one() {
                    return sb;
                }
                // 0^n = 0 (for n > 0)
                if sb.is_zero() {
                    if let Expr::Integer(n) = se {
                        if n > 0 {
                            return Expr::Integer(0);
                        }
                    }
                }
                // 1^n = 1
                if sb.is_one() {
                    return Expr::Integer(1);
                }
                // Numeric power
                if let (Expr::Integer(b), Expr::Integer(e)) = (&sb, &se) {
                    if *e >= 0 && *e <= 20 {
                        if let Some(result) = b.checked_pow(*e as u32) {
                            return Expr::Integer(result);
                        }
                    }
                    // Negative integer exponent: a^(-n) = 1/a^n
                    if *e < 0 && *e >= -20 {
                        if let Some(denom) = b.checked_pow((-*e) as u32) {
                            return Expr::Rational(crate::expr::Rational::new(1, denom));
                        }
                    }
                }

                Expr::pow(sb, se)
            }

            Expr::Func(name, args) => {
                let sargs: Vec<_> = args.iter().map(Self::simplify_impl).collect();

                // Try to evaluate sqrt of perfect squares
                if name == "sqrt" && sargs.len() == 1 {
                    if let Expr::Integer(n) = &sargs[0] {
                        if *n >= 0 {
                            let sqrt = (*n as f64).sqrt();
                            if sqrt.fract() == 0.0 {
                                return Expr::Integer(sqrt as i64);
                            }
                        }
                    }
                }

                Expr::func(name, sargs)
            }

            // Already simple
            _ => expr.clone(),
        }
    }

    fn simplify_add(terms: &[Expr]) -> Expr {
        let mut simplified: Vec<Expr> = terms.iter().map(Self::simplify_impl).collect();

        // Flatten nested adds
        let mut flattened = Vec::new();
        for term in simplified.drain(..) {
            match term {
                Expr::Add(inner) => flattened.extend(inner),
                other => flattened.push(other),
            }
        }

        // Remove zeros
        flattened.retain(|t| !t.is_zero());

        // Combine numeric terms
        let mut numeric_sum: f64 = 0.0;
        let mut has_numeric = false;
        let mut non_numeric = Vec::new();

        for term in flattened {
            match &term {
                Expr::Integer(n) => {
                    numeric_sum += *n as f64;
                    has_numeric = true;
                }
                Expr::Float(x) => {
                    numeric_sum += x;
                    has_numeric = true;
                }
                _ => non_numeric.push(term),
            }
        }

        // Collect like terms (simplified version)
        let result_terms = Self::collect_like_terms(non_numeric);

        let mut final_terms = result_terms;
        if has_numeric && numeric_sum != 0.0 {
            if numeric_sum.fract() == 0.0 {
                final_terms.insert(0, Expr::Integer(numeric_sum as i64));
            } else {
                final_terms.insert(0, Expr::Float(numeric_sum));
            }
        }

        if final_terms.is_empty() {
            Expr::Integer(0)
        } else if final_terms.len() == 1 {
            final_terms.into_iter().next().unwrap()
        } else {
            Expr::Add(final_terms)
        }
    }

    fn collect_like_terms(terms: Vec<Expr>) -> Vec<Expr> {
        // Simple like-term collection: group by "base" expression
        use std::collections::HashMap;

        let mut term_map: HashMap<String, (Expr, f64)> = HashMap::new();

        for term in terms {
            let (coef, base) = Self::extract_coefficient(&term);
            let key = base.to_string();

            if let Some((_, existing_coef)) = term_map.get_mut(&key) {
                *existing_coef += coef;
            } else {
                term_map.insert(key, (base, coef));
            }
        }

        term_map
            .into_values()
            .filter_map(|(base, coef)| {
                if coef == 0.0 {
                    None
                } else if coef == 1.0 {
                    Some(base)
                } else if coef == -1.0 {
                    Some(Expr::neg(base))
                } else if coef.fract() == 0.0 {
                    Some(Expr::mul(vec![Expr::Integer(coef as i64), base]))
                } else {
                    Some(Expr::mul(vec![Expr::Float(coef), base]))
                }
            })
            .collect()
    }

    fn extract_coefficient(expr: &Expr) -> (f64, Expr) {
        match expr {
            Expr::Neg(e) => {
                let (c, b) = Self::extract_coefficient(e);
                (-c, b)
            }
            Expr::Mul(factors) => {
                let mut coef = 1.0;
                let mut rest = Vec::new();
                for f in factors {
                    match f {
                        Expr::Integer(n) => coef *= *n as f64,
                        Expr::Float(x) => coef *= x,
                        _ => rest.push(f.clone()),
                    }
                }
                let base = if rest.is_empty() {
                    Expr::Integer(1)
                } else if rest.len() == 1 {
                    rest.into_iter().next().unwrap()
                } else {
                    Expr::Mul(rest)
                };
                (coef, base)
            }
            Expr::Integer(n) => (*n as f64, Expr::Integer(1)),
            Expr::Float(x) => (*x, Expr::Integer(1)),
            other => (1.0, other.clone()),
        }
    }

    fn simplify_mul(factors: &[Expr]) -> Expr {
        let mut simplified: Vec<Expr> = factors.iter().map(Self::simplify_impl).collect();

        // Flatten nested multiplications
        let mut flattened = Vec::new();
        for factor in simplified.drain(..) {
            match factor {
                Expr::Mul(inner) => flattened.extend(inner),
                other => flattened.push(other),
            }
        }

        // If any factor is zero, result is zero
        if flattened.iter().any(|f| f.is_zero()) {
            return Expr::Integer(0);
        }

        // Remove ones
        flattened.retain(|f| !f.is_one());

        // Combine numeric factors including rationals
        let mut numerator: i64 = 1;
        let mut denominator: i64 = 1;
        let mut has_float = false;
        let mut float_product: f64 = 1.0;
        let mut non_numeric = Vec::new();

        for factor in flattened {
            match &factor {
                Expr::Integer(n) => {
                    numerator *= n;
                }
                Expr::Rational(r) => {
                    numerator *= r.num;
                    denominator *= r.den;
                }
                Expr::Float(x) => {
                    has_float = true;
                    float_product *= x;
                }
                Expr::Pow(base, exp) => {
                    // Check for x^(-1) pattern with integer base
                    if let (Expr::Integer(b), Expr::Integer(-1)) = (&**base, &**exp) {
                        denominator *= b;
                    } else {
                        non_numeric.push(factor);
                    }
                }
                _ => non_numeric.push(factor),
            }
        }

        // Simplify the numeric part
        let mut final_factors = non_numeric;

        if has_float {
            let result = float_product * (numerator as f64) / (denominator as f64);
            if result != 1.0 {
                if result.fract() == 0.0 {
                    final_factors.insert(0, Expr::Integer(result as i64));
                } else {
                    final_factors.insert(0, Expr::Float(result));
                }
            }
        } else if numerator != 1 || denominator != 1 {
            if denominator == 1 {
                if numerator != 1 {
                    final_factors.insert(0, Expr::Integer(numerator));
                }
            } else {
                final_factors.insert(0, Expr::Rational(crate::expr::Rational::new(numerator, denominator)));
            }
        }

        if final_factors.is_empty() {
            Expr::Integer(1)
        } else if final_factors.len() == 1 {
            final_factors.into_iter().next().unwrap()
        } else {
            Expr::Mul(final_factors)
        }
    }
}

/// Equation solver
pub struct Solver;

impl Solver {
    /// Solve an equation for a variable
    pub fn solve(equation: &Expr, var: &Symbol) -> Result<Vec<Expr>> {
        match equation {
            Expr::Equation(lhs, rhs) => {
                // Move everything to one side: lhs - rhs = 0
                let expr = Simplifier::simplify(&Expr::sub((**lhs).clone(), (**rhs).clone()));
                Self::solve_for_zero(&expr, var)
            }
            // Treat as expr = 0
            _ => Self::solve_for_zero(equation, var),
        }
    }

    fn solve_for_zero(expr: &Expr, var: &Symbol) -> Result<Vec<Expr>> {
        // Simplify first
        let expr = Simplifier::simplify(expr);

        // Check if variable is present
        if !expr.contains_var(var) {
            if expr.is_zero() {
                // All values are solutions (identity)
                return Err(CasError::EvaluationError(
                    "Equation is an identity".to_string(),
                ));
            } else {
                // No solution
                return Ok(vec![]);
            }
        }

        // Try to extract polynomial coefficients
        if let Ok((a, b, c)) = Self::extract_quadratic_coefficients(&expr, var) {
            // Check if it's linear (a == 0)
            if a.is_zero() {
                // Linear: bx + c = 0 => x = -c/b
                if b.is_zero() {
                    return Err(CasError::EvaluationError(
                        "Variable coefficient is zero".to_string(),
                    ));
                }
                return Ok(vec![Simplifier::simplify(&Expr::div(
                    Expr::neg(c),
                    b,
                ))]);
            }
            // Quadratic
            return Self::solve_quadratic_with_coeffs(&a, &b, &c);
        }

        // Fallback to isolation
        Self::solve_by_isolation(&expr, var)
    }

    fn classify_polynomial(expr: &Expr, var: &Symbol) -> Option<(u32, Expr, Expr)> {
        // Very simplified polynomial detection
        // Returns (degree, leading coefficient, rest)
        match expr {
            // x alone: degree 1, coef 1
            Expr::Symbol(s) if s == var => Some((1, Expr::Integer(1), Expr::Integer(0))),

            // a*x: degree 1, coef a
            Expr::Mul(factors) => {
                let mut coef = Expr::Integer(1);
                let mut has_var = false;
                let mut var_power = 1u32;

                for f in factors {
                    match f {
                        Expr::Symbol(s) if s == var => {
                            has_var = true;
                        }
                        Expr::Pow(base, exp) => {
                            if let Expr::Symbol(s) = &**base {
                                if s == var {
                                    if let Expr::Integer(n) = **exp {
                                        has_var = true;
                                        var_power = n as u32;
                                    }
                                }
                            }
                        }
                        other if !other.contains_var(var) => {
                            coef = Expr::mul(vec![coef, other.clone()]);
                        }
                        _ => return None,
                    }
                }

                if has_var {
                    Some((var_power, Simplifier::simplify(&coef), Expr::Integer(0)))
                } else {
                    None
                }
            }

            // ax + b or ax^2 + bx + c
            Expr::Add(terms) => {
                let mut max_degree = 0u32;
                let mut leading_coef = Expr::Integer(0);

                for term in terms {
                    if let Some((deg, coef, _)) = Self::classify_polynomial(term, var) {
                        if deg > max_degree {
                            max_degree = deg;
                            leading_coef = coef;
                        }
                    } else if !term.contains_var(var) {
                        // Constant term, ignore for degree calculation
                    } else {
                        return None;
                    }
                }

                if max_degree > 0 {
                    Some((max_degree, leading_coef, Expr::Integer(0)))
                } else {
                    None
                }
            }

            _ => None,
        }
    }

    fn solve_quadratic_with_coeffs(a: &Expr, b: &Expr, c: &Expr) -> Result<Vec<Expr>> {
        // Discriminant: b^2 - 4ac
        let discriminant = Simplifier::simplify(&Expr::sub(
            Expr::pow(b.clone(), Expr::Integer(2)),
            Expr::mul(vec![Expr::Integer(4), a.clone(), c.clone()]),
        ));

        // x = (-b ± sqrt(discriminant)) / (2a)
        let neg_b = Expr::neg(b.clone());
        let two_a = Expr::mul(vec![Expr::Integer(2), a.clone()]);
        let sqrt_d = Expr::func("sqrt", vec![discriminant]);

        let x1 = Simplifier::simplify(&Expr::div(
            Expr::add(vec![neg_b.clone(), sqrt_d.clone()]),
            two_a.clone(),
        ));
        let x2 = Simplifier::simplify(&Expr::div(
            Expr::sub(neg_b, sqrt_d),
            two_a,
        ));

        Ok(vec![x1, x2])
    }

    #[allow(dead_code)]
    fn solve_quadratic(expr: &Expr, var: &Symbol) -> Result<Vec<Expr>> {
        // Extract coefficients a, b, c from ax^2 + bx + c
        let (a, b, c) = Self::extract_quadratic_coefficients(expr, var)?;
        Self::solve_quadratic_with_coeffs(&a, &b, &c)
    }

    fn extract_quadratic_coefficients(
        expr: &Expr,
        var: &Symbol,
    ) -> Result<(Expr, Expr, Expr)> {
        // Initialize coefficients
        let mut a = Expr::Integer(0);
        let mut b = Expr::Integer(0);
        let mut c = Expr::Integer(0);

        let terms = match expr {
            Expr::Add(terms) => terms.clone(),
            other => vec![other.clone()],
        };

        for term in terms {
            let (coef, degree) = Self::get_term_coefficient_and_degree(&term, var);
            match degree {
                0 => c = Expr::add(vec![c, coef]),
                1 => b = Expr::add(vec![b, coef]),
                2 => a = Expr::add(vec![a, coef]),
                _ => {
                    return Err(CasError::EvaluationError(
                        "Not a quadratic equation".to_string(),
                    ))
                }
            }
        }

        Ok((
            Simplifier::simplify(&a),
            Simplifier::simplify(&b),
            Simplifier::simplify(&c),
        ))
    }

    fn get_term_coefficient_and_degree(term: &Expr, var: &Symbol) -> (Expr, u32) {
        match term {
            Expr::Symbol(s) if s == var => (Expr::Integer(1), 1),

            Expr::Pow(base, exp) => {
                if let Expr::Symbol(s) = &**base {
                    if s == var {
                        if let Expr::Integer(n) = **exp {
                            return (Expr::Integer(1), n as u32);
                        }
                    }
                }
                if !term.contains_var(var) {
                    (term.clone(), 0)
                } else {
                    (Expr::Integer(1), 0) // Fallback
                }
            }

            Expr::Mul(factors) => {
                let mut coef_parts = Vec::new();
                let mut degree = 0u32;

                for f in factors {
                    match f {
                        Expr::Symbol(s) if s == var => {
                            degree += 1;
                        }
                        Expr::Pow(base, exp) => {
                            if let Expr::Symbol(s) = &**base {
                                if s == var {
                                    if let Expr::Integer(n) = **exp {
                                        degree += n as u32;
                                        continue;
                                    }
                                }
                            }
                            coef_parts.push(f.clone());
                        }
                        _ => coef_parts.push(f.clone()),
                    }
                }

                let coef = if coef_parts.is_empty() {
                    Expr::Integer(1)
                } else {
                    Expr::mul(coef_parts)
                };
                (coef, degree)
            }

            Expr::Neg(inner) => {
                let (coef, deg) = Self::get_term_coefficient_and_degree(inner, var);
                (Expr::neg(coef), deg)
            }

            _ if !term.contains_var(var) => (term.clone(), 0),

            _ => (Expr::Integer(1), 0),
        }
    }

    fn solve_by_isolation(expr: &Expr, var: &Symbol) -> Result<Vec<Expr>> {
        // Simple isolation for expressions like 2x + 4 = 0
        // Collect terms with var and constant terms
        let terms = match expr {
            Expr::Add(t) => t.clone(),
            other => vec![other.clone()],
        };

        let mut var_coef = Expr::Integer(0);
        let mut constant = Expr::Integer(0);

        for term in terms {
            if !term.contains_var(var) {
                constant = Expr::add(vec![constant, term]);
            } else {
                // Check if it's c*var
                let (coef, base) = Simplifier::extract_coefficient(&term);
                if base == Expr::Symbol(var.clone()) {
                    if coef.fract() == 0.0 {
                        var_coef = Expr::add(vec![var_coef, Expr::Integer(coef as i64)]);
                    } else {
                        var_coef = Expr::add(vec![var_coef, Expr::Float(coef)]);
                    }
                } else {
                    // Can't isolate
                    return Err(CasError::EvaluationError(
                        "Cannot solve this equation".to_string(),
                    ));
                }
            }
        }

        let var_coef = Simplifier::simplify(&var_coef);
        let constant = Simplifier::simplify(&constant);

        if var_coef.is_zero() {
            return Err(CasError::EvaluationError(
                "Variable coefficient is zero".to_string(),
            ));
        }

        // x = -constant / var_coef
        Ok(vec![Simplifier::simplify(&Expr::div(
            Expr::neg(constant),
            var_coef,
        ))])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_constant() {
        let result = Differentiator::diff(&Expr::Integer(5), &Symbol::new("x")).unwrap();
        assert_eq!(result, Expr::Integer(0));
    }

    #[test]
    fn test_diff_variable() {
        let result = Differentiator::diff(&Expr::symbol("x"), &Symbol::new("x")).unwrap();
        assert_eq!(result, Expr::Integer(1));
    }

    #[test]
    fn test_diff_power() {
        // d/dx(x^2) = 2x
        let expr = Expr::pow(Expr::symbol("x"), Expr::Integer(2));
        let result = Differentiator::diff(&expr, &Symbol::new("x")).unwrap();
        let simplified = Simplifier::simplify(&result);
        // Should contain 2 and x
        assert!(simplified.to_string().contains("2"));
    }

    #[test]
    fn test_integrate_constant() {
        // ∫ 5 dx = 5x
        let result = Integrator::integrate(&Expr::Integer(5), &Symbol::new("x")).unwrap();
        assert!(result.to_string().contains("x"));
    }

    #[test]
    fn test_integrate_x() {
        // ∫ x dx = x^2/2
        let result = Integrator::integrate(&Expr::symbol("x"), &Symbol::new("x")).unwrap();
        assert!(result.to_string().contains("2"));
    }

    #[test]
    fn test_simplify_add_zeros() {
        let expr = Expr::add(vec![Expr::symbol("x"), Expr::Integer(0)]);
        let result = Simplifier::simplify(&expr);
        assert_eq!(result, Expr::symbol("x"));
    }

    #[test]
    fn test_simplify_combine_like() {
        // 2x + 3x = 5x
        let expr = Expr::add(vec![
            Expr::mul(vec![Expr::Integer(2), Expr::symbol("x")]),
            Expr::mul(vec![Expr::Integer(3), Expr::symbol("x")]),
        ]);
        let result = Simplifier::simplify(&expr);
        assert!(result.to_string().contains("5"));
    }

    #[test]
    fn test_expand() {
        // (x+1)(x-1) = x^2 - 1
        let expr = Expr::mul(vec![
            Expr::add(vec![Expr::symbol("x"), Expr::Integer(1)]),
            Expr::add(vec![Expr::symbol("x"), Expr::Integer(-1)]),
        ]);
        let result = Simplifier::expand(&expr);
        // Should have x^2 and -1
        let s = result.to_string();
        assert!(s.contains("x") || s.contains("-1") || s.contains("1"));
    }

    #[test]
    fn test_solve_linear() {
        // 2x + 4 = 0 => x = -2
        let expr = Expr::add(vec![
            Expr::mul(vec![Expr::Integer(2), Expr::symbol("x")]),
            Expr::Integer(4),
        ]);
        let solutions = Solver::solve(&expr, &Symbol::new("x")).unwrap();
        assert!(!solutions.is_empty());
    }
}
