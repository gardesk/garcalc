//! Conversion between MathBox and CAS Expr types
//!
//! Enables bidirectional conversion:
//! - MathBox -> Expr for evaluation
//! - Expr -> MathBox for result display

use crate::mathbox::{MathBox, Operator, LimitDirection as MathLimitDirection};
use garcalc_cas::expr::{Expr, Rational, Symbol, LimitDirection, Sign};
use thiserror::Error;

/// Errors that can occur during conversion
#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("Empty slot in expression")]
    EmptySlot,
    #[error("Invalid number: {0}")]
    InvalidNumber(String),
    #[error("Unsupported expression type")]
    Unsupported,
    #[error("Missing required field: {0}")]
    MissingField(String),
}

/// Convert a MathBox to a CAS Expr
pub fn to_expr(mathbox: &MathBox) -> Result<Expr, ConvertError> {
    match mathbox {
        MathBox::Number(s) => parse_number(s),
        MathBox::Symbol(s) => Ok(Expr::Symbol(Symbol::new(s.clone()))),
        MathBox::Operator(_) => Err(ConvertError::Unsupported),
        MathBox::Slot => Err(ConvertError::EmptySlot),

        MathBox::Fraction { num, den } => {
            let num_expr = to_expr(num)?;
            let den_expr = to_expr(den)?;
            Ok(Expr::Mul(vec![
                num_expr,
                Expr::Pow(Box::new(den_expr), Box::new(Expr::Integer(-1))),
            ]))
        }

        MathBox::Power { base, exp } => {
            let base_expr = to_expr(base)?;
            let exp_expr = to_expr(exp)?;
            Ok(Expr::Pow(Box::new(base_expr), Box::new(exp_expr)))
        }

        MathBox::Subscript { base, sub } => {
            // Subscripted variables become a combined symbol
            if let (MathBox::Symbol(b), MathBox::Number(s) | MathBox::Symbol(s)) = (base.as_ref(), sub.as_ref()) {
                Ok(Expr::Symbol(Symbol::new(format!("{}_{}", b, s))))
            } else {
                let base_expr = to_expr(base)?;
                Ok(base_expr)
            }
        }

        MathBox::Root { index, radicand } => {
            let radicand_expr = to_expr(radicand)?;
            let exp = if let Some(idx) = index {
                let idx_expr = to_expr(idx)?;
                Expr::Mul(vec![
                    Expr::Integer(1),
                    Expr::Pow(Box::new(idx_expr), Box::new(Expr::Integer(-1))),
                ])
            } else {
                // Square root: 1/2
                Expr::Rational(Rational::new(1, 2))
            };
            Ok(Expr::Pow(Box::new(radicand_expr), Box::new(exp)))
        }

        MathBox::Func { name, args } => {
            let arg_exprs: Result<Vec<Expr>, ConvertError> = args.iter().map(to_expr).collect();
            Ok(Expr::Func(name.clone(), arg_exprs?))
        }

        MathBox::Abs(inner) => {
            let inner_expr = to_expr(inner)?;
            Ok(Expr::Func("abs".to_string(), vec![inner_expr]))
        }

        MathBox::Parens(inner) => to_expr(inner),

        MathBox::Integral { lower, upper, body, var } => {
            let body_expr = to_expr(body)?;
            let var_sym = Symbol::new(extract_symbol_str(var)?);

            let lower_expr = match lower {
                Some(lo) => Some(Box::new(to_expr(lo)?)),
                None => None,
            };
            let upper_expr = match upper {
                Some(hi) => Some(Box::new(to_expr(hi)?)),
                None => None,
            };

            Ok(Expr::Integral {
                expr: Box::new(body_expr),
                var: var_sym,
                lower: lower_expr,
                upper: upper_expr,
            })
        }

        MathBox::Derivative { order, var, body } => {
            let body_expr = to_expr(body)?;
            let var_sym = Symbol::new(extract_symbol_str(var)?);
            Ok(Expr::Derivative {
                expr: Box::new(body_expr),
                var: var_sym,
                order: *order,
            })
        }

        MathBox::Limit { var, to, direction, body } => {
            let body_expr = to_expr(body)?;
            let var_sym = Symbol::new(extract_symbol_str(var)?);
            let point_expr = to_expr(to)?;

            let dir = direction.map(|d| match d {
                MathLimitDirection::FromLeft => LimitDirection::Left,
                MathLimitDirection::FromRight => LimitDirection::Right,
            });

            Ok(Expr::Limit {
                expr: Box::new(body_expr),
                var: var_sym,
                point: Box::new(point_expr),
                direction: dir,
            })
        }

        MathBox::Sum { var, lower, upper, body } => {
            let var_sym = Symbol::new(extract_symbol_str(var)?);
            let lower_expr = to_expr(lower)?;
            let upper_expr = to_expr(upper)?;
            let body_expr = to_expr(body)?;

            Ok(Expr::Sum {
                expr: Box::new(body_expr),
                var: var_sym,
                lower: Box::new(lower_expr),
                upper: Box::new(upper_expr),
            })
        }

        MathBox::Product { var, lower, upper, body } => {
            let var_sym = Symbol::new(extract_symbol_str(var)?);
            let lower_expr = to_expr(lower)?;
            let upper_expr = to_expr(upper)?;
            let body_expr = to_expr(body)?;

            Ok(Expr::Product {
                expr: Box::new(body_expr),
                var: var_sym,
                lower: Box::new(lower_expr),
                upper: Box::new(upper_expr),
            })
        }

        MathBox::Matrix { rows } => {
            let expr_rows: Result<Vec<Vec<Expr>>, ConvertError> = rows
                .iter()
                .map(|row| row.iter().map(to_expr).collect())
                .collect();
            Ok(Expr::Matrix(expr_rows?))
        }

        MathBox::Row(items) => {
            // Parse a row as an expression (needs operator precedence)
            convert_row(items)
        }
    }
}

