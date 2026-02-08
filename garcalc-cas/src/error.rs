use thiserror::Error;

/// CAS error types
#[derive(Debug, Error)]
pub enum CasError {
    #[error("Parse error at position {position}: {message}")]
    Parse { position: usize, message: String },

    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),

    #[error("Undefined function: {0}")]
    UndefinedFunction(String),

    #[error("Type error: {0}")]
    Type(String),

    #[error("Division by zero")]
    DivisionByZero,

    #[error("Domain error: {0}")]
    Domain(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Evaluation error: {0}")]
    EvaluationError(String),
}

pub type Result<T> = std::result::Result<T, CasError>;
