use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// IPC command sent to garcalc daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    /// Show the calculator window
    Show,
    /// Hide the calculator window
    Hide,
    /// Toggle window visibility
    Toggle,
    /// Evaluate an expression and return the result
    Evaluate { expr: String },
    /// Get current calculator mode
    GetMode,
    /// Set calculator mode
    SetMode { mode: Mode },
    /// Open a document file
    OpenDocument { path: PathBuf },
    /// Get daemon status
    Status,
    /// Quit the daemon
    Quit,
}

/// Calculator operating modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Calculator,
    Graph,
    Graph3D,
    Geometry,
    Spreadsheet,
    Notes,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Calculator
    }
}

/// Response from garcalc daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ResponseData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok() -> Self {
        Self {
            success: true,
            data: None,
            error: None,
        }
    }

    pub fn ok_with_data(data: ResponseData) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

/// Response data variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseData {
    /// Result of expression evaluation
    Evaluation {
        input: String,
        result: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        exact: Option<String>,
    },
    /// Current mode
    Mode { mode: Mode },
    /// Daemon status
    Status {
        visible: bool,
        mode: Mode,
        #[serde(skip_serializing_if = "Option::is_none")]
        document: Option<String>,
        history_count: usize,
    },
}

/// Get the IPC socket path
pub fn socket_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("garcalc.sock")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_serialization() {
        let cmd = Command::Evaluate {
            expr: "2+2".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("evaluate"));
        assert!(json.contains("2+2"));
    }

    #[test]
    fn test_response_serialization() {
        let resp = Response::ok_with_data(ResponseData::Evaluation {
            input: "2+2".to_string(),
            result: "4".to_string(),
            exact: None,
        });
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":true"));
    }
}
