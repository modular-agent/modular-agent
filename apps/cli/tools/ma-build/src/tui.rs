use std::path::Path;

use console::Style;
use dialoguer::{Confirm, Input, MultiSelect, Select};

use crate::config::{AgentEntry, AgentSource, BuildConfig};
use crate::registry::{self, KnownAgent};

const CORE_DEFAULT_PATH: &str = "../modular-agent-core/modular-agent-core";
const CORE_DEFAULT_GIT: &str = "https://github.com/modular-agent/modular-agent-core.git";
const CORE_DEFAULT_VERSION: &str = "0.23.0";

pub fn run_wizard(
    existing_config: Option<&BuildConfig>,
    cli_root: &Path,
    known_agents: &[KnownAgent],
) -> Result<BuildConfig, String> {
    let bold = Style::new().bold();

    println!();
    println!(
        "{}",
        bold.apply_to("=== ma-build: Modular Agent CLI Builder ===")
    );
    println!();

    // Step 1: Core crate config
    let core = prompt_core_config(cli_root)?;

    // Step 2: Agent selection
    let selected_indices = select_agents(existing_config, known_agents)?;

    // Step 3: Build agent entries, auto-detecting local availability
    let mut agents = Vec::new();
    for idx in &selected_indices {
        let known = &known_agents[*idx];
        let default_path = known.default_path();
        let local_exists = cli_root.join(&default_path).join("Cargo.toml").exists();

        let source = if local_exists {
            prompt_local_or_git(&known.name, &default_path, &known.git_url)?
        } else {
            println!("  [{}] not found locally, using git", known.name);
            AgentSource::Git {
                url: known.git_url.to_string(),
                tag: None,
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

    // Step 4: Check conflicts
    check_conflicts(&agents, known_agents)?;

    // Step 5: Custom agents
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

    let config = BuildConfig { core, agents };

    // Step 6: Confirmation
    print_summary(&config);

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
    existing_config: Option<&BuildConfig>,
    known_agents: &[KnownAgent],
) -> Result<Vec<usize>, String> {
    let labels: Vec<String> = known_agents.iter().map(|a| a.display_label()).collect();

    let defaults: Vec<bool> = known_agents
        .iter()
        .map(|a| {
            if let Some(config) = existing_config {
                config.agents.iter().any(|e| e.name == a.name)
            } else {
                a.is_default
            }
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

/// Ask user to choose local path or git for a crate that exists locally.
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

    // Determine pre-selected items: use existing config if editing, else use defaults
    let preselected: Vec<bool> = available
        .iter()
        .map(|feat| {
            if let Some(config) = existing_config {
                config
                    .agents
                    .iter()
                    .find(|a| a.name == known.name)
                    .map(|a| match &a.crate_features {
                        None => defaults.contains(feat),
                        Some(feats) => feats.iter().any(|f| f == feat),
                    })
                    .unwrap_or_else(|| defaults.contains(feat))
            } else {
                defaults.contains(feat)
            }
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
            .with_prompt("Local path")
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

fn prompt_core_config(cli_root: &Path) -> Result<AgentSource, String> {
    let local_exists = cli_root.join(CORE_DEFAULT_PATH).join("Cargo.toml").exists();

    if local_exists {
        let items = &["Local path", "Git repository", "crates.io (version)"];
        let selection = Select::new()
            .with_prompt("[core] Source (local found)")
            .items(items)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;
        match selection {
            0 => Ok(AgentSource::Path {
                path: CORE_DEFAULT_PATH.to_string(),
            }),
            1 => Ok(AgentSource::Git {
                url: CORE_DEFAULT_GIT.to_string(),
                tag: None,
            }),
            _ => {
                let version: String = Input::new()
                    .with_prompt("[core] crates.io version")
                    .default(CORE_DEFAULT_VERSION.to_string())
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                Ok(AgentSource::Registry { version })
            }
        }
    } else {
        println!("  [core] not found locally");
        let items = &["Git repository", "crates.io (version)"];
        let selection = Select::new()
            .with_prompt("[core] Source")
            .items(items)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;
        match selection {
            0 => Ok(AgentSource::Git {
                url: CORE_DEFAULT_GIT.to_string(),
                tag: None,
            }),
            _ => {
                let version: String = Input::new()
                    .with_prompt("[core] crates.io version")
                    .default(CORE_DEFAULT_VERSION.to_string())
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                Ok(AgentSource::Registry { version })
            }
        }
    }
}

fn check_conflicts(entries: &[AgentEntry], known_agents: &[KnownAgent]) -> Result<(), String> {
    let selected_names: Vec<&str> = entries.iter().map(|a| a.name.as_str()).collect();

    for entry in entries {
        if let Some(known) = registry::find_by_name(known_agents, &entry.name) {
            for conflict in &known.conflicts {
                if selected_names.contains(&conflict.with.as_str()) {
                    let platform_note = conflict
                        .platform
                        .as_ref()
                        .map(|p| format!(" ({p} only)"))
                        .unwrap_or_default();

                    eprintln!(
                        "  Warning: {} conflicts with {}{}: {}",
                        known.name, conflict.with, platform_note, conflict.reason
                    );

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
    }

    Ok(())
}

fn format_source(source: &AgentSource) -> String {
    match source {
        AgentSource::Path { path } => format!("path: {path}"),
        AgentSource::Git { url, tag } => {
            let tag_str = tag.as_ref().map(|t| format!(" @ {t}")).unwrap_or_default();
            format!("git: {url}{tag_str}")
        }
        AgentSource::Registry { version } => format!("crates.io: v{version}"),
    }
}

fn print_summary(config: &BuildConfig) {
    let bold = Style::new().bold();
    let dim = Style::new().dim();

    println!();
    println!("{}", bold.apply_to("=== Build Configuration Summary ==="));
    println!();
    println!("  Core: {}", format_source(&config.core));
    println!();
    println!("  Agents:");
    for agent in &config.agents {
        let source_str = format_source(&agent.source);
        let features_str = match &agent.crate_features {
            None => String::new(),
            Some(feats) if feats.is_empty() => " [features: none]".to_string(),
            Some(feats) => format!(" [features: {}]", feats.join(", ")),
        };
        println!(
            "    {} {} {}{}",
            bold.apply_to(&agent.name),
            dim.apply_to("-"),
            source_str,
            features_str
        );
    }
    println!();
}
