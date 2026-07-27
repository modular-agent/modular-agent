use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use toml_edit::{Array, DocumentMut, InlineTable, Item, Value};

use crate::app::AppKind;
use crate::config::{AgentEntry, AgentSource, BuildConfig};
use crate::registry::{self, KnownAgent, Registry};

pub const PATCH_BEGIN: &str =
    "# ===== ma-config managed: external-agent local overrides (BEGIN) =====";
pub const PATCH_END: &str = "# ===== ma-config managed (END) =====";

/// Update the app's `Cargo.toml` with the selected agents as dependencies.
pub fn update_manifest(
    app: AppKind,
    config: &BuildConfig,
    registry: &Registry,
    root: &Path,
) -> Result<(), String> {
    let manifest_path = app.manifest_path(root);
    let original = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read {}: {e}", manifest_path.display()))?;

    let mut doc: DocumentMut = original
        .parse()
        .map_err(|e| format!("Failed to parse {}: {e}", manifest_path.display()))?;

    let deps = doc["dependencies"]
        .as_table_mut()
        .ok_or_else(|| format!("Missing [dependencies] in {}", manifest_path.display()))?;

    let stale: Vec<String> = deps
        .iter()
        .filter(|(key, _)| {
            key.starts_with("modular-agent-") || *key == "tauri-plugin-modular-agent"
        })
        .map(|(key, _)| key.to_string())
        .collect();
    for key in stale {
        deps.remove(&key);
    }

    // core and the plugin are in-tree and not selectable, so they are always
    // plain workspace dependencies.
    let mut core_dep = InlineTable::new();
    core_dep.insert("workspace", Value::from(true));
    let mut core_features = Array::new();
    for feature in app.core_features() {
        core_features.push(*feature);
    }
    core_dep.insert("features", Value::from(core_features));
    deps.insert(
        "modular-agent-core",
        Item::Value(Value::InlineTable(core_dep)),
    );

    if app.needs_plugin() {
        let mut plugin_dep = InlineTable::new();
        plugin_dep.insert("workspace", Value::from(true));
        deps.insert(
            "tauri-plugin-modular-agent",
            Item::Value(Value::InlineTable(plugin_dep)),
        );
    }

    for agent in &config.agents {
        let known = registry::find_by_name(&registry.agents, &agent.name);
        deps.insert(&agent.name, Item::Value(agent_dep_value(app, agent, known)));
    }

    fs::write(&manifest_path, doc.to_string())
        .map_err(|e| format!("Failed to write {}: {e}", manifest_path.display()))
}

/// The `[dependencies]` entry for one agent.
///
/// A local path for a known out-of-tree agent is expressed as the canonical git
/// URL here plus a `[patch]` entry at the workspace root, so reverting to the
/// canonical source only means deleting the patch.
fn agent_dep_value(app: AppKind, agent: &AgentEntry, known: Option<&KnownAgent>) -> Value {
    let custom_features = agent.crate_features.as_deref();

    if let Some(known) = known.filter(|k| k.in_tree) {
        let mut dep = InlineTable::new();
        match custom_features {
            // Feature overrides cannot ride on `workspace = true`: cargo
            // ignores a member's `default-features = false` when the workspace
            // entry does not set it. Spell the path out instead.
            Some(features) => {
                dep.insert(
                    "path",
                    Value::from(format!(
                        "{}/{}",
                        app.crate_dir_to_root(),
                        known.in_tree_path()
                    )),
                );
                dep.insert("default-features", Value::from(false));
                dep.insert("features", Value::from(feature_array(features)));
            }
            None => {
                dep.insert("workspace", Value::from(true));
            }
        }
        return Value::InlineTable(dep);
    }

    let patched_to_git = match agent.source {
        AgentSource::Path { .. } => known.and_then(|k| k.git_url.as_deref()),
        _ => None,
    };

    let mut dep = match patched_to_git {
        Some(url) => {
            let mut table = InlineTable::new();
            table.insert("git", Value::from(url));
            table
        }
        None => source_to_inline_table(&agent.source),
    };
    if let Some(features) = custom_features {
        dep.insert("default-features", Value::from(false));
        dep.insert("features", Value::from(feature_array(features)));
    }
    Value::InlineTable(dep)
}

fn feature_array(features: &[String]) -> Array {
    let mut array = Array::new();
    for feature in features {
        array.push(feature.as_str());
    }
    array
}

/// Generate the app's `agents.rs` with one import per selected agent.
pub fn generate_agents_rs(app: AppKind, config: &BuildConfig, root: &Path) -> Result<(), String> {
    let agents_path = app.agents_rs_path(root);

    let mut content = String::from(
        "//! Agent crate imports (auto-generated by ma-config, do not edit)\n\
         //!\n\
         //! Each `use` pulls in the crate, causing #[modular_agent] registrations\n\
         //! to be linked via the `inventory` crate.\n\n",
    );
    for agent in &config.agents {
        content.push_str(&format!("use {} as _;\n", agent.rust_crate_name()));
    }

    fs::write(&agents_path, content)
        .map_err(|e| format!("Failed to write {}: {e}", agents_path.display()))
}

