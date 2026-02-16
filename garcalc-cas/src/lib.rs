//! garcalc-cas: Computer Algebra System for garcalc
//!
//! A custom CAS engine providing:
//! - Expression parsing and representation
//! - Numeric and symbolic evaluation
//! - Symbolic differentiation and integration
//! - Equation solving
//! - Limits and series expansions

pub mod error;
pub mod eval;
pub mod expr;
pub mod parser;
pub mod symbolic;

pub use error::{CasError, Result};
pub use eval::Evaluator;
pub use expr::{Expr, LimitDirection, Rational, Sign, Symbol};
pub use parser::Parser;
pub use symbolic::{Differentiator, Factorer, Integrator, Limits, Simplifier, Solver};
