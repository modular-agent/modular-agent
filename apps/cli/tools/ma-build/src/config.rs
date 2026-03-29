use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BuildConfig {
    pub core: AgentSource,
    pub agents: Vec<AgentEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentEntry {
    pub name: String,
    pub source: AgentSource,
    /// `None` = use crate defaults, `Some(vec![])` = disable all features.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crate_features: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum AgentSource {
    Path { path: String },
    Git { url: String, tag: Option<String> },
    Registry { version: String },
}

impl BuildConfig {
    pub fn load(path: &Path) -> Result<Self, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read config: {e}"))?;
        toml::from_str(&content).map_err(|e| format!("Failed to parse config: {e}"))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let content =
            toml::to_string_pretty(self).map_err(|e| format!("Failed to serialize config: {e}"))?;
        std::fs::write(path, content).map_err(|e| format!("Failed to write config: {e}"))
    }
}

impl AgentEntry {
    pub fn rust_crate_name(&self) -> String {
        self.name.replace('-', "_")
    }
}
