use anyhow::{Context as _, Result};
use modular_agent_core::{AgentValue, EventEnvelope, ModularAgent, ModularAgentEvent};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast::error::RecvError;

use crate::modular_agent_desktop::app::parent_preset_path;

const EMIT_AGENT_CONFIG_UPDATED: &str = "ma:agent_config_updated";
const EMIT_AGENT_ERROR: &str = "ma:agent_error";
const EMIT_AGENT_IN: &str = "ma:agent_in";
const EMIT_AGENT_SPEC_UPDATED: &str = "ma:agent_spec_updated";
const EMIT_PRESET_STRUCTURE_CHANGED: &str = "ma:preset_structure_changed";
const EMIT_PRESET_LIST_CHANGED: &str = "ma:preset_list_changed";
const EMIT_PRESET_REMOVED: &str = "ma:preset_removed";
const EMIT_PRESET_RENAMED: &str = "ma:preset_renamed";

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
        ModularAgentEvent::PresetStructureChanged { preset_id } => {
            emit_preset_structure_changed(app, origin, preset_id)?;
        }
        ModularAgentEvent::PresetAdded {
            name: Some(name), ..
        } => {
            // Named presets appear in the sidebar; refresh their parent folder.
            emit_preset_list_changed(app, origin, parent_preset_path(&name))?;
        }
        ModularAgentEvent::PresetRemoved { preset_id, name } => {
            emit_preset_removed(app, origin, preset_id, name)?;
        }
        ModularAgentEvent::PresetRenamed {
            preset_id,
            old_name,
            new_name,
        } => {
            let new_parent = parent_preset_path(&new_name);
            emit_preset_renamed(app, origin.clone(), preset_id, old_name.clone(), new_name)?;
            if let Some(old_name) = old_name {
                let old_parent = parent_preset_path(&old_name);
                if old_parent != new_parent {
                    emit_preset_list_changed(app, origin.clone(), old_parent)?;
                }
            }
            emit_preset_list_changed(app, origin, new_parent)?;
        }
        ModularAgentEvent::PresetSaved { preset_id: _, name } => {
            emit_preset_list_changed(app, origin, parent_preset_path(&name))?;
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

fn emit_preset_structure_changed(
    app: &AppHandle,
    origin: Option<String>,
    preset_id: String,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct PresetStructureChangedMessage {
        origin: Option<String>,
        preset_id: String,
    }

    app.emit(
        EMIT_PRESET_STRUCTURE_CHANGED,
        PresetStructureChangedMessage { origin, preset_id },
    )
    .context("Failed to emit preset structure changed message")
}

fn emit_preset_removed(
    app: &AppHandle,
    origin: Option<String>,
    preset_id: String,
    name: Option<String>,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct PresetRemovedMessage {
        origin: Option<String>,
        preset_id: String,
        name: Option<String>,
    }

    app.emit(
        EMIT_PRESET_REMOVED,
        PresetRemovedMessage {
            origin,
            preset_id,
            name,
        },
    )
    .context("Failed to emit preset removed message")
}

fn emit_preset_renamed(
    app: &AppHandle,
    origin: Option<String>,
    id: String,
    old_name: Option<String>,
    new_name: String,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct PresetRenamedMessage {
        origin: Option<String>,
        id: String,
        #[serde(rename = "oldName")]
        old_name: Option<String>,
        #[serde(rename = "newName")]
        new_name: String,
    }

    app.emit(
        EMIT_PRESET_RENAMED,
        PresetRenamedMessage {
            origin,
            id,
            old_name,
            new_name,
        },
    )
    .context("Failed to emit preset renamed message")
}

fn emit_preset_list_changed(app: &AppHandle, origin: Option<String>, path: String) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct PresetListChangedMessage {
        origin: Option<String>,
        path: String,
    }

    app.emit(
        EMIT_PRESET_LIST_CHANGED,
        PresetListChangedMessage { origin, path },
    )
    .context("Failed to emit preset list changed message")
}
