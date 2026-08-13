use std::collections::BTreeMap;

use console::Style;
use dialoguer::{Confirm, Input, MultiSelect};

use crate::app::AppKind;
use crate::config::{AgentEntry, AgentSource, BuildConfig};
use crate::registry::{self, KnownAgent, Registry};

pub fn run_wizard(
    app: AppKind,
    existing_config: Option<&BuildConfig>,
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

        // Out-of-tree agents have no source to pick: they resolve to
        // custom_agents/<name>, and codegen fails if that clone is missing.
        let source = known.in_tree.then_some(AgentSource::Workspace);

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

    // None = registry default_features (materialized by codegen for
    // out-of-tree deps), Some([...]) = explicit override
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

    let path: String = Input::new()
        .with_prompt("Path from the workspace root (e.g. custom_agents/modular-agent-my-custom)")
        .interact_text()
        .map_err(|e| e.to_string())?;

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
        source: Some(AgentSource::Path { path }),
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

fn format_source(agent: &AgentEntry) -> String {
    match &agent.source {
        Some(AgentSource::Workspace) => "in-tree (workspace)".to_string(),
        Some(AgentSource::Path { path }) => format!("path: {path}"),
        None => format!("path: {}", registry::clone_path(&agent.name)),
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
            format_source(agent),
            features_str
        );
    }
    println!();
}