/// Convert a CAS Expr to a MathBox for display
pub fn from_expr(expr: &Expr) -> MathBox {
    match expr {
        Expr::Integer(n) => MathBox::Number(n.to_string()),

        Expr::Rational(r) => {
            if r.den == 1 {
                MathBox::Number(r.num.to_string())
            } else {
                MathBox::Fraction {
                    num: Box::new(MathBox::Number(r.num.to_string())),
                    den: Box::new(MathBox::Number(r.den.to_string())),
                }
            }
        }

        Expr::Float(f) => MathBox::Number(format_float(*f)),

        Expr::Complex(re, im) => {
            if *re == 0.0 {
                MathBox::Row(vec![
                    MathBox::Number(format_float(*im)),
                    MathBox::Symbol("i".to_string()),
                ])
            } else if *im >= 0.0 {
                MathBox::Row(vec![
                    MathBox::Number(format_float(*re)),
                    MathBox::Operator(Operator::Add),
                    MathBox::Number(format_float(*im)),
                    MathBox::Symbol("i".to_string()),
                ])
            } else {
                MathBox::Row(vec![
                    MathBox::Number(format_float(*re)),
                    MathBox::Operator(Operator::Sub),
                    MathBox::Number(format_float(im.abs())),
                    MathBox::Symbol("i".to_string()),
                ])
            }
        }

        Expr::Symbol(s) => {
            // Convert common symbols to proper display
            match s.as_str() {
                "pi" => MathBox::Symbol("π".to_string()),
                "e" => MathBox::Symbol("e".to_string()),
                "inf" | "infinity" => MathBox::Symbol("∞".to_string()),
                _ => MathBox::Symbol(s.0.clone()),
            }
        }

        Expr::Add(terms) => {
            let mut items = Vec::new();
            for (i, term) in terms.iter().enumerate() {
                if i > 0 {
                    // Check if term is negative
                    if is_negative(term) {
                        items.push(MathBox::Operator(Operator::Sub));
                        items.push(from_expr(&negate(term)));
                    } else {
                        items.push(MathBox::Operator(Operator::Add));
                        items.push(from_expr(term));
                    }
                } else {
                    items.push(from_expr(term));
                }
            }
            if items.len() == 1 {
                items.pop().unwrap()
            } else {
                MathBox::Row(items)
            }
        }

        Expr::Mul(factors) => {
            // Check for division pattern (x * y^-1)
            let (numerator, denominator): (Vec<_>, Vec<_>) = factors.iter().partition(|f| !is_reciprocal(f));

            if !denominator.is_empty() {
                let num_box = if numerator.is_empty() {
                    MathBox::Number("1".to_string())
                } else if numerator.len() == 1 {
                    from_expr(numerator[0])
                } else {
                    from_expr(&Expr::Mul(numerator.into_iter().cloned().collect()))
                };

                let den_factors: Vec<Expr> = denominator.iter().map(|f| extract_base_of_reciprocal(f)).collect();
                let den_box = if den_factors.len() == 1 {
                    from_expr(&den_factors[0])
                } else {
                    from_expr(&Expr::Mul(den_factors))
                };

                return MathBox::Fraction {
                    num: Box::new(num_box),
                    den: Box::new(den_box),
                };
            }

            // Normal multiplication
            let mut items = Vec::new();
            for (i, factor) in factors.iter().enumerate() {
                if i > 0 {
                    items.push(MathBox::Operator(Operator::Mul));
                }
                items.push(from_expr(factor));
            }
            if items.len() == 1 {
                items.pop().unwrap()
            } else {
                MathBox::Row(items)
            }
        }

        Expr::Pow(base, exp) => {
            // Check for square root / nth root
            if let Expr::Rational(r) = exp.as_ref() {
                if r.num == 1 && r.den == 2 {
                    return MathBox::Root {
                        index: None,
                        radicand: Box::new(from_expr(base)),
                    };
                } else if r.num == 1 && r.den > 2 {
                    return MathBox::Root {
                        index: Some(Box::new(MathBox::Number(r.den.to_string()))),
                        radicand: Box::new(from_expr(base)),
                    };
                }
            }

            MathBox::Power {
                base: Box::new(from_expr(base)),
                exp: Box::new(from_expr(exp)),
            }
        }

        Expr::Neg(inner) => {
            MathBox::Row(vec![
                MathBox::Operator(Operator::Sub),
                from_expr(inner),
            ])
        }

        Expr::Func(name, args) => {
            let arg_boxes: Vec<MathBox> = args.iter().map(from_expr).collect();

            // Special functions get special rendering
            match name.as_str() {
                "abs" if args.len() == 1 => {
                    MathBox::Abs(Box::new(arg_boxes.into_iter().next().unwrap()))
                }
                _ => MathBox::Func {
                    name: name.clone(),
                    args: arg_boxes,
                },
            }
        }

        Expr::Derivative { expr, var, order } => {
            MathBox::Derivative {
                order: *order,
                var: Box::new(MathBox::Symbol(var.0.clone())),
                body: Box::new(from_expr(expr)),
            }
        }

        Expr::Integral { expr, var, lower, upper } => {
            MathBox::Integral {
                lower: lower.as_ref().map(|lo| Box::new(from_expr(lo))),
                upper: upper.as_ref().map(|hi| Box::new(from_expr(hi))),
                body: Box::new(from_expr(expr)),
                var: Box::new(MathBox::Symbol(var.0.clone())),
            }
        }

        Expr::Limit { expr, var, point, direction } => {
            MathBox::Limit {
                var: Box::new(MathBox::Symbol(var.0.clone())),
                to: Box::new(from_expr(point)),
                direction: direction.map(|d| match d {
                    LimitDirection::Left => MathLimitDirection::FromLeft,
                    LimitDirection::Right => MathLimitDirection::FromRight,
                }),
                body: Box::new(from_expr(expr)),
            }
        }

        Expr::Sum { expr, var, lower, upper } => {
            MathBox::Sum {
                var: Box::new(MathBox::Symbol(var.0.clone())),
                lower: Box::new(from_expr(lower)),
                upper: Box::new(from_expr(upper)),
                body: Box::new(from_expr(expr)),
            }
        }

        Expr::Product { expr, var, lower, upper } => {
            MathBox::Product {
                var: Box::new(MathBox::Symbol(var.0.clone())),
                lower: Box::new(from_expr(lower)),
                upper: Box::new(from_expr(upper)),
                body: Box::new(from_expr(expr)),
            }
        }

        Expr::Matrix(rows) => {
            let box_rows: Vec<Vec<MathBox>> = rows
                .iter()
                .map(|row| row.iter().map(from_expr).collect())
                .collect();
            MathBox::Matrix { rows: box_rows }
        }

        Expr::Equation(lhs, rhs) => {
            MathBox::Row(vec![
                from_expr(lhs),
                MathBox::Operator(Operator::Eq),
                from_expr(rhs),
            ])
        }

        Expr::Undefined => MathBox::Symbol("undefined".to_string()),
        Expr::Infinity(sign) => {
            match sign {
                Sign::Positive => MathBox::Symbol("∞".to_string()),
                Sign::Negative => MathBox::Row(vec![
                    MathBox::Operator(Operator::Sub),
                    MathBox::Symbol("∞".to_string()),
                ]),
            }
        }

        Expr::Vector(elems) => {
            // Display vector as a row matrix
            let box_row: Vec<MathBox> = elems.iter().map(from_expr).collect();
            MathBox::Matrix { rows: vec![box_row] }
        }

        Expr::Inequality { lhs, op, rhs } => {
            use garcalc_cas::expr::InequalityOp;
            let op_box = match op {
                InequalityOp::Lt => MathBox::Operator(Operator::Lt),
                InequalityOp::Le => MathBox::Operator(Operator::Le),
                InequalityOp::Gt => MathBox::Operator(Operator::Gt),
                InequalityOp::Ge => MathBox::Operator(Operator::Ge),
                InequalityOp::Ne => MathBox::Operator(Operator::Ne),
            };
            MathBox::Row(vec![
                from_expr(lhs),
                op_box,
                from_expr(rhs),
            ])
        }
    }
}