/// Ensure the CLI's `main.rs` declares the generated agent module.
pub fn ensure_mod_agents(app: AppKind, root: &Path) -> Result<(), String> {
    let main_path = app.main_rs_path(root);
    let content = fs::read_to_string(&main_path)
        .map_err(|e| format!("Failed to read {}: {e}", main_path.display()))?;
    if content.contains("mod agents;") {
        return Ok(());
    }

    let mut lines: Vec<&str> = content.lines().collect();
    let after_use = lines
        .iter()
        .rposition(|line| line.starts_with("use "))
        .map(|i| i + 1)
        .unwrap_or(0);
    lines.insert(after_use, "mod agents;");
    lines.insert(after_use, "");

    fs::write(&main_path, lines.join("\n") + "\n")
        .map_err(|e| format!("Failed to write {}: {e}", main_path.display()))
}

/// Rewrite the ma-config managed region of the workspace `Cargo.toml`.
///
/// `[patch]` is workspace-wide, so the region holds the union of every app's
/// local overrides rather than only those of the app being configured.
pub fn update_root_patch(
    configs: &BTreeMap<AppKind, BuildConfig>,
    registry: &Registry,
    root: &Path,
) -> Result<(), String> {
    let manifest_path = root.join("Cargo.toml");
    let original = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read {}: {e}", manifest_path.display()))?;

    // git URL -> crate name -> local path
    let mut patches: BTreeMap<&str, BTreeMap<&str, &str>> = BTreeMap::new();
    for config in configs.values() {
        for agent in &config.agents {
            let AgentSource::Path { path } = &agent.source else {
                continue;
            };
            // Agents outside the registry have no canonical URL to patch
            // against; their path goes inline into [dependencies] instead.
            let Some(url) = registry::find_by_name(&registry.agents, &agent.name)
                .and_then(|known| known.git_url.as_deref())
            else {
                continue;
            };
            patches
                .entry(url)
                .or_default()
                .insert(agent.name.as_str(), path.as_str());
        }
    }

    let mut region = String::new();
    for (url, crates) in &patches {
        region.push_str(&format!("[patch.\"{url}\"]\n"));
        for (name, path) in crates {
            region.push_str(&format!(
                "{name} = {{ path = \"{}\" }}\n",
                normalize_path(path)
            ));
        }
        region.push('\n');
    }

    let updated = splice_managed_region(&original, &region)?;
    fs::write(&manifest_path, updated)
        .map_err(|e| format!("Failed to write {}: {e}", manifest_path.display()))
}

fn splice_managed_region(manifest: &str, region: &str) -> Result<String, String> {
    let begin = manifest
        .find(PATCH_BEGIN)
        .ok_or("Workspace Cargo.toml is missing the ma-config BEGIN marker")?;
    let end = manifest
        .find(PATCH_END)
        .ok_or("Workspace Cargo.toml is missing the ma-config END marker")?;
    if end < begin {
        return Err("ma-config markers in the workspace Cargo.toml are out of order".to_string());
    }

    let body_start = begin + PATCH_BEGIN.len();
    let mut out = String::with_capacity(manifest.len() + region.len());
    out.push_str(&manifest[..body_start]);
    out.push('\n');
    out.push_str(region);
    out.push_str(&manifest[end..]);
    Ok(out)
}

fn source_to_inline_table(source: &AgentSource) -> InlineTable {
    let mut dep = InlineTable::new();
    match source {
        AgentSource::Workspace => {
            dep.insert("workspace", Value::from(true));
        }
        AgentSource::Path { path } => {
            dep.insert("path", Value::from(normalize_path(path)));
        }
        AgentSource::Git { url, tag } => {
            dep.insert("git", Value::from(url.as_str()));
            if let Some(tag) = tag {
                dep.insert("tag", Value::from(tag.as_str()));
            }
        }
        AgentSource::Registry { version } => {
            dep.insert("version", Value::from(version.as_str()));
        }
    }
    dep
}

