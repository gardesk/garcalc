//! Expression parser using recursive descent
//!
//! Grammar:
//! ```text
//! expr       = equation
//! equation   = additive ("=" additive)?
//! additive   = multiplicative (("+"|"-") multiplicative)*
//! multiplicative = power (("*"|"/") power)*
//! power      = unary ("^" power)?
//! unary      = "-" unary | postfix
//! postfix    = primary ("!")*
//! primary    = NUMBER | SYMBOL | "(" expr ")" | function | "[" matrix "]"
//! function   = SYMBOL "(" args? ")"
//! args       = expr ("," expr)*
//! matrix     = row (";" row)*
//! row        = expr ("," expr)*
//! ```

use crate::error::{CasError, Result};
use crate::expr::{Expr, Symbol};

/// Token types
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    Symbol(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Bang,
    Eq,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Eof,
}

/// Tokenizer for mathematical expressions
pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn read_number(&mut self) -> Token {
        let start = self.pos;
        let mut has_dot = false;
        let mut has_exp = false;

        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() {
                self.advance();
            } else if c == '.' && !has_dot && !has_exp {
                has_dot = true;
                self.advance();
            } else if (c == 'e' || c == 'E') && !has_exp {
                has_exp = true;
                self.advance();
                // Handle optional sign after exponent
                if let Some(sign) = self.peek_char() {
                    if sign == '+' || sign == '-' {
                        self.advance();
                    }
                }
            } else {
                break;
            }
        }

        let num_str = &self.input[start..self.pos];
        let value: f64 = num_str.parse().unwrap_or(f64::NAN);
        Token::Number(value)
    }

    fn read_symbol(&mut self) -> Token {
        let start = self.pos;

        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }

        let name = self.input[start..self.pos].to_string();
        Token::Symbol(name)
    }

    pub fn next_token(&mut self) -> Result<Token> {
        self.skip_whitespace();

        let Some(c) = self.peek_char() else {
            return Ok(Token::Eof);
        };

        let token = match c {
            '+' => {
                self.advance();
                Token::Plus
            }
            '-' => {
                self.advance();
                Token::Minus
            }
            '*' => {
                self.advance();
                Token::Star
            }
            '/' => {
                self.advance();
                Token::Slash
            }
            '^' => {
                self.advance();
                Token::Caret
            }
            '!' => {
                self.advance();
                Token::Bang
            }
            '=' => {
                self.advance();
                Token::Eq
            }
            '(' => {
                self.advance();
                Token::LParen
            }
            ')' => {
                self.advance();
                Token::RParen
            }
            '[' => {
                self.advance();
                Token::LBracket
            }
            ']' => {
                self.advance();
                Token::RBracket
            }
            ',' => {
                self.advance();
                Token::Comma
            }
            ';' => {
                self.advance();
                Token::Semicolon
            }
            _ if c.is_ascii_digit() || c == '.' => self.read_number(),
            _ if c.is_alphabetic() || c == '_' => self.read_symbol(),
            _ => {
                return Err(CasError::Parse {
                    position: self.pos,
                    message: format!("unexpected character: '{c}'"),
                });
            }
        };

        Ok(token)
    }

    pub fn position(&self) -> usize {
        self.pos
    }
}

