use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub custom_headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSaverConfig {
    pub enabled: bool,
    pub smart_truncate: bool,
    pub max_tool_result_chars: usize,
    pub compress_whitespace: bool,
    pub strip_thinking: bool,
}

impl Default for TokenSaverConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            smart_truncate: true,
            max_tool_result_chars: 4000,
            compress_whitespace: true,
            strip_thinking: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub port: u16,
    pub token_saver: TokenSaverConfig,
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub allowed_keys: Vec<String>,
}

#[derive(Clone)]
pub struct AppState {
    pub config: std::sync::Arc<tokio::sync::RwLock<AppConfig>>,
    pub client: reqwest::Client,
    pub rtk: crate::rtk::RtkEngine,
}

pub type SharedState = std::sync::Arc<AppState>;

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            port: 30335,
            token_saver: TokenSaverConfig::default(),
            providers: HashMap::new(),
            allowed_keys: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn load(path: &str) -> Result<Self, String> {
        let content = fs::read_to_string(path).map_err(|e| format!("Failed to read config: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("Invalid JSON config: {}", e))
    }
}
