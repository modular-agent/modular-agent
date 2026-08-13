//! Known agent crate catalog for the TUI wizard.
//!
//! The catalog comes from two places: the central `registry.yaml` next to this
//! tool, which lists the in-tree crates, and one `ma-registry.yaml` per
//! out-of-tree agent repository, read from its clone under `custom_agents/`.
//! An agent crate therefore describes itself in its own repository, and only
//! clones that are actually present are offered.

use std::path::Path;

use serde::Deserialize;

use crate::app::AppKind;

const REPO_FILE: &str = "ma-registry.yaml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub agents: Vec<KnownAgent>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnownAgent {
    pub name: String,
    pub description: String,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conflict {
    pub with: String,
    pub reason: String,
    pub platform: Option<String>,
}

/// The `ma-registry.yaml` an out-of-tree agent repository carries at its root.
///
/// It describes a single crate, so there is no `in_tree` flag and no source:
/// the file is only ever read from the crate's own clone.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RepoEntry {
    name: String,
    description: String,
    #[serde(default)]
    available_features: Vec<String>,
    #[serde(default)]
    default_features: Vec<String>,
    #[serde(default)]
    conflicts: Vec<Conflict>,
    #[serde(default)]
    default_for: Vec<String>,
}

impl RepoEntry {
    fn into_known_agent(self) -> KnownAgent {
        KnownAgent {
            name: self.name,
            description: self.description,
            in_tree: false,
            available_features: self.available_features,
            default_features: self.default_features,
            conflicts: self.conflicts,
            default_for: self.default_for,
        }
    }
}

/// The full catalog: the in-tree crates plus every cloned out-of-tree agent.
pub fn load_all(central_path: &Path, root: &Path) -> Result<Registry, String> {
    let mut agents = load(central_path)?.agents;
    agents.extend(scan_custom_agents(root)?);
    agents.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Registry { agents })
}

/// Read the central catalog, which only covers the crates in this workspace.
pub fn load(path: &Path) -> Result<Registry, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read registry file {}: {}", path.display(), e))?;
    let registry: Registry = serde_yaml::from_str(&content)
        .map_err(|e| format!("Failed to parse registry file {}: {}", path.display(), e))?;

    for agent in &registry.agents {
        if !agent.in_tree {
            return Err(format!(
                "{}: {} only lists in-tree crates. Describe this crate in a {REPO_FILE} at the \
                 root of the {} repository instead — see custom_agents/README.md.",
                agent.name,
                path.display(),
                agent.name
            ));
        }
    }
    Ok(registry)
}

/// Read one catalog entry per crate cloned under `custom_agents/`.
///
/// A clone without a `ma-registry.yaml` still shows up in the wizard, described
/// by its `Cargo.toml`: it just offers no features and declares no conflicts.
pub fn scan_custom_agents(root: &Path) -> Result<Vec<KnownAgent>, String> {
    let dir = root.join("custom_agents");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };

    let mut agents = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read {}: {e}", dir.display()))?;
        let clone_dir = entry.path();
        let manifest = clone_dir.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().into_owned();

        let repo_file = clone_dir.join(REPO_FILE);
        let agent = if repo_file.is_file() {
            let content = std::fs::read_to_string(&repo_file)
                .map_err(|e| format!("Failed to read {}: {e}", repo_file.display()))?;
            let entry: RepoEntry = serde_yaml::from_str(&content)
                .map_err(|e| format!("Failed to parse {}: {e}", repo_file.display()))?;
            check_name(&entry.name, &dir_name, &repo_file)?;
            entry.into_known_agent()
        } else {
            from_manifest(&manifest, &dir_name)?
        };
        agents.push(agent);
    }

    agents.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(agents)
}

/// Minimal entry for a clone that carries no `ma-registry.yaml`.
fn from_manifest(manifest_path: &Path, dir_name: &str) -> Result<KnownAgent, String> {
    let content = std::fs::read_to_string(manifest_path)
        .map_err(|e| format!("Failed to read {}: {e}", manifest_path.display()))?;
    let manifest: toml::Table = content
        .parse()
        .map_err(|e| format!("Failed to parse {}: {e}", manifest_path.display()))?;
    let package = manifest.get("package");

    let name = package
        .and_then(|p| p.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            format!(
                "{} has no [package].name, so there is no crate to link. Remove the directory or \
                 add a {REPO_FILE} describing the crate.",
                manifest_path.display()
            )
        })?;
    check_name(name, dir_name, manifest_path)?;

    let description = package
        .and_then(|p| p.get("description"))
        .and_then(toml::Value::as_str)
        .unwrap_or("(no description)");

    Ok(KnownAgent {
        name: name.to_string(),
        description: description.to_string(),
        in_tree: false,
        available_features: Vec::new(),
        default_features: Vec::new(),
        conflicts: Vec::new(),
        default_for: Vec::new(),
    })
}

