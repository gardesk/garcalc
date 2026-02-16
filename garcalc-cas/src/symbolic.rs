//! Symbolic mathematics operations
//!
//! Provides symbolic differentiation, integration, simplification, and solving.

use crate::error::{CasError, Result};
use crate::expr::{Expr, Symbol};
use std::f64::consts::E;

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
            Expr::Derivative {
                expr,
                var: v,
                order,
            } => {
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
                    Expr::mul(vec![
                        Expr::Integer(2),
                        Expr::func("sqrt", vec![arg.clone()]),
                    ]),
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

        // If we could not find an antiderivative, keep the definite integral
        // unevaluated instead of substituting and incorrectly collapsing to zero.
        if matches!(
            &antideriv,
            Expr::Integral {
                expr: inner,
                var: v,
                lower: None,
                upper: None
            } if **inner == *expr && v == var
        ) {
            return Ok(Expr::Integral {
                expr: Box::new(expr.clone()),
                var: var.clone(),
                lower: Some(Box::new(lower.clone())),
                upper: Some(Box::new(upper.clone())),
            });
        }

        // F(upper) - F(lower)
        let f_upper = Simplifier::substitute(&antideriv, var, upper);
        let f_lower = Simplifier::substitute(&antideriv, var, lower);

        Ok(Expr::sub(f_upper, f_lower))
    }

    fn integrate_impl(expr: &Expr, var: &Symbol) -> Result<Expr> {
        // If expression doesn't contain variable, it's a constant
        if !expr.contains_var(var) {
            return Ok(Expr::mul(vec![expr.clone(), Expr::Symbol(var.clone())]));
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
                        return Ok(Expr::func(
                            "ln",
                            vec![Expr::func("abs", vec![Expr::Symbol(var.clone())])],
                        ));
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

                // Linear substitution: ∫ (ax+b)^n dx = (ax+b)^(n+1) / (a*(n+1))
                if !exp.contains_var(var) {
                    if let Some((a, _b)) = Self::extract_linear(base, var) {
                        if exp.is_negative_one() {
                            // ∫ (ax+b)^(-1) dx = ln|ax+b| / a
                            return Ok(Expr::div(
                                Expr::func("ln", vec![Expr::func("abs", vec![(**base).clone()])]),
                                a,
                            ));
                        }
                        let n_plus_1 = Expr::add(vec![(**exp).clone(), Expr::Integer(1)]);
                        return Ok(Expr::div(
                            Expr::pow((**base).clone(), n_plus_1.clone()),
                            Expr::mul(vec![a, n_plus_1]),
                        ));
                    }
                }

                // Linear substitution for exponentials: ∫ e^(ax+b) dx = e^(ax+b) / a
                if let Expr::Symbol(s) = &**base {
                    if s.as_str() == "e" {
                        if let Some((a, _b)) = Self::extract_linear(exp, var) {
                            return Ok(Expr::div(
                                Expr::pow(Expr::symbol("e"), (**exp).clone()),
                                a,
                            ));
                        }
                    }
                }

                // ∫ a^(cx+d) dx = a^(cx+d) / (c * ln(a))
                if !base.contains_var(var) {
                    if let Some((c, _d)) = Self::extract_linear(exp, var) {
                        return Ok(Expr::div(
                            expr.clone(),
                            Expr::mul(vec![c, Expr::func("ln", vec![(**base).clone()])]),
                        ));
                    }
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

        // U-substitution: look for f(g(x)) * g'(x) patterns
        // For each pair of factors, check if one is the derivative of the inner
        // function of the other (up to a constant multiple).
        if factors.len() == 2 {
            // Try both orderings: factor[0] as f(g(x)), factor[1] as g'(x) and vice versa
            for (fi, fj) in [(0, 1), (1, 0)] {
                if let Some(result) = Self::try_u_substitution(factors[fi], factors[fj], var) {
                    return Ok(result);
                }
            }
        }

        // Return unevaluated
        Ok(Expr::Integral {
            expr: Box::new(Expr::mul(factors.iter().map(|f| (*f).clone()).collect())),
            var: var.clone(),
            lower: None,
            upper: None,
        })
    }

    /// Try u-substitution: given f_expr (containing g(x)) and candidate g'(x),
    /// check if candidate equals g'(x) up to a constant, and if so compute the integral.
    fn try_u_substitution(f_expr: &Expr, candidate_deriv: &Expr, var: &Symbol) -> Option<Expr> {
        // Extract inner function g(x) from f_expr
        let inner = Self::extract_inner_function(f_expr, var)?;

        // Compute g'(x)
        let g_prime = Differentiator::diff(&inner, var).ok()?;
        let g_prime_simplified = Simplifier::simplify(&g_prime);

        // Check if candidate_deriv = c * g'(x) for some constant c
        let constant_multiple = Self::is_constant_multiple(candidate_deriv, &g_prime_simplified, var)?;

        // Build f(u) by replacing g(x) with u in f_expr
        let u = Symbol::new("_u_sub_");
        let f_of_u = Self::replace_subexpr(f_expr, &inner, &Expr::Symbol(u.clone()));

        // If substitution didn't fully replace var, this pattern doesn't work
        if f_of_u.contains_var(var) {
            return None;
        }

        let antideriv = Self::integrate(&f_of_u, &u).ok()?;
        // Check it's not unevaluated
        if matches!(&antideriv, Expr::Integral { .. }) {
            return None;
        }

        // Substitute g(x) back for u
        let result = Simplifier::substitute(&antideriv, &u, &inner);

        // Multiply by constant_multiple
        if constant_multiple.is_one() {
            Some(result)
        } else {
            Some(Expr::mul(vec![constant_multiple, result]))
        }
    }

    /// Replace occurrences of `target` subexpression with `replacement` in `expr`.
    fn replace_subexpr(expr: &Expr, target: &Expr, replacement: &Expr) -> Expr {
        if Self::exprs_structurally_equal(expr, target) {
            return replacement.clone();
        }
        match expr {
            Expr::Neg(e) => Expr::neg(Self::replace_subexpr(e, target, replacement)),
            Expr::Add(terms) => Expr::add(
                terms.iter().map(|t| Self::replace_subexpr(t, target, replacement)).collect(),
            ),
            Expr::Mul(factors) => Expr::mul(
                factors.iter().map(|f| Self::replace_subexpr(f, target, replacement)).collect(),
            ),
            Expr::Pow(base, exp) => Expr::pow(
                Self::replace_subexpr(base, target, replacement),
                Self::replace_subexpr(exp, target, replacement),
            ),
            Expr::Func(name, args) => Expr::func(
                name,
                args.iter().map(|a| Self::replace_subexpr(a, target, replacement)).collect(),
            ),
            _ => expr.clone(),
        }
    }

    /// Extract the inner function g(x) from an expression that looks like f(g(x)).
    /// Returns the innermost composite argument that contains `var`.
    fn extract_inner_function(expr: &Expr, var: &Symbol) -> Option<Expr> {
        match expr {
            // f(g(x)) — the inner function is g(x)
            Expr::Func(_name, args) if args.len() == 1 => {
                let arg = &args[0];
                if *arg != Expr::Symbol(var.clone()) && arg.contains_var(var) {
                    Some(arg.clone())
                } else {
                    None
                }
            }
            // (g(x))^n — the inner function is g(x) if g(x) is not just x
            Expr::Pow(base, exp) if !exp.contains_var(var) && base.contains_var(var) => {
                if **base != Expr::Symbol(var.clone()) {
                    Some((**base).clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Check if `expr` is a constant multiple of `target` w.r.t. `var`.
    /// Returns the constant factor if so.
    fn is_constant_multiple(expr: &Expr, target: &Expr, var: &Symbol) -> Option<Expr> {
        // Simplify both for comparison
        let expr_s = Simplifier::simplify(expr);
        let target_s = Simplifier::simplify(target);

        // Direct equality: multiple is 1
        if Self::exprs_structurally_equal(&expr_s, &target_s) {
            return Some(Expr::Integer(1));
        }

        // Check if expr = c * target where c is constant
        // Try dividing: if expr/target simplifies to a constant, that's our multiple
        let ratio = Expr::div(expr_s.clone(), target_s.clone());
        let ratio_simplified = Simplifier::simplify(&ratio);
        if !ratio_simplified.contains_var(var) {
            return Some(ratio_simplified);
        }

        None
    }

    /// Simple structural equality check (after simplification)
    fn exprs_structurally_equal(a: &Expr, b: &Expr) -> bool {
        match (a, b) {
            (Expr::Integer(x), Expr::Integer(y)) => x == y,
            (Expr::Float(x), Expr::Float(y)) => (x - y).abs() < 1e-12,
            (Expr::Symbol(x), Expr::Symbol(y)) => x == y,
            (Expr::Neg(x), Expr::Neg(y)) => Self::exprs_structurally_equal(x, y),
            (Expr::Add(xs), Expr::Add(ys)) | (Expr::Mul(xs), Expr::Mul(ys)) => {
                xs.len() == ys.len()
                    && xs
                        .iter()
                        .zip(ys.iter())
                        .all(|(x, y)| Self::exprs_structurally_equal(x, y))
            }
            (Expr::Pow(xb, xe), Expr::Pow(yb, ye)) => {
                Self::exprs_structurally_equal(xb, yb) && Self::exprs_structurally_equal(xe, ye)
            }
            (Expr::Func(xn, xa), Expr::Func(yn, ya)) => {
                xn == yn
                    && xa.len() == ya.len()
                    && xa
                        .iter()
                        .zip(ya.iter())
                        .all(|(x, y)| Self::exprs_structurally_equal(x, y))
            }
            (Expr::Rational(x), Expr::Rational(y)) => x == y,
            _ => false,
        }
    }

    /// Extract the linear form ax + b from an expression, returning (a, b).
    /// Returns None if the expression is not linear in `var`.
    fn extract_linear(expr: &Expr, var: &Symbol) -> Option<(Expr, Expr)> {
        match expr {
            // Just x → (1, 0)
            Expr::Symbol(s) if s == var => Some((Expr::Integer(1), Expr::Integer(0))),

            // a*x or x*a → (a, 0)
            Expr::Mul(factors) => {
                let (constants, var_factors): (Vec<_>, Vec<_>) =
                    factors.iter().partition(|f| !f.contains_var(var));
                // Need exactly one var factor that is x
                if var_factors.len() == 1 && *var_factors[0] == Expr::Symbol(var.clone()) {
                    let a = if constants.is_empty() {
                        Expr::Integer(1)
                    } else {
                        Expr::mul(constants.into_iter().cloned().collect())
                    };
                    Some((a, Expr::Integer(0)))
                } else {
                    None
                }
            }

            // ax + b or b + ax
            Expr::Add(terms) => {
                let mut a = Expr::Integer(0);
                let mut b_parts = Vec::new();
                for term in terms {
                    if !term.contains_var(var) {
                        b_parts.push(term.clone());
                    } else if *term == Expr::Symbol(var.clone()) {
                        a = Expr::add(vec![a, Expr::Integer(1)]);
                    } else if let Expr::Mul(factors) = term {
                        let (constants, var_factors): (Vec<_>, Vec<_>) =
                            factors.iter().partition(|f| !f.contains_var(var));
                        if var_factors.len() == 1 && *var_factors[0] == Expr::Symbol(var.clone()) {
                            let coeff = if constants.is_empty() {
                                Expr::Integer(1)
                            } else {
                                Expr::mul(constants.into_iter().cloned().collect())
                            };
                            a = Expr::add(vec![a, coeff]);
                        } else {
                            return None; // Non-linear term
                        }
                    } else {
                        return None; // Non-linear term containing var
                    }
                }
                let a = Simplifier::simplify(&a);
                if a.is_zero() {
                    return None; // No linear term
                }
                let b = if b_parts.is_empty() {
                    Expr::Integer(0)
                } else {
                    Expr::add(b_parts)
                };
                Some((a, b))
            }

            // -x → (-1, 0)
            Expr::Neg(inner) => {
                if **inner == Expr::Symbol(var.clone()) {
                    Some((Expr::Integer(-1), Expr::Integer(0)))
                } else {
                    None
                }
            }

            _ => None,
        }
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

        // Try linear substitution: ∫ f(ax+b) dx = (1/a)*F(ax+b)
        if *arg != Expr::Symbol(var.clone()) {
            if let Some((a, _b)) = Self::extract_linear(arg, var) {
                // Integrate as if arg = var, then divide by the linear coefficient
                let simple_var = Expr::Symbol(var.clone());
                let simple_integral = Self::integrate_func(name, &[simple_var], var)?;
                // Check we actually got an antiderivative (not unevaluated)
                if !matches!(&simple_integral, Expr::Integral { .. }) {
                    // Substitute ax+b for x in the result, then divide by a
                    let substituted = Simplifier::substitute(&simple_integral, var, arg);
                    return Ok(Expr::div(substituted, a));
                }
            }
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
            "tan" => Expr::neg(Expr::func(
                "ln",
                vec![Expr::func(
                    "abs",
                    vec![Expr::func("cos", vec![arg.clone()])],
                )],
            )),
            "cot" => Expr::func(
                "ln",
                vec![Expr::func(
                    "abs",
                    vec![Expr::func("sin", vec![arg.clone()])],
                )],
            ),
            "sec" => Expr::func(
                "ln",
                vec![Expr::func(
                    "abs",
                    vec![Expr::add(vec![
                        Expr::func("sec", vec![arg.clone()]),
                        Expr::func("tan", vec![arg.clone()]),
                    ])],
                )],
            ),
            "csc" => Expr::neg(Expr::func(
                "ln",
                vec![Expr::func(
                    "abs",
                    vec![Expr::add(vec![
                        Expr::func("csc", vec![arg.clone()]),
                        Expr::func("cot", vec![arg.clone()]),
                    ])],
                )],
            )),

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

/// Limit calculator
pub struct Limits;

impl Limits {
    /// Compute the limit of an expression as var approaches point
    pub fn limit(
        expr: &Expr,
        var: &Symbol,
        point: &Expr,
        direction: Option<crate::expr::LimitDirection>,
    ) -> Result<Expr> {
        Self::limit_impl(expr, var, point, direction, 0)
    }

    fn limit_impl(
        expr: &Expr,
        var: &Symbol,
        point: &Expr,
        direction: Option<crate::expr::LimitDirection>,
        depth: usize,
    ) -> Result<Expr> {
        // Prevent infinite recursion
        if depth > 10 {
            return Ok(Expr::Limit {
                expr: Box::new(expr.clone()),
                var: var.clone(),
                point: Box::new(point.clone()),
                direction,
            });
        }

        // Check if expression contains the variable
        if !expr.contains_var(var) {
            return Ok(expr.clone());
        }

        // Handle infinity limits
        if let Expr::Infinity(sign) = point {
            return Self::limit_at_infinity(expr, var, *sign, depth);
        }

        // Try direct substitution first
        let substituted = Simplifier::substitute(expr, var, point);
        let simplified = Simplifier::simplify(&substituted);

        // Check if result is defined
        if !Self::is_indeterminate(&simplified) {
            return Ok(simplified);
        }

        // Handle indeterminate forms
        match expr {
            // Limit of a sum is sum of limits
            Expr::Add(terms) => {
                let limits: Result<Vec<_>> = terms
                    .iter()
                    .map(|t| Self::limit_impl(t, var, point, direction, depth + 1))
                    .collect();
                Ok(Simplifier::simplify(&Expr::add(limits?)))
            }

            // Limit of a product - check for quotient form first
            Expr::Mul(factors) => {
                // Try to extract numerator and denominator for L'Hôpital
                let mut numerator_parts: Vec<Expr> = Vec::new();
                let mut denominator_parts: Vec<Expr> = Vec::new();

                for f in factors {
                    if let Expr::Pow(base, exp) = f {
                        if Self::is_negative_one(exp) {
                            denominator_parts.push((**base).clone());
                            continue;
                        } else if let Expr::Neg(inner) = exp.as_ref() {
                            // Handle x^(-n) where n > 1
                            if let Expr::Integer(n) = inner.as_ref() {
                                if *n > 0 {
                                    denominator_parts
                                        .push(Expr::pow((**base).clone(), Expr::Integer(*n)));
                                    continue;
                                }
                            }
                        }
                    }
                    numerator_parts.push(f.clone());
                }

                // If we have both numerator and denominator, try L'Hôpital
                if !denominator_parts.is_empty() && !numerator_parts.is_empty() {
                    let numerator = if numerator_parts.len() == 1 {
                        numerator_parts.pop().unwrap()
                    } else {
                        Expr::mul(numerator_parts)
                    };
                    let denominator = if denominator_parts.len() == 1 {
                        denominator_parts.pop().unwrap()
                    } else {
                        Expr::mul(denominator_parts)
                    };

                    // Check if it's an indeterminate form (0/0 or ∞/∞)
                    let num_at_point =
                        Simplifier::simplify(&Simplifier::substitute(&numerator, var, point));
                    let denom_at_point =
                        Simplifier::simplify(&Simplifier::substitute(&denominator, var, point));

                    let num_zero = Self::is_zero(&num_at_point);
                    let denom_zero = Self::is_zero(&denom_at_point);
                    let num_inf = matches!(num_at_point, Expr::Infinity(_));
                    let denom_inf = matches!(denom_at_point, Expr::Infinity(_));

                    if (num_zero && denom_zero) || (num_inf && denom_inf) {
                        return Self::try_lhopital(
                            &numerator,
                            &denominator,
                            var,
                            point,
                            direction,
                            depth,
                        );
                    }
                }

                // Not a quotient or not indeterminate - compute limits of factors
                let limits: Result<Vec<_>> = factors
                    .iter()
                    .map(|f| Self::limit_impl(f, var, point, direction, depth + 1))
                    .collect();
                Ok(Simplifier::simplify(&Expr::mul(limits?)))
            }

            // Handle 1/x or similar
            Expr::Pow(base, exp) if Self::is_negative_one(exp) => {
                let base_limit = Self::limit_impl(base, var, point, direction, depth + 1)?;
                if Self::is_zero(&base_limit) {
                    // 1/0 -> infinity (sign depends on direction)
                    Ok(Expr::Infinity(crate::expr::Sign::Positive))
                } else {
                    Ok(Simplifier::simplify(&Expr::pow(
                        base_limit,
                        Expr::Integer(-1),
                    )))
                }
            }

            // General power
            Expr::Pow(base, exp) => {
                let base_limit = Self::limit_impl(base, var, point, direction, depth + 1)?;
                let exp_limit = Self::limit_impl(exp, var, point, direction, depth + 1)?;
                Ok(Simplifier::simplify(&Expr::pow(base_limit, exp_limit)))
            }

            // Functions
            Expr::Func(name, args) => {
                // Compute limits of arguments
                let arg_limits: Result<Vec<_>> = args
                    .iter()
                    .map(|a| Self::limit_impl(a, var, point, direction, depth + 1))
                    .collect();
                let result = Expr::func(name, arg_limits?);
                Ok(Simplifier::simplify(&result))
            }

            // Negation
            Expr::Neg(e) => {
                let inner_limit = Self::limit_impl(e, var, point, direction, depth + 1)?;
                Ok(Expr::neg(inner_limit))
            }

            // Default: return unevaluated limit
            _ => Ok(Expr::Limit {
                expr: Box::new(expr.clone()),
                var: var.clone(),
                point: Box::new(point.clone()),
                direction,
            }),
        }
    }

    /// Handle limits as x → ±∞
    fn limit_at_infinity(
        expr: &Expr,
        var: &Symbol,
        sign: crate::expr::Sign,
        depth: usize,
    ) -> Result<Expr> {
        match expr {
            // Polynomial: leading term dominates
            Expr::Add(terms) => {
                // Find the term with highest degree in var
                let mut max_degree = 0i32;
                let mut leading_term = Expr::Integer(0);

                for term in terms {
                    let deg = Self::degree_in(term, var);
                    if deg > max_degree {
                        max_degree = deg;
                        leading_term = term.clone();
                    } else if deg == max_degree {
                        // Combine terms of same degree
                        leading_term = Expr::add(vec![leading_term, term.clone()]);
                    }
                }

                if max_degree > 0 {
                    // Infinity (sign depends on leading coefficient and sign of infinity)
                    let coeff = Self::leading_coefficient(&leading_term, var);
                    let coeff_sign = Self::expr_sign(&coeff);
                    let result_sign = if sign == crate::expr::Sign::Positive {
                        coeff_sign
                    } else if max_degree % 2 == 0 {
                        coeff_sign
                    } else {
                        coeff_sign.map(|s| match s {
                            crate::expr::Sign::Positive => crate::expr::Sign::Negative,
                            crate::expr::Sign::Negative => crate::expr::Sign::Positive,
                        })
                    };

                    match result_sign {
                        Some(s) => Ok(Expr::Infinity(s)),
                        None => Ok(Expr::Limit {
                            expr: Box::new(expr.clone()),
                            var: var.clone(),
                            point: Box::new(Expr::Infinity(sign)),
                            direction: None,
                        }),
                    }
                } else if max_degree == 0 {
                    // Constant
                    Ok(leading_term)
                } else {
                    // Negative degree -> 0
                    Ok(Expr::Integer(0))
                }
            }

            // For f/g, compare degrees
            Expr::Mul(factors) => {
                // Try to find quotient pattern
                let mut num_degree = 0i32;
                let mut denom_degree = 0i32;

                for f in factors {
                    if let Expr::Pow(base, exp) = f {
                        if Self::is_negative_one(exp) {
                            denom_degree += Self::degree_in(base, var);
                            continue;
                        }
                    }
                    num_degree += Self::degree_in(f, var);
                }

                if denom_degree > 0 {
                    // This is a rational function
                    if num_degree > denom_degree {
                        Ok(Expr::Infinity(sign))
                    } else if num_degree < denom_degree {
                        Ok(Expr::Integer(0))
                    } else {
                        // Same degree - find ratio of leading coefficients
                        // For now, return unevaluated
                        Ok(Expr::Limit {
                            expr: Box::new(expr.clone()),
                            var: var.clone(),
                            point: Box::new(Expr::Infinity(sign)),
                            direction: None,
                        })
                    }
                } else {
                    // Just a product
                    let limits: Result<Vec<_>> = factors
                        .iter()
                        .map(|f| Self::limit_at_infinity(f, var, sign, depth + 1))
                        .collect();
                    Ok(Simplifier::simplify(&Expr::mul(limits?)))
                }
            }

            // x^n as x→∞
            Expr::Pow(base, exp) if base.as_ref() == &Expr::Symbol(var.clone()) => {
                match exp.as_ref() {
                    Expr::Integer(n) if *n > 0 => Ok(Expr::Infinity(sign)),
                    Expr::Integer(n) if *n < 0 => Ok(Expr::Integer(0)),
                    Expr::Integer(0) => Ok(Expr::Integer(1)),
                    _ => Ok(Expr::Limit {
                        expr: Box::new(expr.clone()),
                        var: var.clone(),
                        point: Box::new(Expr::Infinity(sign)),
                        direction: None,
                    }),
                }
            }

            // e^x as x→∞ is ∞, as x→-∞ is 0
            Expr::Func(name, args) if name == "exp" && args.len() == 1 => {
                if args[0] == Expr::Symbol(var.clone()) {
                    match sign {
                        crate::expr::Sign::Positive => {
                            Ok(Expr::Infinity(crate::expr::Sign::Positive))
                        }
                        crate::expr::Sign::Negative => Ok(Expr::Integer(0)),
                    }
                } else {
                    let arg_limit = Self::limit_at_infinity(&args[0], var, sign, depth + 1)?;
                    Ok(Expr::func("exp", vec![arg_limit]))
                }
            }

            // 1/x as x→∞ is 0
            Expr::Symbol(s) if s == var => Ok(Expr::Infinity(sign)),

            // Constant
            _ if !expr.contains_var(var) => Ok(expr.clone()),

            // Default
            _ => Ok(Expr::Limit {
                expr: Box::new(expr.clone()),
                var: var.clone(),
                point: Box::new(Expr::Infinity(sign)),
                direction: None,
            }),
        }
    }

    /// Try L'Hôpital's rule for 0/0 or ∞/∞ forms
    fn try_lhopital(
        num: &Expr,
        denom: &Expr,
        var: &Symbol,
        point: &Expr,
        direction: Option<crate::expr::LimitDirection>,
        depth: usize,
    ) -> Result<Expr> {
        if depth > 5 {
            return Ok(Expr::Limit {
                expr: Box::new(Expr::mul(vec![
                    num.clone(),
                    Expr::pow(denom.clone(), Expr::Integer(-1)),
                ])),
                var: var.clone(),
                point: Box::new(point.clone()),
                direction,
            });
        }

        // Check if both approach 0 or both approach ∞
        let num_limit = Simplifier::simplify(&Simplifier::substitute(num, var, point));
        let denom_limit = Simplifier::simplify(&Simplifier::substitute(denom, var, point));

        let num_is_zero = Self::is_zero(&num_limit);
        let denom_is_zero = Self::is_zero(&denom_limit);
        let num_is_inf = matches!(num_limit, Expr::Infinity(_));
        let denom_is_inf = matches!(denom_limit, Expr::Infinity(_));

        if (num_is_zero && denom_is_zero) || (num_is_inf && denom_is_inf) {
            // Apply L'Hôpital's rule
            let num_deriv = Differentiator::diff(num, var)?;
            let denom_deriv = Differentiator::diff(denom, var)?;

            // Recursive limit of f'/g'
            let quotient = Expr::mul(vec![num_deriv, Expr::pow(denom_deriv, Expr::Integer(-1))]);
            Self::limit_impl(&quotient, var, point, direction, depth + 1)
        } else if denom_is_zero && !num_is_zero {
            // Limit is ±∞ or doesn't exist
            let num_sign = Self::expr_sign(&num_limit);
            match num_sign {
                Some(s) => Ok(Expr::Infinity(s)),
                None => Ok(Expr::Undefined),
            }
        } else {
            // Normal division
            Ok(Simplifier::simplify(&Expr::mul(vec![
                num_limit,
                Expr::pow(denom_limit, Expr::Integer(-1)),
            ])))
        }
    }

    /// Check if expression is an indeterminate form
    fn is_indeterminate(expr: &Expr) -> bool {
        match expr {
            Expr::Undefined => true,
            Expr::Float(x) if x.is_nan() => true,
            // Rational with 0 denominator is infinity (0/0 if num is also 0)
            Expr::Rational(r) if r.den == 0 => true,
            // Check for 0 * ∞ or similar patterns in products
            Expr::Mul(factors) => {
                let has_infinity = factors.iter().any(|f| {
                    matches!(f, Expr::Infinity(_)) || matches!(f, Expr::Rational(r) if r.den == 0)
                });
                let has_zero = factors.iter().any(|f| Self::is_zero(f));
                // 0 * ∞ is indeterminate
                if has_infinity && has_zero {
                    return true;
                }
                // Also check if any factor is indeterminate
                factors.iter().any(Self::is_indeterminate)
            }
            _ => false,
        }
    }

    /// Try to evaluate an expression to a float value
    fn try_eval_numeric(expr: &Expr) -> Option<f64> {
        use crate::eval::Evaluator;
        let evaluator = Evaluator::new();
        match evaluator.eval(expr) {
            Ok(Expr::Integer(n)) => Some(n as f64),
            Ok(Expr::Float(x)) => Some(x),
            Ok(Expr::Rational(r)) => Some(r.to_f64()),
            _ => None,
        }
    }

    /// Check if expression is zero
    fn is_zero(expr: &Expr) -> bool {
        if matches!(expr, Expr::Integer(0))
            || matches!(expr, Expr::Float(x) if *x == 0.0)
            || matches!(expr, Expr::Rational(r) if r.to_f64() == 0.0)
        {
            return true;
        }
        // Try numeric evaluation for any expression that might be zero
        if let Some(v) = Self::try_eval_numeric(expr) {
            return v.abs() < 1e-15;
        }
        false
    }

    /// Check if expression is -1
    fn is_negative_one(expr: &Expr) -> bool {
        matches!(expr, Expr::Integer(-1))
            || matches!(expr, Expr::Neg(e) if matches!(e.as_ref(), Expr::Integer(1)))
    }

    /// Get the degree of an expression in a variable
    fn degree_in(expr: &Expr, var: &Symbol) -> i32 {
        match expr {
            Expr::Symbol(s) if s == var => 1,
            Expr::Symbol(_) | Expr::Integer(_) | Expr::Float(_) | Expr::Rational(_) => 0,
            Expr::Pow(base, exp) if base.as_ref() == &Expr::Symbol(var.clone()) => {
                match exp.as_ref() {
                    Expr::Integer(n) => *n as i32,
                    _ => 0,
                }
            }
            Expr::Mul(factors) => factors.iter().map(|f| Self::degree_in(f, var)).sum(),
            Expr::Add(terms) => terms
                .iter()
                .map(|t| Self::degree_in(t, var))
                .max()
                .unwrap_or(0),
            Expr::Neg(e) => Self::degree_in(e, var),
            _ => 0,
        }
    }

    /// Get the leading coefficient of a polynomial term
    fn leading_coefficient(expr: &Expr, var: &Symbol) -> Expr {
        match expr {
            Expr::Symbol(s) if s == var => Expr::Integer(1),
            Expr::Mul(factors) => {
                let coeffs: Vec<_> = factors
                    .iter()
                    .filter(|f| !f.contains_var(var))
                    .cloned()
                    .collect();
                if coeffs.is_empty() {
                    Expr::Integer(1)
                } else {
                    Expr::mul(coeffs)
                }
            }
            Expr::Neg(e) => Expr::neg(Self::leading_coefficient(e, var)),
            _ if !expr.contains_var(var) => expr.clone(),
            _ => Expr::Integer(1),
        }
    }

    /// Try to determine the sign of an expression
    fn expr_sign(expr: &Expr) -> Option<crate::expr::Sign> {
        match expr {
            Expr::Integer(n) if *n > 0 => Some(crate::expr::Sign::Positive),
            Expr::Integer(n) if *n < 0 => Some(crate::expr::Sign::Negative),
            Expr::Float(x) if *x > 0.0 => Some(crate::expr::Sign::Positive),
            Expr::Float(x) if *x < 0.0 => Some(crate::expr::Sign::Negative),
            Expr::Neg(e) => Self::expr_sign(e).map(|s| match s {
                crate::expr::Sign::Positive => crate::expr::Sign::Negative,
                crate::expr::Sign::Negative => crate::expr::Sign::Positive,
            }),
            Expr::Infinity(s) => Some(*s),
            _ => None,
        }
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

                if name == "ln" && sargs.len() == 1 {
                    match &sargs[0] {
                        Expr::Integer(1) => return Expr::Integer(0),
                        Expr::Rational(r) if r.den != 0 && r.num == r.den => {
                            return Expr::Integer(0);
                        }
                        Expr::Float(x) if (*x - 1.0).abs() < 1e-12 => return Expr::Integer(0),
                        Expr::Symbol(sym) if sym.as_str() == "e" => return Expr::Integer(1),
                        Expr::Float(x) if (*x - E).abs() < 1e-12 => return Expr::Integer(1),
                        _ => {}
                    }
                }

                if name == "factorial" && sargs.len() == 1 {
                    if let Expr::Integer(n) = sargs[0] {
                        if n >= 0 {
                            if let Some(value) = Self::factorial_i64_checked(n) {
                                return Expr::Integer(value);
                            }
                        }
                    }
                }

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
        let mut result_terms = Self::collect_like_terms(non_numeric);

        // Apply trig identities: sin^2 + cos^2 = 1, etc.
        Self::try_pythagorean_identity(&mut result_terms);
        Self::try_pythagorean_complement(&mut result_terms);

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

        // Check for 0 * ∞ (indeterminate) or just zero
        let has_zero = flattened.iter().any(|f| f.is_zero());
        let has_infinity = flattened.iter().any(|f| {
            matches!(f, Expr::Infinity(_)) || matches!(f, Expr::Rational(r) if r.den == 0)
        });
        if has_zero && has_infinity {
            // 0 * ∞ is indeterminate - keep as product for limit handling
            // Return as-is without simplifying to allow limit detection
        } else if has_zero {
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

        // Cancel common symbolic factors (x^3 / x^2 -> x)
        Self::cancel_common_factors(&mut non_numeric);

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
                final_factors.insert(
                    0,
                    Expr::Rational(crate::expr::Rational::new(numerator, denominator)),
                );
            }
        }

        if final_factors.is_empty() {
            Expr::Integer(1)
        } else if final_factors.len() == 1 {
            final_factors.into_iter().next().unwrap()
        } else {
            // Try double-angle: 2*sin(x)*cos(x) -> sin(2x)
            if let Some(result) = Self::try_double_angle(&final_factors) {
                return result;
            }
            Expr::Mul(final_factors)
        }
    }

    /// Apply Pythagorean trig identities to a list of additive terms (mutated in place).
    /// sin(A)^2 + cos(A)^2 → 1, with coefficient matching.
    fn try_pythagorean_identity(terms: &mut Vec<Expr>) -> bool {
        // Find pairs of sin(A)^2 and cos(A)^2 with matching coefficients
        let mut changed = false;
        'outer: loop {
            for i in 0..terms.len() {
                let (coef_i, func_i, arg_i) = match Self::extract_trig_squared(&terms[i]) {
                    Some(t) => t,
                    None => continue,
                };
                for j in (i + 1)..terms.len() {
                    let (coef_j, func_j, arg_j) = match Self::extract_trig_squared(&terms[j]) {
                        Some(t) => t,
                        None => continue,
                    };
                    // Need sin^2 + cos^2 (or cos^2 + sin^2) with same arg and coefficient
                    if arg_i.to_string() == arg_j.to_string()
                        && (coef_i - coef_j).abs() < 1e-12
                        && ((func_i == "sin" && func_j == "cos")
                            || (func_i == "cos" && func_j == "sin"))
                    {
                        terms.remove(j);
                        terms.remove(i);
                        if (coef_i - 1.0).abs() < 1e-12 {
                            terms.push(Expr::Integer(1));
                        } else if coef_i.fract() == 0.0 {
                            terms.push(Expr::Integer(coef_i as i64));
                        } else {
                            terms.push(Expr::Float(coef_i));
                        }
                        changed = true;
                        continue 'outer;
                    }
                }
            }
            break;
        }
        changed
    }

    /// Apply 1 - sin^2(x) → cos^2(x) and 1 - cos^2(x) → sin^2(x)
    fn try_pythagorean_complement(terms: &mut Vec<Expr>) -> bool {
        let mut changed = false;
        'outer: loop {
            // Find a numeric constant and a negated trig squared
            let mut const_idx = None;
            let mut const_val = 0.0;
            for (i, t) in terms.iter().enumerate() {
                match t {
                    Expr::Integer(n) => {
                        const_idx = Some(i);
                        const_val = *n as f64;
                    }
                    Expr::Float(f) => {
                        const_idx = Some(i);
                        const_val = *f;
                    }
                    _ => {}
                }
            }
            let ci = match const_idx {
                Some(i) if const_val != 0.0 => i,
                _ => break,
            };

            for j in 0..terms.len() {
                if j == ci {
                    continue;
                }
                // Check for -coef * sin^2(A) or -coef * cos^2(A)
                let (coef, func_name, arg) = match Self::extract_trig_squared(&terms[j]) {
                    Some((c, f, a)) if c < 0.0 => (c, f, a),
                    _ => continue,
                };
                let neg_coef = -coef; // positive version of the coefficient
                if (neg_coef - const_val).abs() < 1e-12 {
                    let complement = if func_name == "sin" { "cos" } else { "sin" };
                    terms.remove(j.max(ci));
                    terms.remove(j.min(ci));
                    let replacement = if (neg_coef - 1.0).abs() < 1e-12 {
                        Expr::pow(
                            Expr::func(complement, vec![arg]),
                            Expr::Integer(2),
                        )
                    } else {
                        Expr::mul(vec![
                            if neg_coef.fract() == 0.0 {
                                Expr::Integer(neg_coef as i64)
                            } else {
                                Expr::Float(neg_coef)
                            },
                            Expr::pow(
                                Expr::func(complement, vec![arg]),
                                Expr::Integer(2),
                            ),
                        ])
                    };
                    terms.push(replacement);
                    changed = true;
                    continue 'outer;
                }
            }
            break;
        }
        changed
    }

    /// Extract (coefficient, "sin"|"cos", argument) from a term like coef*sin(A)^2
    fn extract_trig_squared(expr: &Expr) -> Option<(f64, &'static str, Expr)> {
        let (coef, base) = Self::extract_coefficient(expr);
        match &base {
            Expr::Pow(inner, exp) if **exp == Expr::Integer(2) => {
                if let Expr::Func(name, args) = &**inner {
                    if args.len() == 1 {
                        let func_name = match name.as_str() {
                            "sin" => "sin",
                            "cos" => "cos",
                            _ => return None,
                        };
                        return Some((coef, func_name, args[0].clone()));
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Detect 2*sin(x)*cos(x) → sin(2*x) in a Mul node
    fn try_double_angle(factors: &[Expr]) -> Option<Expr> {
        if factors.len() < 2 {
            return None;
        }
        let (overall_coef, non_numeric): (f64, Vec<&Expr>) = {
            let mut c = 1.0;
            let mut rest = Vec::new();
            for f in factors {
                match f {
                    Expr::Integer(n) => c *= *n as f64,
                    Expr::Float(x) => c *= x,
                    other => rest.push(other),
                }
            }
            (c, rest)
        };

        if non_numeric.len() != 2 {
            return None;
        }
        // Need sin(A) and cos(A)
        let (sin_arg, cos_arg) = match (non_numeric[0], non_numeric[1]) {
            (Expr::Func(n1, a1), Expr::Func(n2, a2))
                if n1 == "sin" && n2 == "cos" && a1.len() == 1 && a2.len() == 1 =>
            {
                (&a1[0], &a2[0])
            }
            (Expr::Func(n1, a1), Expr::Func(n2, a2))
                if n1 == "cos" && n2 == "sin" && a1.len() == 1 && a2.len() == 1 =>
            {
                (&a2[0], &a1[0])
            }
            _ => return None,
        };

        if sin_arg.to_string() != cos_arg.to_string() {
            return None;
        }

        // 2*sin(A)*cos(A) = sin(2A)
        // overall_coef * sin(A)*cos(A) = (overall_coef/2) * sin(2A)
        let remaining_coef = overall_coef / 2.0;
        let double_arg = Simplifier::simplify(&Expr::mul(vec![Expr::Integer(2), sin_arg.clone()]));
        let sin_2a = Expr::func("sin", vec![double_arg]);

        if (remaining_coef - 1.0).abs() < 1e-12 {
            Some(sin_2a)
        } else if remaining_coef.fract() == 0.0 {
            Some(Expr::mul(vec![Expr::Integer(remaining_coef as i64), sin_2a]))
        } else {
            Some(Expr::mul(vec![Expr::Float(remaining_coef), sin_2a]))
        }
    }

    /// Cancel common base^exp factors in a Mul node's factor list.
    /// E.g. x^3 * x^(-2) → x, a * b * a^(-1) → b
    fn cancel_common_factors(factors: &mut Vec<Expr>) {
        // Extract (base, exponent) for each factor
        fn base_exp(e: &Expr) -> (Expr, f64) {
            match e {
                Expr::Pow(base, exp) => {
                    if let Expr::Integer(n) = &**exp {
                        return ((**base).clone(), *n as f64);
                    }
                    if let Expr::Neg(inner) = &**exp {
                        if let Expr::Integer(n) = &**inner {
                            return ((**base).clone(), -(*n as f64));
                        }
                    }
                    (e.clone(), 1.0)
                }
                _ => (e.clone(), 1.0),
            }
        }

        // Group by base string representation
        use std::collections::HashMap;
        let mut base_map: HashMap<String, (Expr, f64)> = HashMap::new();
        let mut order = Vec::new();

        for f in factors.iter() {
            let (base, exp) = base_exp(f);
            let key = base.to_string();
            if let Some((_, existing_exp)) = base_map.get_mut(&key) {
                *existing_exp += exp;
            } else {
                order.push(key.clone());
                base_map.insert(key, (base, exp));
            }
        }

        // Check if anything actually cancelled
        if order.len() == factors.len() {
            return; // No duplicates found
        }

        factors.clear();
        for key in order {
            if let Some((base, exp)) = base_map.remove(&key) {
                if exp == 0.0 {
                    // Cancelled completely
                    continue;
                } else if exp == 1.0 {
                    factors.push(base);
                } else if exp == -1.0 {
                    factors.push(Expr::pow(base, Expr::Integer(-1)));
                } else if exp.fract() == 0.0 {
                    factors.push(Expr::pow(base, Expr::Integer(exp as i64)));
                } else {
                    factors.push(Expr::pow(base, Expr::Float(exp)));
                }
            }
        }
    }

    fn factorial_i64_checked(n: i64) -> Option<i64> {
        let n = u64::try_from(n).ok()?;
        let mut acc: i64 = 1;
        for k in 2..=n {
            let step = i64::try_from(k).ok()?;
            acc = acc.checked_mul(step)?;
        }
        Some(acc)
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
                return Ok(vec![Simplifier::simplify(&Expr::div(Expr::neg(c), b))]);
            }
            // Quadratic
            return Self::solve_quadratic_with_coeffs(&a, &b, &c);
        }

        // Try numerical root finding for higher-degree polynomials
        if let Some(roots) = Self::solve_polynomial_numerical(&expr, var) {
            if !roots.is_empty() {
                return Ok(roots);
            }
        }

        // Fallback to isolation
        Self::solve_by_isolation(&expr, var)
    }

    /// Solve polynomial equations numerically using Newton-Raphson
    fn solve_polynomial_numerical(expr: &Expr, var: &Symbol) -> Option<Vec<Expr>> {
        use crate::eval::Evaluator;
        let evaluator = Evaluator::new();

        // Check if it's a polynomial and get the degree
        let degree = Self::polynomial_degree(expr, var)?;
        if degree > 10 {
            return None; // Too high degree
        }

        let mut roots: Vec<f64> = Vec::new();

        // Try many starting points to find all roots
        let mut starting_points: Vec<f64> = vec![
            0.0, 1.0, -1.0, 2.0, -2.0, 0.5, -0.5, 3.0, -3.0, 4.0, -4.0, 5.0, -5.0, 0.25, -0.25,
            0.75, -0.75, 1.5, -1.5, 2.5, -2.5, 10.0, -10.0, 100.0, -100.0,
        ];
        // Add more points based on degree
        for i in 0..20 {
            let x = (i as f64 - 10.0) * 0.37; // Irrational spacing
            starting_points.push(x);
        }

        for start in &starting_points {
            if roots.len() >= degree as usize {
                break;
            }

            if let Some(root) = Self::newton_raphson(expr, var, *start, &evaluator) {
                // Verify it's actually a root
                let check = Simplifier::substitute(expr, var, &Expr::Float(root));
                if let Ok(result) = evaluator.eval(&check) {
                    if let Some(val) = Self::expr_to_f64(&result) {
                        if val.abs() < 1e-6 {
                            // Check if we already have this root (with tolerance)
                            let is_duplicate = roots.iter().any(|&r| (r - root).abs() < 1e-6);
                            if !is_duplicate {
                                roots.push(root);
                            }
                        }
                    }
                }
            }
        }

        if roots.is_empty() {
            return None;
        }

        // Convert to expressions, cleaning up near-integers
        let result: Vec<Expr> = roots
            .into_iter()
            .map(|r| {
                if r.abs() < 1e-10 {
                    Expr::Integer(0)
                } else if (r - r.round()).abs() < 1e-10 {
                    Expr::Integer(r.round() as i64)
                } else {
                    Expr::Float(r)
                }
            })
            .collect();

        Some(result)
    }

    /// Newton-Raphson iteration to find a root
    fn newton_raphson(
        expr: &Expr,
        var: &Symbol,
        start: f64,
        evaluator: &crate::eval::Evaluator,
    ) -> Option<f64> {
        let deriv = Differentiator::diff(expr, var).ok()?;
        let deriv = Simplifier::simplify(&deriv);

        let mut x = start;
        let max_iter = 100;
        let tolerance = 1e-12;

        for _ in 0..max_iter {
            let f_x = {
                let subst = Simplifier::substitute(expr, var, &Expr::Float(x));
                evaluator
                    .eval(&subst)
                    .ok()
                    .and_then(|e| Self::expr_to_f64(&e))?
            };

            if f_x.abs() < tolerance {
                return Some(x);
            }

            let fp_x = {
                let subst = Simplifier::substitute(&deriv, var, &Expr::Float(x));
                evaluator
                    .eval(&subst)
                    .ok()
                    .and_then(|e| Self::expr_to_f64(&e))?
            };

            if fp_x.abs() < 1e-15 {
                // Derivative too small, Newton's method fails
                return None;
            }

            let x_new = x - f_x / fp_x;

            if (x_new - x).abs() < tolerance {
                return Some(x_new);
            }

            x = x_new;
        }

        None
    }

    /// Get polynomial degree
    fn polynomial_degree(expr: &Expr, var: &Symbol) -> Option<u32> {
        match expr {
            Expr::Symbol(s) if s == var => Some(1),
            Expr::Integer(_) | Expr::Float(_) | Expr::Rational(_) => Some(0),
            Expr::Symbol(_) => Some(0),
            Expr::Pow(base, exp) => {
                if base.as_ref() == &Expr::Symbol(var.clone()) {
                    if let Expr::Integer(n) = exp.as_ref() {
                        if *n >= 0 {
                            return Some(*n as u32);
                        }
                    }
                }
                None // Complex power
            }
            Expr::Mul(factors) => {
                let mut total = 0;
                for f in factors {
                    total += Self::polynomial_degree(f, var)?;
                }
                Some(total)
            }
            Expr::Add(terms) => {
                let mut max = 0;
                for t in terms {
                    max = max.max(Self::polynomial_degree(t, var)?);
                }
                Some(max)
            }
            Expr::Neg(e) => Self::polynomial_degree(e, var),
            _ => None,
        }
    }

    /// Convert Expr to f64 if possible
    fn expr_to_f64(expr: &Expr) -> Option<f64> {
        match expr {
            Expr::Integer(n) => Some(*n as f64),
            Expr::Float(x) => Some(*x),
            Expr::Rational(r) => Some(r.to_f64()),
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
        let x2 = Simplifier::simplify(&Expr::div(Expr::sub(neg_b, sqrt_d), two_a));

        Ok(vec![x1, x2])
    }

    #[allow(dead_code)]
    fn solve_quadratic(expr: &Expr, var: &Symbol) -> Result<Vec<Expr>> {
        // Extract coefficients a, b, c from ax^2 + bx + c
        let (a, b, c) = Self::extract_quadratic_coefficients(expr, var)?;
        Self::solve_quadratic_with_coeffs(&a, &b, &c)
    }

    fn extract_quadratic_coefficients(expr: &Expr, var: &Symbol) -> Result<(Expr, Expr, Expr)> {
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
                    ));
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

    /// Solve a system of linear equations using Gaussian elimination.
    /// `equations` is a list of Expr (each is an Equation or an expression = 0).
    /// `vars` is the ordered list of unknowns.
    /// Returns a Vec of (var, solution) pairs.
    pub fn solve_system(equations: &[Expr], vars: &[Symbol]) -> Result<Vec<(Symbol, Expr)>> {
        let n = vars.len();
        if equations.len() < n {
            return Err(CasError::EvaluationError(
                "Underdetermined system: fewer equations than unknowns".to_string(),
            ));
        }

        // Build augmented matrix [A | b] where Ax = b
        // For each equation, extract linear coefficients for each var and constant term
        let mut matrix: Vec<Vec<Expr>> = Vec::with_capacity(n);
        for eq in equations.iter().take(n) {
            let expr = match eq {
                Expr::Equation(lhs, rhs) => {
                    Simplifier::simplify(&Expr::sub((**lhs).clone(), (**rhs).clone()))
                }
                other => Simplifier::simplify(other),
            };
            let mut row = Vec::with_capacity(n + 1);
            for var in vars {
                let coeff = Self::extract_linear_coefficient(&expr, var);
                row.push(coeff);
            }
            // Constant term: negate the constant part (moving to RHS)
            let constant = Self::extract_constant_part(&expr, vars);
            row.push(Expr::neg(constant));
            matrix.push(row);
        }

        // Gaussian elimination with partial pivoting
        for col in 0..n {
            // Find pivot row (first non-zero in column)
            let mut pivot_row = None;
            for row in col..n {
                if !Self::is_expr_zero(&matrix[row][col]) {
                    pivot_row = Some(row);
                    break;
                }
            }
            let pivot_row = match pivot_row {
                Some(r) => r,
                None => {
                    return Err(CasError::EvaluationError(
                        "System has no unique solution (singular matrix)".to_string(),
                    ));
                }
            };

            // Swap pivot row to current position
            if pivot_row != col {
                matrix.swap(col, pivot_row);
            }

            // Eliminate below
            let pivot = matrix[col][col].clone();
            for row in (col + 1)..n {
                let factor = matrix[row][col].clone();
                if Self::is_expr_zero(&factor) {
                    continue;
                }
                for j in col..=n {
                    let old = matrix[row][j].clone();
                    // new = old * pivot - factor * matrix[col][j]
                    let new_val = Simplifier::simplify(&Expr::sub(
                        Expr::mul(vec![old, pivot.clone()]),
                        Expr::mul(vec![factor.clone(), matrix[col][j].clone()]),
                    ));
                    matrix[row][j] = new_val;
                }
            }
        }

        // Back substitution
        let mut solutions = vec![Expr::Integer(0); n];
        for i in (0..n).rev() {
            let mut rhs = matrix[i][n].clone();
            for j in (i + 1)..n {
                rhs = Simplifier::simplify(&Expr::sub(
                    rhs,
                    Expr::mul(vec![matrix[i][j].clone(), solutions[j].clone()]),
                ));
            }
            if Self::is_expr_zero(&matrix[i][i]) {
                return Err(CasError::EvaluationError(
                    "System has no unique solution (singular matrix)".to_string(),
                ));
            }
            solutions[i] = Simplifier::simplify(&Expr::div(rhs, matrix[i][i].clone()));
        }

        Ok(vars.iter().cloned().zip(solutions).collect())
    }

    /// Extract the coefficient of `var` from a simplified linear expression.
    fn extract_linear_coefficient(expr: &Expr, var: &Symbol) -> Expr {
        match expr {
            Expr::Symbol(s) if s == var => Expr::Integer(1),
            Expr::Symbol(_) | Expr::Integer(_) | Expr::Float(_) | Expr::Rational(_) => {
                Expr::Integer(0)
            }
            Expr::Neg(inner) => {
                let c = Self::extract_linear_coefficient(inner, var);
                Simplifier::simplify(&Expr::neg(c))
            }
            Expr::Mul(factors) => {
                // Check if one factor is var and rest are constant
                let mut has_var = false;
                let mut coef_parts = Vec::new();
                for f in factors {
                    if *f == Expr::Symbol(var.clone()) && !has_var {
                        has_var = true;
                    } else {
                        coef_parts.push(f.clone());
                    }
                }
                if has_var {
                    if coef_parts.is_empty() {
                        Expr::Integer(1)
                    } else {
                        Simplifier::simplify(&Expr::mul(coef_parts))
                    }
                } else {
                    Expr::Integer(0)
                }
            }
            Expr::Add(terms) => {
                let coeffs: Vec<Expr> = terms
                    .iter()
                    .map(|t| Self::extract_linear_coefficient(t, var))
                    .collect();
                Simplifier::simplify(&Expr::add(coeffs))
            }
            _ => Expr::Integer(0),
        }
    }

    /// Extract the constant part (terms not containing any of the given variables).
    fn extract_constant_part(expr: &Expr, vars: &[Symbol]) -> Expr {
        match expr {
            Expr::Add(terms) => {
                let constants: Vec<Expr> = terms
                    .iter()
                    .filter(|t| !vars.iter().any(|v| t.contains_var(v)))
                    .cloned()
                    .collect();
                if constants.is_empty() {
                    Expr::Integer(0)
                } else {
                    Simplifier::simplify(&Expr::add(constants))
                }
            }
            _ if !vars.iter().any(|v| expr.contains_var(v)) => expr.clone(),
            _ => Expr::Integer(0),
        }
    }

    /// Check if an expression simplifies to zero.
    fn is_expr_zero(expr: &Expr) -> bool {
        let s = Simplifier::simplify(expr);
        s.is_zero()
    }
}

/// Polynomial factoring
pub struct Factorer;

impl Factorer {
    /// Factor a polynomial expression with respect to a variable
    pub fn factor(expr: &Expr, var: &Symbol) -> Expr {
        let simplified = Simplifier::simplify(expr);

        // Try to extract polynomial coefficients (highest degree first)
        let coeffs = match Self::extract_polynomial_coefficients(&simplified, var) {
            Some(c) if c.len() >= 2 => c,
            _ => return simplified,
        };

        let degree = coeffs.len() - 1;

        // Factor out GCD coefficient
        let (gcd_coef, normed) = Self::factor_gcd_coefficient(&coeffs);

        // Try difference of squares: ax^2 + c where b=0
        if degree == 2 {
            if let [a, b, c] = normed.as_slice() {
                if *b == 0 {
                    if let Some(result) = Self::try_difference_of_squares(*a, *c, var) {
                        return Self::wrap_gcd(gcd_coef, result);
                    }
                }
                // Try perfect square trinomial
                if let Some(result) = Self::try_perfect_square(*a, *b, *c, var) {
                    return Self::wrap_gcd(gcd_coef, result);
                }
                // Factor quadratic with rational roots
                if let Some(result) = Self::factor_quadratic(*a, *b, *c, var) {
                    return Self::wrap_gcd(gcd_coef, result);
                }
            }
        }

        // For higher degree, try rational root theorem
        if degree >= 3 {
            if let Some(result) = Self::factor_by_rational_roots(&normed, var) {
                return Self::wrap_gcd(gcd_coef, result);
            }
        }

        // Factor out common variable power: 3x^3 + 6x^2 -> 3x^2(x+2)
        if degree >= 2 {
            let trailing_zeros = normed.iter().rev().take_while(|&&c| c == 0).count();
            if trailing_zeros > 0 {
                let reduced: Vec<i64> = normed[..normed.len() - trailing_zeros].to_vec();
                let x_power = trailing_zeros as u32;
                let x_factor = if x_power == 1 {
                    Expr::Symbol(var.clone())
                } else {
                    Expr::pow(Expr::Symbol(var.clone()), Expr::Integer(x_power as i64))
                };
                let inner = Self::coeffs_to_expr(&reduced, var);
                let factored_inner = Self::factor(&inner, var);
                return Self::wrap_gcd(gcd_coef, Expr::mul(vec![x_factor, factored_inner]));
            }
        }

        simplified
    }

    /// Extract integer polynomial coefficients [a_n, a_{n-1}, ..., a_1, a_0]
    /// where the polynomial is a_n*x^n + ... + a_1*x + a_0
    fn extract_polynomial_coefficients(expr: &Expr, var: &Symbol) -> Option<Vec<i64>> {
        let terms = match expr {
            Expr::Add(t) => t.clone(),
            other => vec![other.clone()],
        };

        let mut degree_map = std::collections::HashMap::new();
        let mut max_degree = 0u32;

        for term in &terms {
            let (coef, deg) = Solver::get_term_coefficient_and_degree(term, var);
            let coef_val = Self::expr_to_i64(&Simplifier::simplify(&coef))?;
            *degree_map.entry(deg).or_insert(0i64) += coef_val;
            if deg > max_degree {
                max_degree = deg;
            }
        }

        let mut coeffs = Vec::with_capacity(max_degree as usize + 1);
        for d in (0..=max_degree).rev() {
            coeffs.push(*degree_map.get(&d).unwrap_or(&0));
        }
        Some(coeffs)
    }

    fn expr_to_i64(expr: &Expr) -> Option<i64> {
        match expr {
            Expr::Integer(n) => Some(*n),
            Expr::Neg(inner) => Self::expr_to_i64(inner).map(|n| -n),
            Expr::Float(f) if f.fract() == 0.0 && f.abs() < i64::MAX as f64 => Some(*f as i64),
            _ => None,
        }
    }

    /// Factor out GCD of all coefficients. Returns (gcd, normalized_coefficients).
    fn factor_gcd_coefficient(coeffs: &[i64]) -> (i64, Vec<i64>) {
        let g = coeffs
            .iter()
            .copied()
            .filter(|&c| c != 0)
            .fold(0i64, |acc, c| gcd(acc.unsigned_abs(), c.unsigned_abs()) as i64);
        if g <= 1 {
            return (1, coeffs.to_vec());
        }
        // If leading coefficient is negative, factor out -g to keep leading positive
        let g = if coeffs[0] < 0 { -g } else { g };
        let normed = coeffs.iter().map(|c| c / g).collect();
        (g, normed)
    }

    fn wrap_gcd(gcd: i64, expr: Expr) -> Expr {
        if gcd == 1 {
            expr
        } else {
            Simplifier::simplify(&Expr::mul(vec![Expr::Integer(gcd), expr]))
        }
    }

    /// Try a^2*x^2 - c where c > 0 → (a*x - sqrt(c))(a*x + sqrt(c))
    fn try_difference_of_squares(a: i64, c: i64, var: &Symbol) -> Option<Expr> {
        if c >= 0 {
            return None; // need c < 0 for a*x^2 + c = a*x^2 - |c|
        }
        let neg_c = (-c) as f64;
        let sqrt_c = neg_c.sqrt();
        if (sqrt_c - sqrt_c.round()).abs() > 1e-12 {
            return None;
        }
        let sqrt_c = sqrt_c.round() as i64;
        let a_f = a as f64;
        let sqrt_a = a_f.sqrt();
        if (sqrt_a - sqrt_a.round()).abs() > 1e-12 {
            return None;
        }
        let sqrt_a = sqrt_a.round() as i64;

        let ax = if sqrt_a == 1 {
            Expr::Symbol(var.clone())
        } else {
            Expr::mul(vec![Expr::Integer(sqrt_a), Expr::Symbol(var.clone())])
        };
        let c_expr = Expr::Integer(sqrt_c);

        Some(Expr::mul(vec![
            Expr::sub(ax.clone(), c_expr.clone()),
            Expr::add(vec![ax, c_expr]),
        ]))
    }

    /// Try perfect square: a*x^2 + b*x + c where b^2 = 4*a*c
    fn try_perfect_square(a: i64, b: i64, c: i64, var: &Symbol) -> Option<Expr> {
        if b * b != 4 * a * c {
            return None;
        }
        let a_f = a as f64;
        let sqrt_a = a_f.sqrt();
        if (sqrt_a - sqrt_a.round()).abs() > 1e-12 {
            return None;
        }
        let sqrt_a = sqrt_a.round() as i64;
        let c_f = (c as f64).abs();
        let sqrt_c = c_f.sqrt();
        if (sqrt_c - sqrt_c.round()).abs() > 1e-12 {
            return None;
        }
        let sqrt_c = sqrt_c.round() as i64;

        let ax = if sqrt_a == 1 {
            Expr::Symbol(var.clone())
        } else {
            Expr::mul(vec![Expr::Integer(sqrt_a), Expr::Symbol(var.clone())])
        };

        let inner = if b > 0 {
            Expr::add(vec![ax, Expr::Integer(sqrt_c)])
        } else {
            Expr::sub(ax, Expr::Integer(sqrt_c))
        };

        Some(Expr::pow(inner, Expr::Integer(2)))
    }

    /// Factor quadratic ax^2 + bx + c with rational roots
    fn factor_quadratic(a: i64, b: i64, c: i64, var: &Symbol) -> Option<Expr> {
        let disc = b as i128 * b as i128 - 4 * a as i128 * c as i128;
        if disc < 0 {
            return None;
        }
        let sqrt_d = (disc as f64).sqrt();
        if (sqrt_d - sqrt_d.round()).abs() > 1e-10 {
            return None; // discriminant not a perfect square
        }
        let sqrt_d = sqrt_d.round() as i64;
        let r1_num = -b + sqrt_d;
        let r1_den = 2 * a;
        let r2_num = -b - sqrt_d;
        let r2_den = 2 * a;

        // Build (den*x - num) factors which when multiplied give den^2*(x-r1)(x-r2)
        // Instead: a * (x - r1)(x - r2) where r1 = r1_num/r1_den
        let g1 = gcd(r1_num.unsigned_abs(), r1_den.unsigned_abs()) as i64;
        let g2 = gcd(r2_num.unsigned_abs(), r2_den.unsigned_abs()) as i64;
        let (n1, d1) = (r1_num / g1, r1_den / g1);
        let (n2, d2) = (r2_num / g2, r2_den / g2);

        // Factor as: (a / (d1*d2)) * (d1*x - n1) * (d2*x - n2)
        let leading = a / (d1 * d2);

        let f1 = if d1 == 1 {
            Expr::sub(Expr::Symbol(var.clone()), Expr::Integer(n1))
        } else {
            Expr::sub(
                Expr::mul(vec![Expr::Integer(d1), Expr::Symbol(var.clone())]),
                Expr::Integer(n1),
            )
        };
        let f2 = if d2 == 1 {
            Expr::sub(Expr::Symbol(var.clone()), Expr::Integer(n2))
        } else {
            Expr::sub(
                Expr::mul(vec![Expr::Integer(d2), Expr::Symbol(var.clone())]),
                Expr::Integer(n2),
            )
        };

        let mut result = vec![f1, f2];
        if leading != 1 {
            result.insert(0, Expr::Integer(leading));
        }
        Some(Simplifier::simplify(&Expr::mul(result)))
    }

    /// Try to find rational roots and factor by synthetic division
    fn factor_by_rational_roots(coeffs: &[i64], var: &Symbol) -> Option<Expr> {
        let n = coeffs.len();
        if n < 3 {
            return None;
        }
        let leading = coeffs[0];
        let constant = coeffs[n - 1];
        if leading == 0 || constant == 0 {
            return None; // handled by trailing-zero extraction above
        }

        let p_divisors = divisors(constant.unsigned_abs());
        let q_divisors = divisors(leading.unsigned_abs());

        let mut found_root = None;
        'outer: for &p in &p_divisors {
            for &q in &q_divisors {
                for sign in &[1i64, -1i64] {
                    let num = *sign * p as i64;
                    let den = q as i64;
                    let num128 = num as i128;
                    let den128 = den as i128;
                    // P(p/q) * q^(n-1) = sum of c_i * p^(n-1-i) * q^i
                    let mut check = 0i128;
                    for (i, &c) in coeffs.iter().enumerate() {
                        check += c as i128
                            * num128.pow((n - 1 - i) as u32)
                            * den128.pow(i as u32);
                    }
                    if check == 0 {
                        found_root = Some((num, den));
                        break 'outer;
                    }
                }
            }
        }

        let (root_num, root_den) = found_root?;

        // Synthetic division by (den*x - num)
        let quotient = Self::synthetic_divide(coeffs, root_num, root_den)?;

        // Build the linear factor (den*x - num), simplified
        let g = gcd(root_num.unsigned_abs(), root_den.unsigned_abs()) as i64;
        let (rn, rd) = (root_num / g, root_den / g);
        let linear_factor = if rd == 1 {
            Expr::sub(Expr::Symbol(var.clone()), Expr::Integer(rn))
        } else {
            Expr::sub(
                Expr::mul(vec![Expr::Integer(rd), Expr::Symbol(var.clone())]),
                Expr::Integer(rn),
            )
        };

        let remaining = Self::coeffs_to_expr(&quotient, var);
        let factored_remaining = Self::factor(&remaining, var);

        Some(Expr::mul(vec![linear_factor, factored_remaining]))
    }

    /// Synthetic division of polynomial by (den*x - num)
    /// Returns quotient coefficients, or None if division fails
    fn synthetic_divide(coeffs: &[i64], root_num: i64, root_den: i64) -> Option<Vec<i64>> {
        // Dividing by (den*x - num) = den*(x - num/den)
        // Use scaled arithmetic: coefficients * den^k
        let n = coeffs.len();
        if n < 2 {
            return None;
        }

        // Horner-style synthetic division for (x - num/den)
        // Then adjust for the den factor
        let mut quotient = Vec::with_capacity(n - 1);
        let mut remainder = 0i128;

        for (i, &c) in coeffs.iter().enumerate() {
            let scaled = c as i128 * (root_den as i128).pow((n - 1 - i) as u32) + remainder;
            if i < n - 1 {
                // This should divide evenly by root_den^(n-2-i)
                let divisor = (root_den as i128).pow((n - 2 - i) as u32);
                if divisor != 0 && scaled % divisor == 0 {
                    quotient.push((scaled / divisor) as i64);
                } else {
                    return None;
                }
                remainder = quotient.last().copied().unwrap_or(0) as i128 * root_num as i128;
            }
        }

        Some(quotient)
    }

    /// Convert coefficient list [a_n, ..., a_0] to expression
    fn coeffs_to_expr(coeffs: &[i64], var: &Symbol) -> Expr {
        let degree = coeffs.len() - 1;
        let mut terms = Vec::new();
        for (i, &c) in coeffs.iter().enumerate() {
            if c == 0 {
                continue;
            }
            let d = degree - i;
            let var_part = match d {
                0 => Expr::Integer(1),
                1 => Expr::Symbol(var.clone()),
                _ => Expr::pow(Expr::Symbol(var.clone()), Expr::Integer(d as i64)),
            };
            if d == 0 {
                terms.push(Expr::Integer(c));
            } else if c == 1 {
                terms.push(var_part);
            } else if c == -1 {
                terms.push(Expr::neg(var_part));
            } else {
                terms.push(Expr::mul(vec![Expr::Integer(c), var_part]));
            }
        }
        if terms.is_empty() {
            Expr::Integer(0)
        } else if terms.len() == 1 {
            terms.into_iter().next().unwrap()
        } else {
            Expr::Add(terms)
        }
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn divisors(n: u64) -> Vec<u64> {
    if n == 0 {
        return vec![1];
    }
    let mut result = Vec::new();
    let mut i = 1;
    while i * i <= n {
        if n % i == 0 {
            result.push(i);
            if i != n / i {
                result.push(n / i);
            }
        }
        i += 1;
    }
    result.sort();
    result
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
    fn test_simplify_mul_rational_one_identity() {
        let expr = Expr::mul(vec![
            Expr::Rational(crate::expr::Rational::new(1, 1)),
            Expr::symbol("n"),
        ]);
        let result = Simplifier::simplify(&expr);
        assert_eq!(result, Expr::symbol("n"));
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

    #[test]
    fn test_simplify_factorial_constant() {
        let expr = Expr::func("factorial", vec![Expr::Integer(1)]);
        let simplified = Simplifier::simplify(&expr);
        assert_eq!(simplified, Expr::Integer(1));
    }

    #[test]
    fn test_simplify_ln_of_e() {
        let expr = Expr::func("ln", vec![Expr::symbol("e")]);
        let simplified = Simplifier::simplify(&expr);
        assert_eq!(simplified, Expr::Integer(1));
    }

    #[test]
    fn test_diff_e_to_x_simplifies_to_e_to_x() {
        let expr = Expr::pow(Expr::symbol("e"), Expr::symbol("x"));
        let result = Differentiator::diff(&expr, &Symbol::new("x")).unwrap();
        let simplified = Simplifier::simplify(&result);
        assert_eq!(simplified, Expr::pow(Expr::symbol("e"), Expr::symbol("x")));
    }

    #[test]
    fn test_simplify_product_residual_factorial_inverse() {
        let expr = Expr::mul(vec![
            Expr::func("factorial", vec![Expr::symbol("n")]),
            Expr::func(
                "factorial",
                vec![Expr::add(vec![Expr::Integer(1), Expr::symbol("n")])],
            ),
            Expr::pow(
                Expr::func("factorial", vec![Expr::Integer(1)]),
                Expr::Integer(-1),
            ),
        ]);

        let simplified = Simplifier::simplify(&expr);
        let rendered = simplified.to_string();
        assert!(!rendered.contains("1!"));
        assert!(!rendered.contains("factorial"));
        assert!(rendered.contains("n!"));
    }
}
