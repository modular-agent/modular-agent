use clap::Parser;
use modular_agent_core::{AgentError, AgentValue, ModularAgent, ModularAgentEvent};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::select;

#[derive(Parser)]
#[command(name = "cli")]
#[command(about = "Run a modular agent patch with stdin/stdout")]
struct Args {
    /// Path to the patch JSON file
    patch: String,

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

    // Validate patch file exists
    if !Path::new(&args.patch).exists() {
        return Err(AgentError::IoError(format!(
            "Patch file not found: {}",
            args.patch
        )));
    }

    // Initialize ModularAgent
    let ma = ModularAgent::init()?;
    ma.ready().await?;

    // Subscribe to external output BEFORE starting patch (avoid race condition)
    let output_channel = args.output.clone();
    let mut output_rx = ma.subscribe_to_event(move |envelope| {
        if let ModularAgentEvent::ExternalOutput(name, value) = envelope.event
            && name == output_channel
        {
            return Some(value);
        }
        None
    });

    // Load and start patch
    let patch_id = ma.open_patch_from_file(&args.patch, None).await?;
    ma.start_patch(&patch_id).await?;

    if args.verbose {
        eprintln!("Patch loaded: {}", args.patch);
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
    ma.stop_patch(&patch_id).await?;
    ma.shutdown(std::time::Duration::from_secs(5)).await?;

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
