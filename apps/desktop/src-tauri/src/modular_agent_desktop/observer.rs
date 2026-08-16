use anyhow::{Context as _, Result};
use modular_agent_core::{AgentValue, EventEnvelope, ModularAgent, ModularAgentEvent};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast::error::RecvError;

use crate::modular_agent_desktop::app::parent_patch_path;

const EMIT_AGENT_CONFIG_UPDATED: &str = "ma:agent_config_updated";
const EMIT_AGENT_ERROR: &str = "ma:agent_error";
const EMIT_AGENT_IN: &str = "ma:agent_in";
const EMIT_AGENT_SPEC_UPDATED: &str = "ma:agent_spec_updated";
const EMIT_PATCH_STRUCTURE_CHANGED: &str = "ma:patch_structure_changed";
const EMIT_PATCH_LIST_CHANGED: &str = "ma:patch_list_changed";
const EMIT_PATCH_REMOVED: &str = "ma:patch_removed";
const EMIT_PATCH_RENAMED: &str = "ma:patch_renamed";
const EMIT_PATCH_RUNNING_CHANGED: &str = "ma:patch_running_changed";

pub fn start_modular_agent_observer(ma: &ModularAgent, app: AppHandle) {
    let mut rx = ma.subscribe();

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(EventEnvelope { origin, event }) => {
                    let origin = origin.map(|o| o.to_string());
                    handle_event(&app, origin, event).unwrap_or_else(|e| {
                        log::error!("Failed to emit Tauri event: {}", e);
                    });
                }
                Err(RecvError::Lagged(n)) => {
                    log::warn!("ModularAgent event listener lagged by {} events.", n);
                }
                Err(RecvError::Closed) => {
                    break; // Channel closed, exit the loop
                }
            }
        }
    });
}

fn handle_event(app: &AppHandle, origin: Option<String>, event: ModularAgentEvent) -> Result<()> {
    match event {
        ModularAgentEvent::AgentConfigUpdated(agent_id, key, value) => {
            emit_agent_config_updated(app, origin, agent_id, key, value)?;
        }
        ModularAgentEvent::AgentError(agent_id, message) => {
            emit_agent_error(app, origin, agent_id, message)?;
        }
        ModularAgentEvent::AgentIn(agent_id, connection) => {
            emit_agent_in(app, origin, agent_id, connection)?;
        }
        ModularAgentEvent::AgentSpecUpdated(agent_id) => {
            emit_agent_spec_updated(app, origin, agent_id)?;
        }
        ModularAgentEvent::PatchStructureChanged { patch_id } => {
            emit_patch_structure_changed(app, origin, patch_id)?;
        }
        ModularAgentEvent::PatchAdded {
            name: Some(name), ..
        } => {
            // Named patches appear in the sidebar; refresh their parent folder.
            emit_patch_list_changed(app, origin, parent_patch_path(&name))?;
        }
        ModularAgentEvent::PatchRemoved { patch_id, name } => {
            emit_patch_removed(app, origin, patch_id, name)?;
        }
        // Both directions land on one event: the frontend tracks a boolean, not
        // two separate signals.
        ModularAgentEvent::PatchStarted { patch_id } => {
            emit_patch_running_changed(app, origin, patch_id, true)?;
        }
        ModularAgentEvent::PatchStopped { patch_id } => {
            emit_patch_running_changed(app, origin, patch_id, false)?;
        }
        ModularAgentEvent::PatchRenamed {
            patch_id,
            old_name,
            new_name,
        } => {
            let new_parent = parent_patch_path(&new_name);
            emit_patch_renamed(app, origin.clone(), patch_id, old_name.clone(), new_name)?;
            if let Some(old_name) = old_name {
                let old_parent = parent_patch_path(&old_name);
                if old_parent != new_parent {
                    emit_patch_list_changed(app, origin.clone(), old_parent)?;
                }
            }
            emit_patch_list_changed(app, origin, new_parent)?;
        }
        ModularAgentEvent::PatchSaved { patch_id: _, name } => {
            emit_patch_list_changed(app, origin, parent_patch_path(&name))?;
        }
        _ => {}
    }
    Ok(())
}

