// Release builds check `Send` on futures holding `im::Vector<ToolCall>`, and im's
// sized-chunks type-level arithmetic recurses past the default limit of 128.
#![recursion_limit = "256"]

use anyhow::Result;
use clap::{Parser, Subcommand};

mod events;
mod host;
mod logging;
mod modules;
mod run;
mod serve;

#[derive(Parser)]
#[command(name = "ma", about = "Run modular agent patches")]
struct Cli {
    /// Log format on stderr; RUST_LOG sets the filter
    #[arg(long, global = true, value_enum, default_value_t)]
    log_format: logging::LogFormat,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a workspace's auto-start patches until SIGTERM / SIGINT
    Serve(serve::ServeArgs),
    /// Run one patch with stdin/stdout wired to its external channels
    Run(run::RunArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    logging::init(cli.log_format);
    match cli.command {
        Command::Serve(args) => serve::serve(args).await,
        Command::Run(args) => run::run(args).await,
    }
}