// Helper functions

fn parse_number(s: &str) -> Result<Expr, ConvertError> {
    if s.contains('.') {
        s.parse::<f64>()
            .map(Expr::Float)
            .map_err(|_| ConvertError::InvalidNumber(s.to_string()))
    } else {
        s.parse::<i64>()
            .map(Expr::Integer)
            .map_err(|_| ConvertError::InvalidNumber(s.to_string()))
    }
}

fn extract_symbol_str(mathbox: &MathBox) -> Result<String, ConvertError> {
    match mathbox {
        MathBox::Symbol(s) => Ok(s.clone()),
        MathBox::Slot => Err(ConvertError::EmptySlot),
        _ => Err(ConvertError::MissingField("variable".to_string())),
    }
}

fn convert_row(items: &[MathBox]) -> Result<Expr, ConvertError> {
    if items.is_empty() {
        return Err(ConvertError::EmptySlot);
    }
    if items.len() == 1 {
        return to_expr(&items[0]);
    }

    // Simple parsing: collect operands and operators
    let mut operands: Vec<Expr> = Vec::new();
    let mut operators: Vec<Operator> = Vec::new();

    let mut i = 0;
    while i < items.len() {
        match &items[i] {
            MathBox::Operator(op) => {
                operators.push(*op);
            }
            other => {
                operands.push(to_expr(other)?);
            }
        }
        i += 1;
    }

    // Simple left-to-right evaluation (no precedence for now)
    if operands.is_empty() {
        return Err(ConvertError::EmptySlot);
    }

    let mut result = operands[0].clone();
    for (i, op) in operators.iter().enumerate() {
        if i + 1 < operands.len() {
            let rhs = operands[i + 1].clone();
            result = match op {
                Operator::Add => Expr::Add(vec![result, rhs]),
                Operator::Sub => Expr::Add(vec![result, Expr::Neg(Box::new(rhs))]),
                Operator::Mul => Expr::Mul(vec![result, rhs]),
                Operator::Div => Expr::Mul(vec![
                    result,
                    Expr::Pow(Box::new(rhs), Box::new(Expr::Integer(-1))),
                ]),
                Operator::Eq => Expr::Equation(Box::new(result), Box::new(rhs)),
                _ => result, // Ignore comparison operators for now
            };
        }
    }

    Ok(result)
}