/// Normalize path separators: backslash -> forward slash for Cargo.toml compatibility.
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Report `Path` sources that do not point at a crate. Paths are relative to
/// the workspace root.
pub fn validate_paths(config: &BuildConfig, root: &Path) -> Vec<String> {
    config
        .agents
        .iter()
        .filter_map(|agent| match &agent.source {
            AgentSource::Path { path } if !root.join(path).join("Cargo.toml").exists() => {
                Some(format!(
                    "{}: path '{path}' does not contain a Cargo.toml",
                    agent.name
                ))
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> Registry {
        Registry {
            agents: vec![
                KnownAgent {
                    name: "modular-agent-std".into(),
                    description: "Standard agents".into(),
                    git_url: None,
                    in_tree: true,
                    available_features: vec!["image".into(), "yaml".into()],
                    default_features: vec!["image".into(), "yaml".into()],
                    conflicts: vec![],
                    default_for: vec!["desktop".into(), "cli".into()],
                },
                KnownAgent {
                    name: "modular-agent-slack".into(),
                    description: "Slack agents".into(),
                    git_url: Some(
                        "https://github.com/modular-agent/modular-agent-slack.git".into(),
                    ),
                    in_tree: false,
                    available_features: vec![],
                    default_features: vec![],
                    conflicts: vec![],
                    default_for: vec!["desktop".into()],
                },
            ],
        }
    }

    fn entry(name: &str, source: AgentSource, features: Option<Vec<String>>) -> AgentEntry {
        AgentEntry {
            name: name.into(),
            source,
            crate_features: features,
        }
    }

    fn scratch(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ma-config-{}-{label}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn manifest_after(label: &str, app: AppKind, config: &BuildConfig) -> String {
        let dir = scratch(label);
        let manifest = app.manifest_path(&dir);
        fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        fs::write(
            &manifest,
            "[dependencies]\nserde = \"1\"\nmodular-agent-gone = { git = \"stale\" }\n",
        )
        .unwrap();
        update_manifest(app, config, &registry(), &dir).unwrap();
        let out = fs::read_to_string(&manifest).unwrap();
        let _ = fs::remove_dir_all(&dir);
        out
    }

    #[test]
    fn in_tree_agents_become_workspace_dependencies() {
        let config = BuildConfig {
            agents: vec![entry("modular-agent-std", AgentSource::Workspace, None)],
        };
        let out = manifest_after("in-tree", AppKind::Desktop, &config);

        assert!(out.contains("modular-agent-std = { workspace = true }"));
        assert!(
            out.contains("modular-agent-core = { workspace = true, features = [\"mcp-server\"] }")
        );
        assert!(out.contains("tauri-plugin-modular-agent = { workspace = true }"));
        // Agents dropped from the selection lose their dependency line.
        assert!(!out.contains("modular-agent-gone"));
    }

    #[test]
    fn in_tree_feature_overrides_spell_out_the_path() {
        let config = BuildConfig {
            agents: vec![entry(
                "modular-agent-std",
                AgentSource::Workspace,
                Some(vec!["yaml".into()]),
            )],
        };
        let out = manifest_after("in-tree-features", AppKind::Cli, &config);

        assert!(out.contains(
            "modular-agent-std = { path = \"../../crates/modular-agent-std\", \
             default-features = false, features = [\"yaml\"] }"
        ));
        // The CLI does not link the Tauri plugin.
        assert!(!out.contains("tauri-plugin-modular-agent"));
    }

    #[test]
    fn local_paths_stay_out_of_the_app_manifest() {
        let config = BuildConfig {
            agents: vec![entry(
                "modular-agent-slack",
                AgentSource::Path {
                    path: "../modular-agent-slack".into(),
                },
                None,
            )],
        };
        let out = manifest_after("local-path", AppKind::Desktop, &config);

        assert!(out.contains(
            "modular-agent-slack = { git = \"https://github.com/modular-agent/modular-agent-slack.git\" }"
        ));
        assert!(!out.contains("path = \"../modular-agent-slack\""));
    }

    #[test]
    fn managed_region_holds_the_union_across_apps() {
        let manifest = format!("[workspace]\n\n{PATCH_BEGIN}\nstale content\n{PATCH_END}\n");
        let mut configs = BTreeMap::new();
        configs.insert(
            AppKind::Desktop,
            BuildConfig {
                agents: vec![entry(
                    "modular-agent-slack",
                    AgentSource::Path {
                        path: "..\\modular-agent-slack".into(),
                    },
                    None,
                )],
            },
        );
        configs.insert(
            AppKind::Cli,
            BuildConfig {
                agents: vec![entry("modular-agent-std", AgentSource::Workspace, None)],
            },
        );

        let dir = scratch("union");
        fs::write(dir.join("Cargo.toml"), &manifest).unwrap();
        update_root_patch(&configs, &registry(), &dir).unwrap();
        let out = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        let _ = fs::remove_dir_all(&dir);

        assert!(!out.contains("stale content"));
        assert!(
            out.contains("[patch.\"https://github.com/modular-agent/modular-agent-slack.git\"]")
        );
        // Backslashes from a Windows path are normalized for Cargo.
        assert!(out.contains("modular-agent-slack = { path = \"../modular-agent-slack\" }"));
        // In-tree agents never produce a patch entry.
        assert!(!out.contains("modular-agent-std ="));
        assert!(out.contains(PATCH_BEGIN) && out.contains(PATCH_END));
    }

    #[test]
    fn managed_region_clears_when_nothing_is_overridden() {
        let manifest = format!(
            "[workspace]\n\n{PATCH_BEGIN}\n[patch.\"x\"]\nfoo = {{ path = \"y\" }}\n{PATCH_END}\n"
        );
        let dir = scratch("clear");
        fs::write(dir.join("Cargo.toml"), &manifest).unwrap();
        update_root_patch(&BTreeMap::new(), &registry(), &dir).unwrap();
        let out = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(out, format!("[workspace]\n\n{PATCH_BEGIN}\n{PATCH_END}\n"));
    }
}