/// Recursive descent parser
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Result<Self> {
        let mut lexer = Lexer::new(input);
        let current = lexer.next_token()?;
        Ok(Self { lexer, current })
    }

    fn advance(&mut self) -> Result<Token> {
        let prev = std::mem::replace(&mut self.current, self.lexer.next_token()?);
        Ok(prev)
    }

    fn expect(&mut self, expected: Token) -> Result<()> {
        if self.current == expected {
            self.advance()?;
            Ok(())
        } else {
            Err(CasError::Parse {
                position: self.lexer.position(),
                message: format!("expected {expected:?}, found {:?}", self.current),
            })
        }
    }

    /// Parse a complete expression
    pub fn parse(&mut self) -> Result<Expr> {
        let expr = self.parse_equation()?;
        if self.current != Token::Eof {
            return Err(CasError::Parse {
                position: self.lexer.position(),
                message: format!("unexpected token: {:?}", self.current),
            });
        }
        Ok(expr)
    }

    fn parse_equation(&mut self) -> Result<Expr> {
        let lhs = self.parse_additive()?;
        if self.current == Token::Eq {
            self.advance()?;
            let rhs = self.parse_additive()?;
            Ok(Expr::Equation(Box::new(lhs), Box::new(rhs)))
        } else {
            Ok(lhs)
        }
    }

    fn parse_additive(&mut self) -> Result<Expr> {
        let mut terms = vec![self.parse_multiplicative()?];

        loop {
            match &self.current {
                Token::Plus => {
                    self.advance()?;
                    terms.push(self.parse_multiplicative()?);
                }
                Token::Minus => {
                    self.advance()?;
                    terms.push(Expr::neg(self.parse_multiplicative()?));
                }
                _ => break,
            }
        }

        Ok(Expr::add(terms))
    }

    fn parse_multiplicative(&mut self) -> Result<Expr> {
        let mut factors = vec![self.parse_power()?];

        loop {
            match &self.current {
                Token::Star => {
                    self.advance()?;
                    factors.push(self.parse_power()?);
                }
                Token::Slash => {
                    self.advance()?;
                    let divisor = self.parse_power()?;
                    factors.push(Expr::pow(divisor, Expr::integer(-1)));
                }
                // Implicit multiplication: 2x, xy, x(y+1)
                Token::Symbol(_) | Token::LParen | Token::Number(_) => {
                    // Check if this could be implicit multiplication
                    if let Some(last) = factors.last() {
                        if matches!(
                            last,
                            Expr::Symbol(_)
                                | Expr::Integer(_)
                                | Expr::Float(_)
                                | Expr::Func(_, _)
                        ) {
                            factors.push(self.parse_power()?);
                            continue;
                        }
                    }
                    break;
                }
                _ => break,
            }
        }

        Ok(Expr::mul(factors))
    }

    fn parse_power(&mut self) -> Result<Expr> {
        let base = self.parse_unary()?;
        if self.current == Token::Caret {
            self.advance()?;
            let exp = self.parse_power()?; // Right associative
            Ok(Expr::pow(base, exp))
        } else {
            Ok(base)
        }
    }

    fn parse_unary(&mut self) -> Result<Expr> {
        if self.current == Token::Minus {
            self.advance()?;
            let expr = self.parse_unary()?;
            Ok(Expr::neg(expr))
        } else {
            self.parse_postfix()
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary()?;

        while self.current == Token::Bang {
            self.advance()?;
            expr = Expr::func("factorial", vec![expr]);
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        match &self.current {
            Token::Number(n) => {
                let n = *n;
                self.advance()?;
                // Convert to integer if possible
                if n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
                    Ok(Expr::integer(n as i64))
                } else {
                    Ok(Expr::float(n))
                }
            }
            Token::Symbol(name) => {
                let name = name.clone();
                self.advance()?;

                // Check for function call
                if self.current == Token::LParen {
                    self.advance()?;
                    let args = self.parse_args()?;
                    self.expect(Token::RParen)?;
                    Ok(self.handle_special_function(&name, args))
                } else {
                    // Constants
                    match name.as_str() {
                        "pi" | "PI" => Ok(Expr::symbol("pi")),
                        "e" | "E" => Ok(Expr::symbol("e")),
                        "i" => Ok(Expr::Complex(0.0, 1.0)),
                        "inf" | "infinity" => Ok(Expr::Infinity(crate::expr::Sign::Positive)),
                        _ => Ok(Expr::symbol(name)),
                    }
                }
            }
            Token::LParen => {
                self.advance()?;
                let expr = self.parse_equation()?;
                self.expect(Token::RParen)?;
                Ok(expr)
            }
            Token::LBracket => {
                self.advance()?;
                let matrix = self.parse_matrix()?;
                self.expect(Token::RBracket)?;
                Ok(matrix)
            }
            _ => Err(CasError::Parse {
                position: self.lexer.position(),
                message: format!("unexpected token: {:?}", self.current),
            }),
        }
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>> {
        if self.current == Token::RParen {
            return Ok(vec![]);
        }

        let mut args = vec![self.parse_equation()?];

        while self.current == Token::Comma {
            self.advance()?;
            args.push(self.parse_equation()?);
        }

        Ok(args)
    }

    fn parse_matrix(&mut self) -> Result<Expr> {
        let mut rows = vec![self.parse_row()?];

        while self.current == Token::Semicolon {
            self.advance()?;
            rows.push(self.parse_row()?);
        }

        // Single row = vector, multiple rows = matrix
        if rows.len() == 1 {
            Ok(Expr::Vector(rows.into_iter().next().unwrap()))
        } else {
            Ok(Expr::Matrix(rows))
        }
    }

    fn parse_row(&mut self) -> Result<Vec<Expr>> {
        let mut elems = vec![self.parse_equation()?];

        while self.current == Token::Comma {
            self.advance()?;
            elems.push(self.parse_equation()?);
        }

        Ok(elems)
    }

    /// Handle special functions that need custom AST nodes
    fn handle_special_function(&self, name: &str, mut args: Vec<Expr>) -> Expr {
        match name {
            "diff" | "derivative" => {
                if args.len() >= 2 {
                    let expr = args.remove(0);
                    let var = match args.remove(0) {
                        Expr::Symbol(s) => s,
                        _ => Symbol::new("x"),
                    };
                    let order = if args.is_empty() {
                        1
                    } else {
                        match &args[0] {
                            Expr::Integer(n) => *n as u32,
                            _ => 1,
                        }
                    };
                    Expr::Derivative {
                        expr: Box::new(expr),
                        var,
                        order,
                    }
                } else if args.len() == 1 {
                    Expr::Derivative {
                        expr: Box::new(args.remove(0)),
                        var: Symbol::new("x"),
                        order: 1,
                    }
                } else {
                    Expr::func(name, args)
                }
            }
            "integrate" | "int" => {
                if args.len() >= 2 {
                    let expr = args.remove(0);
                    let var = match args.remove(0) {
                        Expr::Symbol(s) => s,
                        _ => Symbol::new("x"),
                    };
                    let (lower, upper) = if args.len() >= 2 {
                        (Some(Box::new(args.remove(0))), Some(Box::new(args.remove(0))))
                    } else {
                        (None, None)
                    };
                    Expr::Integral {
                        expr: Box::new(expr),
                        var,
                        lower,
                        upper,
                    }
                } else {
                    Expr::func(name, args)
                }
            }
            "lim" | "limit" => {
                if args.len() >= 3 {
                    let expr = args.remove(0);
                    let var = match args.remove(0) {
                        Expr::Symbol(s) => s,
                        _ => Symbol::new("x"),
                    };
                    let point = args.remove(0);
                    Expr::Limit {
                        expr: Box::new(expr),
                        var,
                        point: Box::new(point),
                        direction: None,
                    }
                } else {
                    Expr::func(name, args)
                }
            }
            "sum" => {
                if args.len() >= 4 {
                    let expr = args.remove(0);
                    let var = match args.remove(0) {
                        Expr::Symbol(s) => s,
                        _ => Symbol::new("n"),
                    };
                    let lower = args.remove(0);
                    let upper = args.remove(0);
                    Expr::Sum {
                        expr: Box::new(expr),
                        var,
                        lower: Box::new(lower),
                        upper: Box::new(upper),
                    }
                } else {
                    Expr::func(name, args)
                }
            }
            "product" | "prod" => {
                if args.len() >= 4 {
                    let expr = args.remove(0);
                    let var = match args.remove(0) {
                        Expr::Symbol(s) => s,
                        _ => Symbol::new("n"),
                    };
                    let lower = args.remove(0);
                    let upper = args.remove(0);
                    Expr::Product {
                        expr: Box::new(expr),
                        var,
                        lower: Box::new(lower),
                        upper: Box::new(upper),
                    }
                } else {
                    Expr::func(name, args)
                }
            }
            _ => Expr::func(name, args),
        }
    }
}

/// Parse a string into an expression
pub fn parse(input: &str) -> Result<Expr> {
    Parser::new(input)?.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number() {
        let expr = parse("42").unwrap();
        assert_eq!(expr, Expr::integer(42));
    }

    #[test]
    fn test_float() {
        let expr = parse("3.14").unwrap();
        assert_eq!(expr, Expr::float(3.14));
    }

    #[test]
    fn test_addition() {
        let expr = parse("1 + 2 + 3").unwrap();
        assert_eq!(
            expr,
            Expr::add(vec![Expr::integer(1), Expr::integer(2), Expr::integer(3)])
        );
    }

    #[test]
    fn test_subtraction() {
        let expr = parse("5 - 3").unwrap();
        assert_eq!(
            expr,
            Expr::add(vec![Expr::integer(5), Expr::neg(Expr::integer(3))])
        );
    }

    #[test]
    fn test_multiplication() {
        let expr = parse("2 * 3").unwrap();
        assert_eq!(expr, Expr::mul(vec![Expr::integer(2), Expr::integer(3)]));
    }

    #[test]
    fn test_division() {
        let expr = parse("6 / 2").unwrap();
        assert_eq!(
            expr,
            Expr::mul(vec![
                Expr::integer(6),
                Expr::pow(Expr::integer(2), Expr::integer(-1))
            ])
        );
    }

    #[test]
    fn test_power() {
        let expr = parse("2^3").unwrap();
        assert_eq!(expr, Expr::pow(Expr::integer(2), Expr::integer(3)));
    }

    #[test]
    fn test_implicit_multiplication() {
        let expr = parse("2x").unwrap();
        assert_eq!(expr, Expr::mul(vec![Expr::integer(2), Expr::symbol("x")]));
    }

    #[test]
    fn test_function() {
        let expr = parse("sin(x)").unwrap();
        assert_eq!(expr, Expr::func("sin", vec![Expr::symbol("x")]));
    }

    #[test]
    fn test_nested() {
        let expr = parse("sin(x + 1)").unwrap();
        assert_eq!(
            expr,
            Expr::func(
                "sin",
                vec![Expr::add(vec![Expr::symbol("x"), Expr::integer(1)])]
            )
        );
    }

    #[test]
    fn test_equation() {
        let expr = parse("x + 1 = 5").unwrap();
        assert!(matches!(expr, Expr::Equation(_, _)));
    }

    #[test]
    fn test_derivative() {
        let expr = parse("diff(x^2, x)").unwrap();
        assert!(matches!(expr, Expr::Derivative { .. }));
    }

    #[test]
    fn test_integral() {
        let expr = parse("integrate(x^2, x)").unwrap();
        assert!(matches!(expr, Expr::Integral { lower: None, .. }));
    }

    #[test]
    fn test_definite_integral() {
        let expr = parse("integrate(x^2, x, 0, 1)").unwrap();
        assert!(matches!(expr, Expr::Integral { lower: Some(_), .. }));
    }

    #[test]
    fn test_vector() {
        let expr = parse("[1, 2, 3]").unwrap();
        assert!(matches!(expr, Expr::Vector(_)));
    }

    #[test]
    fn test_matrix() {
        let expr = parse("[1, 2; 3, 4]").unwrap();
        assert!(matches!(expr, Expr::Matrix(_)));
    }

    #[test]
    fn test_factorial() {
        let expr = parse("5!").unwrap();
        assert_eq!(expr, Expr::func("factorial", vec![Expr::integer(5)]));
    }
}
