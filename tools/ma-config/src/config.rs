use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::app::AppKind;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct BuildConfig {
    pub modules: Vec<ModuleEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModuleEntry {
    pub name: String,
    /// `None` for a registry module: it always resolves to the clone under
    /// `custom_modules/<name>`, so there is nothing to record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ModuleSource>,
    /// `None` = use crate defaults, `Some(vec![])` = disable all features.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crate_features: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum ModuleSource {
    /// An in-tree crate under `crates/`, taken from `[workspace.dependencies]`.
    Workspace,
    /// A module the registry does not know, at a workspace-root-relative path.
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

        // `Git` / `Registry` sources predate custom_modules/: out-of-tree modules
        // are clones under custom_modules/<name> now and carry no source at all.
        // Serde would only report an unknown variant, so name the real problem.
        if let Some(kind) = legacy_source_kind(&content) {
            return Err(format!(
                "{} is in the old format ('{kind}' module source): out-of-tree modules are \
                 cloned into custom_modules/<name> now. Re-run the wizard to rebuild it: \
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

impl ModuleEntry {
    pub fn rust_crate_name(&self) -> String {
        self.name.replace('-', "_")
    }
}

/// The first `[modules.source] type` that no longer exists, if the file has one.
fn legacy_source_kind(content: &str) -> Option<String> {
    let modules = content.parse::<toml::Table>().ok()?;
    modules
        .get("modules")?
        .as_array()?
        .iter()
        .filter_map(|module| module.get("source")?.get("type")?.as_str())
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
            "[[modules]]\nname = \"modular-agent-lifelog\"\n\n[modules.source]\n\
             type = \"Git\"\nurl = \"https://example.com/x.git\"\n",
        );
        let err = BuildConfig::load(AppKind::Desktop, &path).unwrap_err();

        assert!(err.contains("old format"));
        assert!(err.contains("cargo run --manifest-path tools/ma-config/Cargo.toml -- desktop"));
    }

    #[test]
    fn a_registry_module_needs_no_source() {
        let path = write(
            "no-source",
            "[[modules]]\nname = \"modular-agent-lifelog\"\n\n[[modules]]\n\
             name = \"modular-agent-std\"\n\n[modules.source]\ntype = \"Workspace\"\n",
        );
        let config = BuildConfig::load(AppKind::Cli, &path).unwrap();

        assert_eq!(config.modules[0].source, None);
        assert_eq!(config.modules[1].source, Some(ModuleSource::Workspace));
    }
}
