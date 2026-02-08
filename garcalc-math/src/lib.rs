//! Math rendering and structured input for garcalc
//!
//! Provides TI-Nspire-style math input with:
//! - Structured input templates (fractions, powers, radicals, integrals)
//! - Proper mathematical typesetting
//! - Keyboard navigation through expression tree
//! - Bidirectional conversion with CAS Expr type

pub mod mathbox;
pub mod layout;
pub mod render;
pub mod input;
pub mod convert;

pub use mathbox::{MathBox, Cursor};
pub use layout::{LayoutBox, MathLayoutEngine};
pub use render::MathRenderer;
pub use input::{MathInput, InputResult};
pub use convert::{to_expr, from_expr, ConvertError};
