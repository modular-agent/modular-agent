use clap::Parser;
use modular_agent_core::{AgentError, AgentValue, ModularAgent, ModularAgentEvent};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::select;

// Feature-gated imports to trigger inventory registration.
// Each `use` pulls in the crate, causing #[modular_agent] registrations to be linked.

#[cfg(feature = "std")]
#[allow(unused_imports)]
use modular_agent_std;

#[cfg(feature = "llm")]
#[allow(unused_imports)]
use modular_agent_llm;

#[cfg(feature = "web")]
#[allow(unused_imports)]
use modular_agent_web;

#[cfg(feature = "slack")]
#[allow(unused_imports)]
use modular_agent_slack;

#[cfg(feature = "sqlx")]
#[allow(unused_imports)]
use modular_agent_sqlx;

#[cfg(feature = "mongodb")]
#[allow(unused_imports)]
use modular_agent_mongodb;

#[cfg(feature = "lifelog")]
#[allow(unused_imports)]
use modular_agent_lifelog;

#[cfg(feature = "surrealdb")]
#[allow(unused_imports)]
use modular_agent_surrealdb;

#[cfg(feature = "lancedb")]
#[allow(unused_imports)]
use modular_agent_lancedb;

#[derive(Parser)]
#[command(name = "ma")]
#[command(about = "Run a modular agent preset with stdin/stdout")]
struct Args {
    /// Path to the preset JSON file
    preset: String,

    /// Name of the input channel
    #[arg(short, long, default_value = "input")]
    input: String,

    /// Name of the output channel
    #[arg(short, long, default_value = "output")]
    output: String,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), AgentError> {
    let args = Args::parse();

    // Initialize logging if verbose
    if args.verbose {
        env_logger::Builder::new()
            .filter_level(log::LevelFilter::Info)
            .init();
    }

    // Validate preset file exists
    if !Path::new(&args.preset).exists() {
        return Err(AgentError::IoError(format!(
            "Preset file not found: {}",
            args.preset
        )));
    }

    // Initialize ModularAgent
    let ma = ModularAgent::init()?;
    ma.ready().await?;

    // Subscribe to external output BEFORE starting preset (avoid race condition)
    let output_channel = args.output.clone();
    let mut output_rx = ma.subscribe_to_event(move |event| {
        if let ModularAgentEvent::ExternalOutput(name, value) = event {
            if name == output_channel {
                return Some(value);
            }
        }
        None
    });

    // Load and start preset
    let preset_id = ma.open_preset_from_file(&args.preset, None).await?;
    ma.start_preset(&preset_id).await?;

    if args.verbose {
        eprintln!("Preset loaded: {}", args.preset);
        eprintln!(
            "Input channel: {}, Output channel: {}",
            args.input, args.output
        );
    }

    // Setup async stdin
    let stdin = tokio::io::stdin();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    // Main loop with signal handling
    loop {
        select! {
            _ = tokio::signal::ctrl_c() => {
                if args.verbose {
                    eprintln!("\nShutting down...");
                }
                break;
            }
            result = lines.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        ma.write_external_input(
                            args.input.clone(),
                            AgentValue::string(line)
                        ).await?;
                    }
                    Ok(None) => break, // EOF
                    Err(e) => {
                        eprintln!("Error reading stdin: {}", e);
                        break;
                    }
                }
            }
            Some(value) = output_rx.recv() => {
                println!("{}", format_value(&value));
            }
        }
    }

    // Graceful shutdown
    ma.stop_preset(&preset_id).await?;
    ma.quit();

    // Drain any remaining output
    while let Ok(value) = output_rx.try_recv() {
        println!("{}", format_value(&value));
    }

    Ok(())
}

fn format_value(value: &AgentValue) -> String {
    match value {
        AgentValue::String(s) => s.to_string(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| format!("{:?}", value)),
    }
}
