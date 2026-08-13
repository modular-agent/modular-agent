use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::app::AppKind;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct BuildConfig {
    pub agents: Vec<AgentEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentEntry {
    pub name: String,
    /// `None` for a registry agent: it always resolves to the clone under
    /// `custom_agents/<name>`, so there is nothing to record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<AgentSource>,
    /// `None` = use crate defaults, `Some(vec![])` = disable all features.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crate_features: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum AgentSource {
    /// An in-tree crate under `crates/`, taken from `[workspace.dependencies]`.
    Workspace,
    /// An agent the registry does not know, at a workspace-root-relative path.
    Path { path: String },
}

impl BuildConfig {
    pub fn load(app: AppKind, path: &Path) -> Result<Self, String> {
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

        // `Git` / `Registry` sources predate custom_agents/: out-of-tree agents
        // are clones under custom_agents/<name> now and carry no source at all.
        // Serde would only report an unknown variant, so name the real problem.
        if let Some(kind) = legacy_source_kind(&content) {
            return Err(format!(
                "{} is in the old format ('{kind}' agent source): out-of-tree agents are \
                 cloned into custom_agents/<name> now. Re-run the wizard to rebuild it: \
                 cargo run --manifest-path tools/ma-config/Cargo.toml -- {}",
                path.display(),
                app.slug()
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

/// The first `[agents.source] type` that no longer exists, if the file has one.
fn legacy_source_kind(content: &str) -> Option<String> {
    let agents = content.parse::<toml::Table>().ok()?;
    agents
        .get("agents")?
        .as_array()?
        .iter()
        .filter_map(|agent| agent.get("source")?.get("type")?.as_str())
        .find(|kind| matches!(*kind, "Git" | "Registry"))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(label: &str, content: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ma-config-cfg-{}-{label}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ma-config.toml");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn a_git_source_is_reported_as_the_old_format() {
        let path = write(
            "git",
            "[[agents]]\nname = \"modular-agent-lifelog\"\n\n[agents.source]\n\
             type = \"Git\"\nurl = \"https://example.com/x.git\"\n",
        );
        let err = BuildConfig::load(AppKind::Desktop, &path).unwrap_err();

        assert!(err.contains("old format"));
        assert!(err.contains("cargo run --manifest-path tools/ma-config/Cargo.toml -- desktop"));
    }

    #[test]
    fn a_registry_agent_needs_no_source() {
        let path = write(
            "no-source",
            "[[agents]]\nname = \"modular-agent-lifelog\"\n\n[[agents]]\n\
             name = \"modular-agent-std\"\n\n[agents.source]\ntype = \"Workspace\"\n",
        );
        let config = BuildConfig::load(AppKind::Cli, &path).unwrap();

        assert_eq!(config.agents[0].source, None);
        assert_eq!(config.agents[1].source, Some(AgentSource::Workspace));
    }
}
