mod codegen;
mod config;
mod registry;
mod tui;

use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use dialoguer::Confirm;

use config::BuildConfig;

#[derive(Parser)]
#[command(name = "ma-config")]
#[command(about = "TUI wizard for configuring modular-agent-desktop with custom agent selections")]
struct Args {
    /// Path to the build config file
    #[arg(default_value = "ma-config.toml")]
    config: String,

    /// Path to the agent registry YAML file
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
    // Resolve desktop crate root directory
    let desktop_root = resolve_desktop_root()?;
    let tauri_root = desktop_root.join("src-tauri");

    // Load agent registry
    let registry_path = if Path::new(&args.registry).is_absolute() {
        PathBuf::from(&args.registry)
    } else {
        desktop_root.join("tools/ma-config").join(&args.registry)
    };
    let registry = registry::load(&registry_path)?;

    let config_path = desktop_root.join(&args.config);

    // Get or create build config
    let build_config = {
        let existing = if config_path.exists() {
            let existing_config = BuildConfig::load(&config_path)?;
            Some(existing_config)
        } else {
            None
        };

        if let Some(ref existing_config) = existing {
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
                1 => tui::run_wizard(Some(existing_config), &tauri_root, &registry)?,
                _ => tui::run_wizard(None, &tauri_root, &registry)?,
            }
        } else {
            tui::run_wizard(None, &tauri_root, &registry)?
        }
    };

    // Validate paths
    let warnings = codegen::validate_paths(&build_config, &tauri_root);
    if !warnings.is_empty() {
        eprintln!("\nPath validation warnings:");
        for w in &warnings {
            eprintln!("  - {w}");
        }
        let proceed = Confirm::new()
            .with_prompt("Continue anyway?")
            .default(false)
            .interact()
            .map_err(|e| e.to_string())?;
        if !proceed {
            return Err("Cancelled due to path validation warnings".to_string());
        }
    }

    // Save config
    build_config.save(&config_path)?;
    println!("Config saved to {}", config_path.display());

    // Generate files
    println!("Updating src-tauri/Cargo.toml...");
    codegen::update_cargo_toml(&build_config, &tauri_root)?;

    println!("Generating src-tauri/src/agents.rs...");
    codegen::generate_agents_rs(&build_config, &tauri_root)?;

    // Update Cargo.lock
    let should_update = Confirm::new()
        .with_prompt("Run cargo update?")
        .default(true)
        .interact()
        .map_err(|e| e.to_string())?;
    if should_update {
        println!("\nRunning: cargo update (in src-tauri/)");
        run_cargo_update(&tauri_root)?;
    }

    codegen::cleanup_backups(&tauri_root);
    println!("\nDone! Run `npm run tauri dev` or `npm run tauri build` to build.");

    Ok(())
}

fn resolve_desktop_root() -> Result<PathBuf, String> {
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;

    // Check ../../ first (when run from tools/ma-config/)
    let ancestor = current_dir.join("../../");
    if is_desktop_root(&ancestor) {
        return Ok(ancestor.canonicalize().map_err(|e| e.to_string())?);
    }

    // Check if we're in the desktop root already
    if is_desktop_root(&current_dir) {
        return Ok(current_dir);
    }

    Err(
        "Could not find modular-agent-desktop root. Run from the desktop root or from tools/ma-config/."
            .to_string(),
    )
}

/// Identify the desktop root by checking for src-tauri/Cargo.toml and src-tauri/src/lib.rs.
fn is_desktop_root(path: &Path) -> bool {
    path.join("src-tauri/Cargo.toml").exists() && path.join("src-tauri/src/lib.rs").exists()
}

fn run_cargo_update(tauri_root: &Path) -> Result<(), String> {
    let status = Command::new("cargo")
        .arg("update")
        .current_dir(tauri_root)
        .status()
        .map_err(|e| format!("Failed to run cargo update: {e}"))?;
    if !status.success() {
        return Err("cargo update failed".to_string());
    }
    Ok(())
}
