//! Known agent crate catalog for the TUI wizard.

use std::path::Path;

use serde::Deserialize;

use crate::app::AppKind;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub agents: Vec<KnownAgent>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnownAgent {
    pub name: String,
    pub description: String,
    /// Absent for in-tree crates, which are never fetched from a remote.
    #[serde(default)]
    pub git_url: Option<String>,
    /// Lives in this workspace under `crates/`, so it is always a plain
    /// workspace dependency with no source to choose.
    #[serde(default)]
    pub in_tree: bool,
    #[serde(default)]
    pub available_features: Vec<String>,
    #[serde(default)]
    pub default_features: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<Conflict>,
    /// Apps that pre-select this agent on a fresh configuration.
    #[serde(default)]
    pub default_for: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conflict {
    pub with: String,
    pub reason: String,
    pub platform: Option<String>,
}

pub fn load(path: &Path) -> Result<Registry, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read registry file {}: {}", path.display(), e))?;
    let registry: Registry = serde_yaml::from_str(&content)
        .map_err(|e| format!("Failed to parse registry file {}: {}", path.display(), e))?;

    for agent in &registry.agents {
        if agent.in_tree == agent.git_url.is_some() {
            return Err(format!(
                "{}: registry entries need either in_tree: true or a git_url, not both or neither",
                agent.name
            ));
        }
    }
    Ok(registry)
}

impl KnownAgent {
    /// Where an out-of-tree agent is expected to be checked out, relative to
    /// the workspace root.
    pub fn default_path(&self) -> String {
        format!("../{}", self.name)
    }

    /// Path to an in-tree crate, relative to the workspace root.
    pub fn in_tree_path(&self) -> String {
        format!("crates/{}", self.name)
    }

    pub fn is_default_for(&self, app: AppKind) -> bool {
        self.default_for.iter().any(|a| a == app.slug())
    }

    pub fn has_selectable_features(&self) -> bool {
        !self.available_features.is_empty()
    }

    pub fn display_label(&self) -> String {
        let mut notes = String::new();
        if self.in_tree {
            notes.push_str(" [in-tree]");
        }
        if !self.conflicts.is_empty() {
            let names: Vec<&str> = self.conflicts.iter().map(|c| c.with.as_str()).collect();
            notes.push_str(&format!(" ⚠ {}", names.join(",")));
        }
        format!("{:<28} {}{}", self.name, self.description, notes)
    }
}

pub fn find_by_name<'a>(known_agents: &'a [KnownAgent], name: &str) -> Option<&'a KnownAgent> {
    known_agents.iter().find(|a| a.name == name)
}
