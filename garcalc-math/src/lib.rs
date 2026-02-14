//! Math rendering and structured input for garcalc
//!
//! Provides TI-Nspire-style math input with:
//! - Structured input templates (fractions, powers, radicals, integrals)
//! - Proper mathematical typesetting
//! - Keyboard navigation through expression tree
//! - Bidirectional conversion with CAS Expr type

pub mod convert;
pub mod input;
pub mod layout;
pub mod mathbox;
pub mod render;

pub use convert::{ConvertError, from_expr, to_expr};
pub use input::{InputResult, MathInput};
pub use layout::{LayoutBox, MathLayoutEngine};
pub use mathbox::{Cursor, MathBox};
pub use render::MathRenderer;