fn emit_agent_config_updated(
    app: &AppHandle,
    origin: Option<String>,
    agent_id: String,
    key: String,
    value: AgentValue,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct AgentConfigUpdatedMessage {
        origin: Option<String>,
        agent_id: String,
        key: String,
        value: AgentValue,
    }

    app.emit(
        EMIT_AGENT_CONFIG_UPDATED,
        AgentConfigUpdatedMessage {
            origin,
            agent_id,
            key,
            value,
        },
    )
    .context("Failed to emit agent config updated message")
}

fn emit_agent_error(
    app: &AppHandle,
    origin: Option<String>,
    agent_id: String,
    message: String,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct AgentErrorMessage {
        origin: Option<String>,
        agent_id: String,
        message: String,
    }

    app.emit(
        EMIT_AGENT_ERROR,
        AgentErrorMessage {
            origin,
            agent_id,
            message,
        },
    )
    .context("Failed to emit agent error message")
}

fn emit_agent_in(
    app: &AppHandle,
    origin: Option<String>,
    agent_id: String,
    port: String,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct AgentInMessage {
        origin: Option<String>,
        agent_id: String,
        port: String,
    }

    app.emit(
        EMIT_AGENT_IN,
        AgentInMessage {
            origin,
            agent_id,
            port,
        },
    )
    .context("Failed to emit agent-in message")
}

fn emit_agent_spec_updated(
    app: &AppHandle,
    origin: Option<String>,
    agent_id: String,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct AgentSpecUpdatedMessage {
        origin: Option<String>,
        agent_id: String,
    }

    app.emit(
        EMIT_AGENT_SPEC_UPDATED,
        AgentSpecUpdatedMessage { origin, agent_id },
    )
    .context("Failed to emit agent spec updated message")
}

fn emit_patch_structure_changed(
    app: &AppHandle,
    origin: Option<String>,
    patch_id: String,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct PatchStructureChangedMessage {
        origin: Option<String>,
        patch_id: String,
    }

    app.emit(
        EMIT_PATCH_STRUCTURE_CHANGED,
        PatchStructureChangedMessage { origin, patch_id },
    )
    .context("Failed to emit patch structure changed message")
}

fn emit_patch_removed(
    app: &AppHandle,
    origin: Option<String>,
    patch_id: String,
    name: Option<String>,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct PatchRemovedMessage {
        origin: Option<String>,
        patch_id: String,
        name: Option<String>,
    }

    app.emit(
        EMIT_PATCH_REMOVED,
        PatchRemovedMessage {
            origin,
            patch_id,
            name,
        },
    )
    .context("Failed to emit patch removed message")
}

fn emit_patch_renamed(
    app: &AppHandle,
    origin: Option<String>,
    id: String,
    old_name: Option<String>,
    new_name: String,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct PatchRenamedMessage {
        origin: Option<String>,
        id: String,
        #[serde(rename = "oldName")]
        old_name: Option<String>,
        #[serde(rename = "newName")]
        new_name: String,
    }

    app.emit(
        EMIT_PATCH_RENAMED,
        PatchRenamedMessage {
            origin,
            id,
            old_name,
            new_name,
        },
    )
    .context("Failed to emit patch renamed message")
}

fn emit_patch_running_changed(
    app: &AppHandle,
    origin: Option<String>,
    patch_id: String,
    running: bool,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct PatchRunningChangedMessage {
        origin: Option<String>,
        patch_id: String,
        running: bool,
    }

    app.emit(
        EMIT_PATCH_RUNNING_CHANGED,
        PatchRunningChangedMessage {
            origin,
            patch_id,
            running,
        },
    )
    .context("Failed to emit patch running changed message")
}

fn emit_patch_list_changed(app: &AppHandle, origin: Option<String>, path: String) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct PatchListChangedMessage {
        origin: Option<String>,
        path: String,
    }

    app.emit(
        EMIT_PATCH_LIST_CHANGED,
        PatchListChangedMessage { origin, path },
    )
    .context("Failed to emit patch list changed message")
}
