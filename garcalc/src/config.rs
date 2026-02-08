//! Configuration loading

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Calculator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub popup: PopupConfig,
    pub graph: GraphConfig,
    pub appearance: AppearanceConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            popup: PopupConfig::default(),
            graph: GraphConfig::default(),
            appearance: AppearanceConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub default_mode: String,
    pub precision: u32,
    pub angle_mode: String,
    pub exact_mode: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            default_mode: "calculator".to_string(),
            precision: 15,
            angle_mode: "radians".to_string(),
            exact_mode: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PopupConfig {
    pub width: u32,
    pub height: u32,
    pub position: String,
}

impl Default for PopupConfig {
    fn default() -> Self {
        Self {
            width: 700,
            height: 500,
            position: "center".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphConfig {
    pub default_x_range: (f64, f64),
    pub default_y_range: (f64, f64),
    pub grid_enabled: bool,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            default_x_range: (-10.0, 10.0),
            default_y_range: (-10.0, 10.0),
            grid_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub font_family: String,
    pub font_size: u32,
    pub button_panel_visible: bool,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            font_family: "monospace".to_string(),
            font_size: 14,
            button_panel_visible: false,
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn load() -> Result<Self> {
        let path = config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("garcalc")
        .join("config.toml")
}
