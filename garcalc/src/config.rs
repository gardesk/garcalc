//! Configuration loading
//!
//! Supports:
//! - Lua configuration via ~/.config/gar/init.lua (gar.calculator table)
//! - TOML fallback via ~/.config/garcalc/config.toml

use anyhow::Result;
use mlua::Lua;
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
    /// Load configuration
    /// Tries Lua config first (~/.config/gar/init.lua), then falls back to TOML
    pub fn load() -> Result<Self> {
        // Try Lua config first (shared with other gar components)
        if let Ok(config) = Self::load_from_lua() {
            return Ok(config);
        }

        // Fall back to TOML config
        let path = toml_config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Load configuration from Lua (~/.config/gar/init.lua)
    fn load_from_lua() -> Result<Self> {
        let lua_path = lua_config_path();
        if !lua_path.exists() {
            return Err(anyhow::anyhow!("Lua config not found"));
        }

        let lua = Lua::new();
        let content = std::fs::read_to_string(&lua_path)?;

        // Create gar table if it doesn't exist
        lua.scope(|_scope| {
            let globals = lua.globals();

            // Initialize gar table
            let gar: mlua::Table = lua.create_table()?;
            globals.set("gar", gar)?;

            // Execute the config file
            lua.load(&content).exec()?;

            // Get gar.calculator table
            let gar: mlua::Table = globals.get("gar")?;
            let calc: Option<mlua::Table> = gar.get("calculator").ok();

            if let Some(calc) = calc {
                let config = Self::from_lua_table(&calc)?;
                Ok(config)
            } else {
                Err(mlua::Error::RuntimeError("gar.calculator not found".to_string()))
            }
        }).map_err(|e| anyhow::anyhow!("Lua error: {}", e))
    }

    /// Parse Config from Lua table
    fn from_lua_table(table: &mlua::Table) -> mlua::Result<Self> {
        let mut config = Config::default();

        // General settings
        if let Ok(general) = table.get::<mlua::Table>("general") {
            if let Ok(mode) = general.get::<String>("default_mode") {
                config.general.default_mode = mode;
            }
            if let Ok(precision) = general.get::<u32>("precision") {
                config.general.precision = precision;
            }
            if let Ok(angle) = general.get::<String>("angle_mode") {
                config.general.angle_mode = angle;
            }
            if let Ok(exact) = general.get::<bool>("exact_mode") {
                config.general.exact_mode = exact;
            }
        }

        // Popup settings
        if let Ok(popup) = table.get::<mlua::Table>("popup") {
            if let Ok(w) = popup.get::<u32>("width") {
                config.popup.width = w;
            }
            if let Ok(h) = popup.get::<u32>("height") {
                config.popup.height = h;
            }
            if let Ok(pos) = popup.get::<String>("position") {
                config.popup.position = pos;
            }
        }

        // Graph settings
        if let Ok(graph) = table.get::<mlua::Table>("graph") {
            if let Ok(x_range) = graph.get::<mlua::Table>("x_range") {
                if let (Ok(min), Ok(max)) = (x_range.get::<f64>(1), x_range.get::<f64>(2)) {
                    config.graph.default_x_range = (min, max);
                }
            }
            if let Ok(y_range) = graph.get::<mlua::Table>("y_range") {
                if let (Ok(min), Ok(max)) = (y_range.get::<f64>(1), y_range.get::<f64>(2)) {
                    config.graph.default_y_range = (min, max);
                }
            }
            if let Ok(grid) = graph.get::<bool>("grid") {
                config.graph.grid_enabled = grid;
            }
        }

        // Appearance settings
        if let Ok(appearance) = table.get::<mlua::Table>("appearance") {
            if let Ok(font) = appearance.get::<String>("font_family") {
                config.appearance.font_family = font;
            }
            if let Ok(size) = appearance.get::<u32>("font_size") {
                config.appearance.font_size = size;
            }
            if let Ok(panel) = appearance.get::<bool>("button_panel") {
                config.appearance.button_panel_visible = panel;
            }
        }

        Ok(config)
    }

    /// Save configuration to TOML file
    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let path = toml_config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

fn lua_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gar")
        .join("init.lua")
}

fn toml_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("garcalc")
        .join("config.toml")
}
