//! `ma serve`: run a workspace's auto-start patches until a signal arrives.

use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::host::{Host, ServerArgs, shutdown_signal};

#[derive(clap::Args)]
pub(crate) struct ServeArgs {
    /// Workspace directory (settings.json and patches/)
    #[arg(long, value_name = "DIR")]
    workspace: PathBuf,

    #[command(flatten)]
    server: ServerArgs,
}

pub(crate) async fn serve(args: ServeArgs) -> Result<()> {
    let host = Host::start(Some(args.workspace), &args.server, None).await?;

    let failures = match &host.workspace {
        Some(workspace) => workspace.auto_start(&host.ma).await,
        None => Vec::new(),
    };
    if !failures.is_empty() {
        for (name, e) in &failures {
            log::error!(target: "modular_agent::events", patch = name.as_str(); "failed to auto-start: {e}");
        }
        // Leave restarting to the orchestrator; stop what did start first.
        host.shutdown().await?;
        bail!("{} patch(es) failed to auto-start", failures.len());
    }
    host.set_ready();

    shutdown_signal().await;
    log::info!(target: "modular_agent::events", "shutting down");
    host.shutdown().await
}
