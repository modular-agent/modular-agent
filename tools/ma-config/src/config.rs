use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct BuildConfig {
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum AgentSource {
    /// An in-tree crate under `crates/`, taken from `[workspace.dependencies]`.
    Workspace,
    Path {
        path: String,
    },
    Git {
        url: String,
        tag: Option<String>,
    },
    Registry {
        version: String,
    },
}

impl BuildConfig {
    pub fn load(path: &Path) -> Result<Self, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read config: {e}"))?;

        // Configs written before the monorepo carried core/plugin source
        // selections and paths relative to the app directory. Neither means
        // anything now, and silently dropping them would leave the user with a
        // selection they never made.
        let stale = content
            .parse::<toml::Table>()
            .map(|raw| raw.contains_key("core") || raw.contains_key("plugin"))
            .unwrap_or(false);
        if stale {
            return Err(format!(
                "{} predates the monorepo: it pins modular-agent-core / \
                 tauri-plugin-modular-agent to a source, and both are in-tree now. \
                 Delete the file and re-run the wizard.",
                path.display()
            ));
        }

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
