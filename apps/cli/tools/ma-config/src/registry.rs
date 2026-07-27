/// Known agent crate catalog for the TUI wizard.
use std::path::Path;

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepDefaults {
    pub default_path: String,
    pub git_url: String,
    pub default_version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub core: DepDefaults,
    pub agents: Vec<KnownAgent>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnownAgent {
    pub name: String,
    pub description: String,
    pub git_url: String,
    #[serde(default)]
    pub available_features: Vec<String>,
    #[serde(default)]
    pub default_features: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<Conflict>,
    #[serde(default)]
    pub is_default: bool,
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
    Ok(registry)
}

impl KnownAgent {
    pub fn default_path(&self) -> String {
        format!("../{}", self.name)
    }

    pub fn has_selectable_features(&self) -> bool {
        !self.available_features.is_empty()
    }

    pub fn display_label(&self) -> String {
        let conflict_note = if self.conflicts.is_empty() {
            String::new()
        } else {
            let names: Vec<&str> = self.conflicts.iter().map(|c| c.with.as_str()).collect();
            format!(" ⚠ {}", names.join(","))
        };
        format!("{:<28} {}{}", self.name, self.description, conflict_note)
    }
}

pub fn find_by_name<'a>(known_agents: &'a [KnownAgent], name: &str) -> Option<&'a KnownAgent> {
    known_agents.iter().find(|a| a.name == name)
}
