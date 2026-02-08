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
pub mod symbolic;

pub use expr::{Expr, Symbol, Rational, LimitDirection, Sign};
pub use parser::Parser;
pub use eval::Evaluator;
pub use error::{CasError, Result};
pub use symbolic::{Differentiator, Integrator, Limits, Simplifier, Solver};
