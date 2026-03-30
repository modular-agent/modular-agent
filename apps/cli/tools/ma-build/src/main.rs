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
#[command(name = "ma-build")]
#[command(about = "TUI wizard for building modular-agent-cli with custom agent selections")]
struct Args {
    /// Path to the build config file
    #[arg(default_value = "ma-build.toml")]
    config: String,

    /// Path to the agent registry YAML file
    #[arg(long, default_value = "registry.yaml")]
    registry: String,

    /// Build in release mode
    #[arg(long)]
    release: bool,
}

fn main() {
    let args = Args::parse();

    if let Err(e) = run(args) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<(), String> {
    // Resolve CLI crate root directory
    let cli_root = resolve_cli_root()?;

    // Load agent registry
    let registry_path = if Path::new(&args.registry).is_absolute() {
        PathBuf::from(&args.registry)
    } else {
        cli_root.join("tools/ma-build").join(&args.registry)
    };
    let known_agents = registry::load(&registry_path)?;

    let config_path = cli_root.join(&args.config);

    // Get or create build config
    let build_config = {
        // Interactive TUI wizard
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
                1 => tui::run_wizard(Some(existing_config), &cli_root, &known_agents)?,
                _ => tui::run_wizard(None, &cli_root, &known_agents)?,
            }
        } else {
            tui::run_wizard(None, &cli_root, &known_agents)?
        }
    };

    // Validate paths
    let warnings = codegen::validate_paths(&build_config, &cli_root);
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
    println!("Generating src/agents.rs...");
    codegen::generate_agents_rs(&build_config, &cli_root)?;

    println!("Updating Cargo.toml...");
    codegen::update_cargo_toml(&build_config, &cli_root)?;

    println!("Updating src/main.rs...");
    codegen::update_main_rs(&cli_root)?;

    // Build
    let should_build = Confirm::new()
        .with_prompt("Run cargo build now?")
        .default(true)
        .interact()
        .map_err(|e| e.to_string())?;

    if should_build {
        let release = if args.release {
            true
        } else {
            let items = &["Release", "Debug"];
            let selection = dialoguer::Select::new()
                .with_prompt("Build mode")
                .items(items)
                .default(0)
                .interact()
                .map_err(|e| e.to_string())?;
            selection == 0
        };
        let success = run_cargo_build(&cli_root, release)?;
        if success {
            codegen::cleanup_backups(&cli_root);
            println!("\nBuild succeeded!");
        } else {
            eprintln!("\nBuild failed.");
            let restore = Confirm::new()
                .with_prompt("Restore files from backup?")
                .default(true)
                .interact()
                .map_err(|e| e.to_string())?;
            if restore {
                codegen::restore_backups(&cli_root);
            }
            return Err("Build failed".to_string());
        }
    } else {
        codegen::cleanup_backups(&cli_root);
        println!("\nFiles generated. Run `cargo build` manually to build the CLI.");
    }

    Ok(())
}

fn resolve_cli_root() -> Result<PathBuf, String> {
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;

    // Check ../../ first (when run from tools/ma-build/)
    let ancestor = current_dir.join("../../");
    if is_cli_root(&ancestor) {
        return Ok(ancestor.canonicalize().map_err(|e| e.to_string())?);
    }

    // Check if we're in the CLI root already
    if is_cli_root(&current_dir) {
        return Ok(current_dir);
    }

    Err("Could not find modular-agent-cli root. Run from the CLI crate root or from tools/ma-build/.".to_string())
}

/// Identify the CLI root by checking for src/agents.rs (generated by ma-build).
fn is_cli_root(path: &PathBuf) -> bool {
    path.join("Cargo.toml").exists() && path.join("src/agents.rs").exists()
}

fn run_cargo_build(cli_root: &PathBuf, release: bool) -> Result<bool, String> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build");
    cmd.arg("--bin").arg("ma");

    if release {
        cmd.arg("--release");
    }

    cmd.current_dir(cli_root);

    println!(
        "\nRunning: cargo build --bin ma{}",
        if release { " --release" } else { "" }
    );

    let status = cmd
        .status()
        .map_err(|e| format!("Failed to run cargo: {e}"))?;
    Ok(status.success())
}
