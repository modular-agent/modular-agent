use std::collections::BTreeMap;
use std::path::Path;

use console::Style;
use dialoguer::{Confirm, Input, MultiSelect, Select};

use crate::app::AppKind;
use crate::config::{AgentEntry, AgentSource, BuildConfig};
use crate::registry::{self, KnownAgent, Registry};

pub fn run_wizard(
    app: AppKind,
    existing_config: Option<&BuildConfig>,
    root: &Path,
    registry: &Registry,
) -> Result<BuildConfig, String> {
    let known_agents = registry.agents.as_slice();
    let bold = Style::new().bold();

    println!();
    println!(
        "{}",
        bold.apply_to(format!("=== ma-config: {} ===", app.title()))
    );
    println!();

    let selected_indices = select_agents(app, existing_config, known_agents)?;

    let mut agents = Vec::new();
    for idx in &selected_indices {
        let known = &known_agents[*idx];

        let source = if known.in_tree {
            AgentSource::Workspace
        } else {
            let default_path = known.default_path();
            let git_url = known.git_url.as_deref().unwrap_or_default();
            if root.join(&default_path).join("Cargo.toml").exists() {
                prompt_local_or_git(&known.name, &default_path, git_url)?
            } else {
                println!("  [{}] not found locally, using git", known.name);
                AgentSource::Git {
                    url: git_url.to_string(),
                    tag: None,
                }
            }
        };

        let crate_features = if known.has_selectable_features() {
            prompt_crate_features(known, existing_config)?
        } else {
            None
        };

        agents.push(AgentEntry {
            name: known.name.to_string(),
            source,
            crate_features,
        });
    }

    loop {
        let add_custom = Confirm::new()
            .with_prompt("Add a custom agent crate not in the list above?")
            .default(false)
            .interact()
            .map_err(|e| e.to_string())?;

        if !add_custom {
            break;
        }
        agents.push(prompt_custom_agent()?);
    }

    let config = BuildConfig { agents };
    print_summary(app, &config);

    let confirmed = Confirm::new()
        .with_prompt("Proceed with this configuration?")
        .default(true)
        .interact()
        .map_err(|e| e.to_string())?;

    if !confirmed {
        return Err("Cancelled by user".to_string());
    }

    Ok(config)
}

fn select_agents(
    app: AppKind,
    existing_config: Option<&BuildConfig>,
    known_agents: &[KnownAgent],
) -> Result<Vec<usize>, String> {
    let labels: Vec<String> = known_agents.iter().map(|a| a.display_label()).collect();

    let defaults: Vec<bool> = known_agents
        .iter()
        .map(|a| match existing_config {
            Some(config) => config.agents.iter().any(|e| e.name == a.name),
            None => a.is_default_for(app),
        })
        .collect();

    let selected = MultiSelect::new()
        .with_prompt("Select agents to include (Space to toggle, Enter to confirm)")
        .items(&labels)
        .defaults(&defaults)
        .max_length(known_agents.len())
        .interact()
        .map_err(|e| e.to_string())?;

    if selected.is_empty() {
        return Err("No agents selected".to_string());
    }

    let names: Vec<&str> = selected
        .iter()
        .map(|&i| known_agents[i].name.as_str())
        .collect();
    println!("  Selected: {}", names.join(", "));

    Ok(selected)
}

/// Ask the user to choose local path or git for a crate that exists locally.
fn prompt_local_or_git(
    name: &str,
    default_path: &str,
    git_url: &str,
) -> Result<AgentSource, String> {
    let items = &["Local path", "Git repository"];
    let selection = Select::new()
        .with_prompt(format!("[{name}] Source (local found)"))
        .items(items)
        .default(0)
        .interact()
        .map_err(|e| e.to_string())?;

    if selection == 0 {
        Ok(AgentSource::Path {
            path: default_path.to_string(),
        })
    } else {
        Ok(AgentSource::Git {
            url: git_url.to_string(),
            tag: None,
        })
    }
}

fn prompt_crate_features(
    known: &KnownAgent,
    existing_config: Option<&BuildConfig>,
) -> Result<Option<Vec<String>>, String> {
    let available = &known.available_features;
    let defaults = &known.default_features;

    let preselected: Vec<bool> = available
        .iter()
        .map(|feat| match existing_config {
            Some(config) => config
                .agents
                .iter()
                .find(|a| a.name == known.name)
                .map(|a| match &a.crate_features {
                    None => defaults.contains(feat),
                    Some(feats) => feats.contains(feat),
                })
                .unwrap_or_else(|| defaults.contains(feat)),
            None => defaults.contains(feat),
        })
        .collect();

    let labels: Vec<String> = available
        .iter()
        .map(|feat| {
            if defaults.contains(feat) {
                format!("{feat} (default)")
            } else {
                feat.to_string()
            }
        })
        .collect();

    let selected_indices = MultiSelect::new()
        .with_prompt(format!(
            "[{}] Select crate features (Space to toggle)",
            known.name
        ))
        .items(&labels)
        .defaults(&preselected)
        .interact()
        .map_err(|e| e.to_string())?;

    let selected: Vec<String> = selected_indices
        .iter()
        .map(|&i| available[i].to_string())
        .collect();

    // None = use crate defaults, Some([...]) = explicit override
    if selected.len() == defaults.len() && selected.iter().all(|f| defaults.contains(f)) {
        Ok(None)
    } else {
        Ok(Some(selected))
    }
}

