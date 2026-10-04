//! Startup and shutdown shared by `ma serve` and `ma run`.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use axum::http::StatusCode;
use axum::routing::get;
use modular_agent_core::mcp_server::{McpServerConfig, McpServerHandle, mcp_router};
use modular_agent_core::workspace::Workspace;
use modular_agent_core::{CancellationToken, ModularAgent};
use tokio::task::JoinHandle;

use crate::events;

/// Bearer token for `/mcp`, read from the environment so it never shows up
/// in a process listing.
const TOKEN_ENV: &str = "MA_TOKEN";

#[derive(clap::Args)]
pub(crate) struct ServerArgs {
    /// Serve /healthz and /mcp on this address
    #[arg(long, value_name = "ADDR:PORT")]
    listen: Option<SocketAddr>,

    /// Extra Host header accepted by /mcp, for clients reaching a non-loopback address (repeatable)
    #[arg(long = "allowed-host", value_name = "HOST")]
    allowed_hosts: Vec<String>,

    /// Time allowed for patches to stop on shutdown
    #[arg(long, value_name = "DURATION", default_value = "5s", value_parser = parse_duration)]
    shutdown_timeout: Duration,
}

pub(crate) fn parse_duration(s: &str) -> Result<Duration, String> {
    let duration: jiff::SignedDuration = s.parse().map_err(|e| format!("{e}"))?;
    Duration::try_from(duration).map_err(|e| format!("{e}"))
}

pub(crate) struct Host {
    pub(crate) ma: ModularAgent,
    pub(crate) workspace: Option<Workspace>,
    /// Set once the patches are running; /healthz answers 503 until then.
    ready: Arc<AtomicBool>,
    http: Option<Http>,
    events_stop: CancellationToken,
    events: JoinHandle<()>,
    shutdown_timeout: Duration,
}

struct Http {
    mcp: McpServerHandle,
    shutdown: CancellationToken,
    server: JoinHandle<std::io::Result<()>>,
}

impl Host {
    /// Initializes the runtime, applies the workspace's global configs and
    /// starts listening. No patch runs yet, so a failure here leaves nothing
    /// to clean up.
    pub(crate) async fn start(
        workspace: Option<PathBuf>,
        server: &ServerArgs,
        fallback_patches_dir: Option<PathBuf>,
    ) -> Result<Self> {
        let ma = ModularAgent::init()?;
        ma.ready().await?;
        let events_stop = CancellationToken::new();
        let events = events::spawn(&ma, events_stop.clone());

        let workspace = workspace
            .map(|dir| -> Result<Workspace> {
                // Absolute first: the working directory moves into it below.
                let dir = std::path::absolute(&dir)
                    .with_context(|| format!("Invalid workspace path {}", dir.display()))?;
                let workspace = Workspace::open(&dir)
                    .with_context(|| format!("Failed to open workspace {}", dir.display()))?;
                let warnings = workspace.apply_global_configs(&ma);
                if !warnings.is_empty() {
                    for warning in &warnings {
                        log::error!("Workspace settings: {warning}");
                    }
                    bail!(
                        "{} setting(s) in {} cannot be applied",
                        warnings.len(),
                        dir.join("settings.json").display()
                    );
                }
                // Relative paths in patches (e.g. data/...) resolve against the
                // workspace, as they do in the desktop app.
                std::env::set_current_dir(&dir)
                    .with_context(|| format!("Failed to enter workspace {}", dir.display()))?;
                Ok(workspace)
            })
            .transpose()?;

        let ready = Arc::new(AtomicBool::new(false));
        let http = match server.listen {
            Some(addr) => {
                let patches_dir = workspace
                    .as_ref()
                    .map(Workspace::patches_dir)
                    .or(fallback_patches_dir);
                Some(listen(&ma, addr, server, patches_dir, ready.clone()).await?)
            }
            None => None,
        };

        Ok(Self {
            ma,
            workspace,
            ready,
            http,
            events_stop,
            events,
            shutdown_timeout: server.shutdown_timeout,
        })
    }

    pub(crate) fn set_ready(&self) {
        self.ready.store(true, Ordering::Relaxed);
    }

    /// Stops the MCP sessions, then the HTTP server, then the patches.
    pub(crate) async fn shutdown(self) -> Result<()> {
        if let Some(http) = self.http {
            // Open SSE streams would otherwise keep the graceful shutdown waiting.
            http.mcp.stop().await;
            http.shutdown.cancel();
            match http.server.await {
                Ok(Err(e)) => log::error!("HTTP server error: {e}"),
                Err(e) => log::error!("HTTP server task failed: {e}"),
                Ok(Ok(())) => {}
            }
        }
        let result = self.ma.shutdown(self.shutdown_timeout).await;
        self.events_stop.cancel();
        let _ = self.events.await;
        Ok(result?)
    }
}

async fn listen(
    ma: &ModularAgent,
    addr: SocketAddr,
    server: &ServerArgs,
    patches_dir: Option<PathBuf>,
    ready: Arc<AtomicBool>,
) -> Result<Http> {
    let token = std::env::var(TOKEN_ENV).ok().filter(|t| !t.is_empty());
    check_token(addr, token.is_some())?;

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to listen on {addr}"))?;
    let (mcp_router, mcp) = mcp_router(
        ma.clone(),
        McpServerConfig {
            patches_dir,
            token,
            allowed_hosts: server.allowed_hosts.clone(),
        },
    );
    let router = axum::Router::new()
        .route(
            "/healthz",
            get(move || async move {
                if ready.load(Ordering::Relaxed) {
                    StatusCode::OK
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                }
            }),
        )
        .merge(mcp_router);
    let shutdown = CancellationToken::new();
    let server = tokio::spawn(
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.clone().cancelled_owned())
            .into_future(),
    );
    log::info!(target: "modular_agent::events", "listening on http://{addr} (/healthz, /mcp)");
    Ok(Http {
        mcp,
        shutdown,
        server,
    })
}

/// Beyond loopback, /mcp must not be open to anyone who can reach the port.
fn check_token(addr: SocketAddr, has_token: bool) -> Result<()> {
    if !addr.ip().is_loopback() && !has_token {
        bail!("Listening on {addr} beyond loopback requires the {TOKEN_ENV} environment variable");
    }
    Ok(())
}

/// Resolves on SIGINT or SIGTERM (Ctrl+C only on Windows).
pub(crate) async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            Err(e) => {
                log::error!("Failed to listen for SIGTERM: {e}");
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_required_beyond_loopback() {
        let addr = |s: &str| s.parse::<SocketAddr>().unwrap();
        assert!(check_token(addr("127.0.0.1:8080"), false).is_ok());
        assert!(check_token(addr("[::1]:8080"), false).is_ok());
        assert!(check_token(addr("0.0.0.0:8080"), false).is_err());
        assert!(check_token(addr("0.0.0.0:8080"), true).is_ok());
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("5s"), Ok(Duration::from_secs(5)));
        assert_eq!(parse_duration("500ms"), Ok(Duration::from_millis(500)));
        assert_eq!(parse_duration("2m"), Ok(Duration::from_secs(120)));
        assert!(parse_duration("-1s").is_err());
        assert!(parse_duration("soon").is_err());
    }
}
