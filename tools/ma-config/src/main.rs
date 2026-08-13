mod app;
mod codegen;
mod config;
mod registry;
mod tui;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use dialoguer::Confirm;

use app::{ALL_APPS, AppKind};
use config::BuildConfig;

#[derive(Parser)]
#[command(name = "ma-config")]
#[command(about = "TUI wizard for choosing which agent crates an app links")]
struct Args {
    /// Application to configure: desktop or cli
    app: String,

    /// Regenerate from the saved ma-config.toml without prompting
    #[arg(long)]
    apply: bool,

    /// Path to the in-tree agent registry YAML file
    #[arg(long, default_value = "registry.yaml")]
    registry: String,
}

fn main() {
    let args = Args::parse();

    if let Err(e) = run(args) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<(), String> {
    let app = AppKind::parse(&args.app)
        .ok_or_else(|| format!("Unknown app '{}'. Expected 'desktop' or 'cli'.", args.app))?;

    let root = resolve_workspace_root()?;

    let registry_path = if Path::new(&args.registry).is_absolute() {
        PathBuf::from(&args.registry)
    } else {
        root.join("tools/ma-config").join(&args.registry)
    };
    let registry = registry::load_all(&registry_path, &root)?;

    // Every app's selection is loaded: dependency resolution is workspace-wide,
    // so the conflict check cannot be decided from one app alone.
    let mut configs = load_all_configs(&root)?;
    let existing = configs.get(&app).cloned();

    let build_config = match &existing {
        Some(existing_config) if args.apply => existing_config.clone(),
        Some(existing_config) => {
            let items = &[
                "Rebuild with same configuration",
                "Modify configuration",
                "Start fresh",
            ];
            let selection = dialoguer::Select::new()
                .with_prompt("Found existing configuration. What would you like to do?")
                .items(items)
                .default(0)
                .interact()
                .map_err(|e| e.to_string())?;

            match selection {
                0 => existing_config.clone(),
                1 => tui::run_wizard(app, Some(existing_config), &registry)?,
                _ => tui::run_wizard(app, None, &registry)?,
            }
        }
        None if args.apply => {
            return Err(format!(
                "--apply needs an existing {}; run the wizard first",
                rel(&app.config_path(&root), &root)
            ));
        }
        None => tui::run_wizard(app, None, &registry)?,
    };

    configs.insert(app, build_config.clone());
    tui::check_conflicts(&configs, &registry, !args.apply)?;

    // Saved before validation so a missing clone does not throw away the
    // selection: clone what the error names, then re-run with --apply.
    let config_path = app.config_path(&root);
    build_config.save(&config_path)?;
    println!("Config saved to {}", config_path.display());

    codegen::validate_paths(&build_config, &root)?;

    println!("Updating {}...", rel(&app.manifest_path(&root), &root));
    codegen::update_manifest(app, &build_config, &registry, &root)?;

    println!("Generating {}...", rel(&app.agents_rs_path(&root), &root));
    codegen::generate_agents_rs(app, &build_config, &root)?;

    if app.needs_mod_agents() {
        codegen::ensure_mod_agents(app, &root)?;
    }

    let should_update = args.apply
        || Confirm::new()
            .with_prompt("Run cargo update?")
            .default(true)
            .interact()
            .map_err(|e| e.to_string())?;
    if should_update {
        run_cargo_update(&root)?;
    }

    println!("\nDone! {}", app.build_hint());
    Ok(())
}

fn load_all_configs(root: &Path) -> Result<BTreeMap<AppKind, BuildConfig>, String> {
    let mut configs = BTreeMap::new();
    for app in ALL_APPS {
        let path = app.config_path(root);
        if path.exists() {
            configs.insert(app, BuildConfig::load(app, &path)?);
        }
    }
    Ok(configs)
}

/// Walk up from the current directory to the manifest that declares the workspace.
fn resolve_workspace_root() -> Result<PathBuf, String> {
    let current = std::env::current_dir().map_err(|e| e.to_string())?;
    for dir in current.ancestors() {
        let manifest = dir.join("Cargo.toml");
        if !manifest.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&manifest)
            .map_err(|e| format!("Failed to read {}: {e}", manifest.display()))?;
        if content.contains("[workspace]") {
            return Ok(dir.to_path_buf());
        }
    }
    Err(format!(
        "No Cargo.toml with a [workspace] section above {}",
        current.display()
    ))
}

fn rel(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

fn run_cargo_update(root: &Path) -> Result<(), String> {
    println!("\nRunning: cargo update");
    let status = Command::new("cargo")
        .arg("update")
        .current_dir(root)
        .status()
        .map_err(|e| format!("Failed to run cargo update: {e}"))?;
    if !status.success() {
        return Err("cargo update failed".to_string());
    }
    Ok(())
}
