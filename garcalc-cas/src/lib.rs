//! garcalc-cas: Computer Algebra System for garcalc
//!
//! A custom CAS engine providing:
//! - Expression parsing and representation
//! - Numeric and symbolic evaluation
//! - Symbolic differentiation and integration
//! - Equation solving
//! - Limits and series expansions

pub mod expr;
pub mod parser;
pub mod eval;
pub mod error;

pub use expr::{Expr, Symbol, Rational};
pub use parser::Parser;
pub use eval::Evaluator;
pub use error::{CasError, Result};
