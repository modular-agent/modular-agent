use anyhow::{Context as _, Result};
use modular_agent_kit::{AgentValue, MAKEvent, MAK};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast::error::RecvError;

const EMIT_AGENT_CONFIG_UPDATED: &str = "mak:agent_config_updated";
const EMIT_AGENT_ERROR: &str = "mak:agent_error";
const EMIT_AGENT_IN: &str = "mak:agent_in";
const EMIT_AGENT_SPEC_UPDATED: &str = "mak:agent_spec_updated";

pub fn start_mak_observer(mak: &MAK, app: AppHandle) {
    let mut rx = mak.subscribe();

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    handle_event(&app, event).unwrap_or_else(|e| {
                        log::error!("Failed to emit Tauri event: {}", e);
                    });
                }
                Err(RecvError::Lagged(n)) => {
                    log::warn!("MAK event listener lagged by {} events.", n);
                }
                Err(RecvError::Closed) => {
                    break; // Channel closed, exit the loop
                }
            }
        }
    });
}

fn handle_event(app: &AppHandle, event: MAKEvent) -> Result<()> {
    match event {
        MAKEvent::AgentConfigUpdated(agent_id, key, value) => {
            emit_agent_config_updated(app, agent_id, key, value)?;
        }
        MAKEvent::AgentError(agent_id, message) => {
            emit_agent_error(app, agent_id, message)?;
        }
        MAKEvent::AgentIn(agent_id, connection) => {
            emit_agent_in(app, agent_id, connection)?;
        }
        MAKEvent::AgentSpecUpdated(agent_id) => {
            emit_agent_spec_updated(app, agent_id)?;
        }
        MAKEvent::Board(_, _) => {}
    }
    Ok(())
}

fn emit_agent_config_updated(
    app: &AppHandle,
    agent_id: String,
    key: String,
    value: AgentValue,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct AgentConfigUpdatedMessage {
        agent_id: String,
        key: String,
        value: AgentValue,
    }

    app.emit(
        EMIT_AGENT_CONFIG_UPDATED,
        AgentConfigUpdatedMessage {
            agent_id,
            key,
            value,
        },
    )
    .context("Failed to emit agent config updated message")
}

fn emit_agent_error(app: &AppHandle, agent_id: String, message: String) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct AgentErrorMessage {
        agent_id: String,
        message: String,
    }

    app.emit(EMIT_AGENT_ERROR, AgentErrorMessage { agent_id, message })
        .context("Failed to emit agent error message")
}

fn emit_agent_in(app: &AppHandle, agent_id: String, port: String) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct AgentInMessage {
        agent_id: String,
        port: String,
    }

    app.emit(EMIT_AGENT_IN, AgentInMessage { agent_id, port })
        .context("Failed to emit agent-in message")
}

fn emit_agent_spec_updated(app: &AppHandle, agent_id: String) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct AgentSpecUpdatedMessage {
        agent_id: String,
    }

    app.emit(
        EMIT_AGENT_SPEC_UPDATED,
        AgentSpecUpdatedMessage { agent_id },
    )
    .context("Failed to emit agent spec updated message")
}
