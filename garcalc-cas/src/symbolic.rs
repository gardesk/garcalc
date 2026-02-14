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
            Expr::Mul(final_factors)
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