/// The crate is linked as a path dependency on `custom_agents/<name>`, so a
/// clone whose directory does not carry the crate's name cannot be linked at
/// all — usually a copy-paste slip in a freshly added `ma-registry.yaml`.
fn check_name(name: &str, dir_name: &str, source: &Path) -> Result<(), String> {
    if name == dir_name {
        return Ok(());
    }
    Err(format!(
        "{}: name '{name}' does not match the directory it was found in, custom_agents/{dir_name}. \
         Clone the repository into custom_agents/{name}.",
        source.display()
    ))
}

impl KnownAgent {
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

/// Where an out-of-tree agent is cloned, relative to the workspace root.
pub fn clone_path(name: &str) -> String {
    format!("custom_agents/{name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ma-registry-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Lay out one clone under `custom_agents/` and scan the result.
    fn scan_one(
        label: &str,
        dir_name: &str,
        manifest: &str,
        repo_file: Option<&str>,
    ) -> Result<Vec<KnownAgent>, String> {
        let root = scratch(label);
        let clone_dir = root.join("custom_agents").join(dir_name);
        std::fs::create_dir_all(&clone_dir).unwrap();
        std::fs::write(clone_dir.join("Cargo.toml"), manifest).unwrap();
        if let Some(content) = repo_file {
            std::fs::write(clone_dir.join(REPO_FILE), content).unwrap();
        }
        let result = scan_custom_agents(&root);
        let _ = std::fs::remove_dir_all(&root);
        result
    }

    #[test]
    fn a_repo_file_describes_the_cloned_crate() {
        let agents = scan_one(
            "repo-file",
            "modular-agent-example",
            "[package]\nname = \"modular-agent-example\"\n",
            Some(
                "name: modular-agent-example\n\
                 description: Example agents\n\
                 available_features:\n  - basic\n  - extra\n\
                 default_features:\n  - extra\n\
                 conflicts:\n  - with: modular-agent-sqlx\n    reason: made up\n    platform: windows\n\
                 default_for: [desktop]\n",
            ),
        )
        .unwrap();

        assert_eq!(agents.len(), 1);
        let example = &agents[0];
        assert_eq!(example.name, "modular-agent-example");
        assert_eq!(example.description, "Example agents");
        assert!(!example.in_tree);
        assert_eq!(example.available_features, ["basic", "extra"]);
        assert_eq!(example.default_features, ["extra"]);
        assert_eq!(example.conflicts[0].with, "modular-agent-sqlx");
        assert_eq!(example.conflicts[0].platform.as_deref(), Some("windows"));
        assert!(example.is_default_for(AppKind::Desktop));
        assert!(!example.is_default_for(AppKind::Cli));
    }

    #[test]
    fn a_repo_file_naming_another_crate_is_rejected() {
        let err = scan_one(
            "name-mismatch",
            "modular-agent-example",
            "[package]\nname = \"modular-agent-example\"\n",
            Some("name: modular-agent-lifelog\ndescription: Lifelog agents\n"),
        )
        .unwrap_err();

        assert!(err.contains("modular-agent-lifelog"));
        assert!(err.contains("custom_agents/modular-agent-example"));
    }

    #[test]
    fn a_clone_without_a_repo_file_falls_back_to_its_manifest() {
        let agents = scan_one(
            "manifest-fallback",
            "modular-agent-custom",
            "[package]\n\
             name = \"modular-agent-custom\"\n\
             description = \"Custom experiment agents\"\n\
             [features]\nextra = []\n",
            None,
        )
        .unwrap();

        assert_eq!(agents.len(), 1);
        let custom = &agents[0];
        assert_eq!(custom.name, "modular-agent-custom");
        assert_eq!(custom.description, "Custom experiment agents");
        assert!(!custom.in_tree);
        // Features are only selectable when a repo file declares them.
        assert!(!custom.has_selectable_features());
        assert!(custom.conflicts.is_empty());
        assert!(custom.default_for.is_empty());
    }

    #[test]
    fn an_out_of_tree_entry_in_the_central_file_is_rejected() {
        let dir = scratch("central-out-of-tree");
        let path = dir.join("registry.yaml");
        std::fs::write(
            &path,
            "agents:\n\
             \x20 - name: modular-agent-std\n    description: Standard agents\n    in_tree: true\n\
             \x20 - name: modular-agent-lifelog\n    description: Lifelog agents\n",
        )
        .unwrap();

        let err = load(&path).unwrap_err();
        let _ = std::fs::remove_dir_all(&dir);

        assert!(err.contains("modular-agent-lifelog"));
        assert!(err.contains(REPO_FILE));
    }
}