fn format_float(f: f64) -> String {
    if f == f.trunc() && f.abs() < 1e15 {
        format!("{:.0}", f)
    } else {
        format!("{:.10}", f).trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn is_negative(expr: &Expr) -> bool {
    match expr {
        Expr::Integer(n) => *n < 0,
        Expr::Float(f) => *f < 0.0,
        Expr::Neg(_) => true,
        Expr::Mul(factors) => factors.first().map(is_negative).unwrap_or(false),
        _ => false,
    }
}

fn negate(expr: &Expr) -> Expr {
    match expr {
        Expr::Integer(n) => Expr::Integer(-n),
        Expr::Float(f) => Expr::Float(-f),
        Expr::Neg(inner) => inner.as_ref().clone(),
        _ => Expr::Neg(Box::new(expr.clone())),
    }
}

fn is_reciprocal(expr: &Expr) -> bool {
    matches!(expr, Expr::Pow(_, exp) if matches!(exp.as_ref(), Expr::Integer(-1) | Expr::Neg(_)))
}

fn extract_base_of_reciprocal(expr: &Expr) -> Expr {
    if let Expr::Pow(base, _) = expr {
        base.as_ref().clone()
    } else {
        expr.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number_conversion() {
        let mb = MathBox::Number("42".to_string());
        let expr = to_expr(&mb).unwrap();
        assert!(matches!(expr, Expr::Integer(42)));

        let back = from_expr(&expr);
        assert!(matches!(back, MathBox::Number(s) if s == "42"));
    }

    #[test]
    fn test_fraction_conversion() {
        let mb = MathBox::Fraction {
            num: Box::new(MathBox::Number("1".to_string())),
            den: Box::new(MathBox::Number("2".to_string())),
        };

        let expr = to_expr(&mb).unwrap();
        // Should be 1 * 2^-1
        assert!(matches!(expr, Expr::Mul(_)));
    }

    #[test]
    fn test_rational_to_fraction() {
        let expr = Expr::Rational(Rational::new(3, 4));
        let mb = from_expr(&expr);

        assert!(matches!(mb, MathBox::Fraction { .. }));
    }
}