fn prompt_custom_agent() -> Result<AgentEntry, String> {
    let name: String = Input::new()
        .with_prompt("Crate name (e.g., modular-agent-my-custom)")
        .interact_text()
        .map_err(|e| e.to_string())?;

    let items = &["Local path", "Git repository"];
    let selection = Select::new()
        .with_prompt("Source type")
        .items(items)
        .default(0)
        .interact()
        .map_err(|e| e.to_string())?;

    let source = if selection == 0 {
        let path: String = Input::new()
            .with_prompt("Local path (relative to the workspace root)")
            .interact_text()
            .map_err(|e| e.to_string())?;
        AgentSource::Path { path }
    } else {
        let url: String = Input::new()
            .with_prompt("Git URL")
            .interact_text()
            .map_err(|e| e.to_string())?;
        let tag: String = Input::new()
            .with_prompt("Git tag (empty for latest)")
            .default(String::new())
            .allow_empty(true)
            .interact_text()
            .map_err(|e| e.to_string())?;
        AgentSource::Git {
            url,
            tag: if tag.is_empty() { None } else { Some(tag) },
        }
    };

    let features_str: String = Input::new()
        .with_prompt("Crate features (comma-separated, empty for none)")
        .default(String::new())
        .allow_empty(true)
        .interact_text()
        .map_err(|e| e.to_string())?;

    let crate_features: Option<Vec<String>> = if features_str.is_empty() {
        None
    } else {
        Some(
            features_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        )
    };

    Ok(AgentEntry {
        name,
        source,
        crate_features,
    })
}

/// Warn about conflicting agents across every configured app.
///
/// The workspace resolves dependencies once for all members, so two apps that
/// each pick one half of a `links` conflict break the whole workspace, not just
/// their own build.
pub fn check_conflicts(
    configs: &BTreeMap<AppKind, BuildConfig>,
    registry: &Registry,
    interactive: bool,
) -> Result<(), String> {
    let mut selected: BTreeMap<&str, Vec<AppKind>> = BTreeMap::new();
    for (app, config) in configs {
        for agent in &config.agents {
            selected.entry(agent.name.as_str()).or_default().push(*app);
        }
    }

    let mut reported: Vec<(&str, &str)> = Vec::new();
    for name in selected.keys() {
        let Some(known) = registry::find_by_name(&registry.agents, name) else {
            continue;
        };
        for conflict in &known.conflicts {
            let Some(other_apps) = selected.get(conflict.with.as_str()) else {
                continue;
            };
            // Report each pair once, in a stable order.
            let pair = if *name < conflict.with.as_str() {
                (*name, conflict.with.as_str())
            } else {
                (conflict.with.as_str(), *name)
            };
            if reported.contains(&pair) {
                continue;
            }
            reported.push(pair);

            let platform_note = conflict
                .platform
                .as_ref()
                .map(|p| format!(" ({p} only)"))
                .unwrap_or_default();
            let apps: Vec<&str> = selected[name]
                .iter()
                .chain(other_apps)
                .map(|a| a.slug())
                .collect();

            eprintln!(
                "  Warning: {} conflicts with {}{}: {} [selected in: {}]",
                known.name,
                conflict.with,
                platform_note,
                conflict.reason,
                apps.join(", ")
            );

            if interactive {
                let proceed = Confirm::new()
                    .with_prompt("Continue anyway?")
                    .default(false)
                    .interact()
                    .map_err(|e| e.to_string())?;
                if !proceed {
                    return Err("Cancelled due to conflict".to_string());
                }
            }
        }
    }

    Ok(())
}

fn format_source(source: &AgentSource) -> String {
    match source {
        AgentSource::Workspace => "in-tree (workspace)".to_string(),
        AgentSource::Path { path } => format!("path: {path}"),
        AgentSource::Git { url, tag } => {
            let tag_str = tag.as_ref().map(|t| format!(" @ {t}")).unwrap_or_default();
            format!("git: {url}{tag_str}")
        }
        AgentSource::Registry { version } => format!("crates.io: v{version}"),
    }
}

fn print_summary(app: AppKind, config: &BuildConfig) {
    let bold = Style::new().bold();
    let dim = Style::new().dim();

    println!();
    println!(
        "{}",
        bold.apply_to(format!("=== {} configuration ===", app.title()))
    );
    println!();
    for agent in &config.agents {
        let features_str = match &agent.crate_features {
            None => String::new(),
            Some(feats) if feats.is_empty() => " [features: none]".to_string(),
            Some(feats) => format!(" [features: {}]", feats.join(", ")),
        };
        println!(
            "  {} {} {}{}",
            bold.apply_to(&agent.name),
            dim.apply_to("-"),
            format_source(&agent.source),
            features_str
        );
    }
    println!();
}
