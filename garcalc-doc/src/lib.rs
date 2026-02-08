//! garcalc-doc: Document format
//!
//! Provides document structure for calculator pages, graphs,
//! geometry, spreadsheets, and notes.
//!
//! This is a stub for Sprint 6-7 implementation.

use std::collections::HashMap;

use garcalc_cas::Expr;
use serde::{Deserialize, Serialize};

/// A garcalc document (similar to TI-Nspire .tns)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub version: u32,
    pub metadata: DocumentMetadata,
    pub pages: Vec<Page>,
    pub variables: HashMap<String, Expr>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            version: 1,
            metadata: DocumentMetadata::default(),
            pages: vec![Page::Calculator(CalculatorPage::default())],
            variables: HashMap::new(),
        }
    }
}

/// Document metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub title: String,
    pub created: Option<String>,
    pub modified: Option<String>,
}

/// A page within a document
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Page {
    Calculator(CalculatorPage),
    Graph(GraphPage),
    Geometry(GeometryPage),
    Spreadsheet(SpreadsheetPage),
    Notes(NotesPage),
}

/// Calculator page with expression history
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CalculatorPage {
    pub entries: Vec<CalculatorEntry>,
}

/// A single calculation entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatorEntry {
    pub input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Graph page (stub)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphPage {
    pub functions: Vec<String>,
}

/// Geometry page (stub)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeometryPage {
    pub shapes: Vec<String>,
}

/// Spreadsheet page
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpreadsheetPage {
    pub cells: HashMap<String, Cell>,
    pub rows: usize,
    pub cols: usize,
}

/// Spreadsheet cell
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub formula: Option<String>,
    pub value: Option<String>,
}

/// Notes page (stub)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NotesPage {
    pub content: String,
}
