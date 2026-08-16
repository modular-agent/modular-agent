// Release builds check `Send` on futures holding `im::Vector<ToolCall>`, and im's
// sized-chunks type-level arithmetic recurses past the default limit of 128.
#![recursion_limit = "256"]

use clap::Parser;
use modular_agent_core::mcp_server::{McpServerConfig, start_mcp_server};
use modular_agent_core::{AgentError, AgentValue, ModularAgent, ModularAgentEvent};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::select;

mod agents;

#[derive(Parser)]
#[command(name = "ma")]
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

    /// Serve the built-in MCP server on this port (binds 127.0.0.1 only)
    #[arg(long, value_name = "PORT")]
    mcp_port: Option<u16>,

    /// Bearer token required for MCP requests (omit to disable auth)
    #[arg(long, value_name = "TOKEN", requires = "mcp_port")]
    mcp_token: Option<String>,

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

    // Load the patch first so MCP clients can see it as soon as the server
    // is up, but start the MCP server before starting the patch: a bind
    // failure (e.g. port already in use) must not leave running agents
    // behind without their stop() hooks being called.
    let patch_id = ma.open_patch_from_file(&args.patch, None).await?;

    // Optionally serve the built-in MCP server so external agents (e.g.
    // Claude Code) can inspect and edit the running flow.
    let mcp_server = match args.mcp_port {
        Some(port) => {
            // save_patch writes relative to this dir; the directory of the
            // patch being run is the only patches root the CLI knows about.
            let patches_dir = match Path::new(&args.patch).parent() {
                Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
                _ => PathBuf::from("."),
            };
            let config = McpServerConfig {
                port,
                patches_dir: Some(patches_dir),
                token: args.mcp_token.clone(),
            };
            let handle = start_mcp_server(ma.clone(), config).await?;
            if args.verbose {
                eprintln!("MCP server listening on http://127.0.0.1:{}/mcp", port);
            }
            Some(handle)
        }
        None => None,
    };

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

    // Graceful shutdown: stop the MCP server first so no external edits
    // arrive while the patch is being torn down.
    if let Some(server) = mcp_server {
        server.stop().await;
    }
    ma.stop_patch(&patch_id).await?;
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
