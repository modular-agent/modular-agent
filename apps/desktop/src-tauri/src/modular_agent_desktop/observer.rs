use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use modular_agent_core::{EventEnvelope, ModularAgent, ModularAgentEvent, ModuleStatus, Value};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast::error::RecvError;

use crate::modular_agent_desktop::app::parent_patch_path;

const EMIT_MODULE_CONFIG_UPDATED: &str = "ma:module_config_updated";
const EMIT_MODULE_ERROR: &str = "ma:module_error";
const EMIT_MODULE_IN: &str = "ma:module_in";
const EMIT_MODULE_SPEC_UPDATED: &str = "ma:module_spec_updated";
const EMIT_MODULE_STATUS_CHANGED: &str = "ma:module_status_changed";
const EMIT_PATCH_STRUCTURE_CHANGED: &str = "ma:patch_structure_changed";
const EMIT_PATCH_LIST_CHANGED: &str = "ma:patch_list_changed";
const EMIT_PATCH_REMOVED: &str = "ma:patch_removed";
const EMIT_PATCH_RENAMED: &str = "ma:patch_renamed";
const EMIT_PATCH_RUNNING_CHANGED: &str = "ma:patch_running_changed";

/// Config updates carry their value across the IPC boundary, so a wire
/// driving a config at high frequency would flood the webview with
/// serialization work. Relay them with a leading + trailing throttle per
/// (module_id, key): an idle key emits immediately, later events within the
/// window are coalesced and the latest one is flushed at the window's end.
/// Best-effort — the broadcast receiver above can still drop events under
/// extreme lag before the throttle ever sees them.
const CONFIG_UPDATE_THROTTLE: Duration = Duration::from_millis(100);

struct ConfigThrottleState {
    last_emit: Instant,
    /// Latest coalesced event, kept with its own origin: origins must not be
    /// mixed across coalesced events, or the frontend's origin filter could
    /// drop the trailing value (e.g. a wire value flushed under a "desktop"
    /// echo's origin).
    pending: Option<(Option<String>, Value)>,
    flush_scheduled: bool,
}

type ConfigThrottleMap = Arc<Mutex<HashMap<(String, String), ConfigThrottleState>>>;

pub fn start_modular_agent_observer(ma: &ModularAgent, app: AppHandle) {
    let mut rx = ma.subscribe();
    let throttle: ConfigThrottleMap = Arc::new(Mutex::new(HashMap::new()));

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(EventEnvelope { origin, event }) => {
                    let origin = origin.map(|o| o.to_string());
                    handle_event(&app, &throttle, origin, event).unwrap_or_else(|e| {
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

fn handle_event(
    app: &AppHandle,
    throttle: &ConfigThrottleMap,
    origin: Option<String>,
    event: ModularAgentEvent,
) -> Result<()> {
    match event {
        ModularAgentEvent::ModuleConfigUpdated(module_id, key, value) => {
            throttled_module_config_updated(app, throttle, origin, module_id, key, value)?;
        }
        ModularAgentEvent::ModuleError(module_id, message) => {
            emit_module_error(app, origin, module_id, message)?;
        }
        ModularAgentEvent::ModuleIn(module_id, connection) => {
            emit_module_in(app, origin, module_id, connection)?;
        }
        ModularAgentEvent::ModuleSpecUpdated(module_id) => {
            emit_module_spec_updated(app, origin, module_id)?;
        }
        ModularAgentEvent::ModuleStatusChanged { module_id, status } => {
            emit_module_status_changed(app, origin, module_id, status)?;
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
            // Drop accumulated throttle state. The observer can't map module
            // ids to patches, so clear everything — losing state only means
            // the next event for a key emits immediately.
            throttle.lock().unwrap().clear();
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

fn throttled_module_config_updated(
    app: &AppHandle,
    throttle: &ConfigThrottleMap,
    origin: Option<String>,
    module_id: String,
    key: String,
    value: Value,
) -> Result<()> {
    let now = Instant::now();
    let emit_now = {
        let mut map = throttle.lock().unwrap();
        match map.entry((module_id.clone(), key.clone())) {
            Entry::Vacant(entry) => {
                entry.insert(ConfigThrottleState {
                    last_emit: now,
                    pending: None,
                    flush_scheduled: false,
                });
                Some((origin, value))
            }
            Entry::Occupied(mut entry) => {
                let state = entry.get_mut();
                if !state.flush_scheduled
                    && now.duration_since(state.last_emit) >= CONFIG_UPDATE_THROTTLE
                {
                    state.last_emit = now;
                    Some((origin, value))
                } else {
                    state.pending = Some((origin, value));
                    if !state.flush_scheduled {
                        state.flush_scheduled = true;
                        let delay = (state.last_emit + CONFIG_UPDATE_THROTTLE)
                            .saturating_duration_since(now);
                        spawn_config_flush(
                            app.clone(),
                            throttle.clone(),
                            module_id.clone(),
                            key.clone(),
                            delay,
                        );
                    }
                    None
                }
            }
        }
    };
    if let Some((origin, value)) = emit_now {
        emit_module_config_updated(app, origin, module_id, key, value)?;
    }
    Ok(())
}

fn spawn_config_flush(
    app: AppHandle,
    throttle: ConfigThrottleMap,
    module_id: String,
    key: String,
    delay: Duration,
) {
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        let pending = {
            let mut map = throttle.lock().unwrap();
            // Entry gone: the map was cleared on patch removal.
            let Some(state) = map.get_mut(&(module_id.clone(), key.clone())) else {
                return;
            };
            state.flush_scheduled = false;
            state.last_emit = Instant::now();
            state.pending.take()
        };
        if let Some((origin, value)) = pending {
            emit_module_config_updated(&app, origin, module_id, key, value).unwrap_or_else(|e| {
                log::error!("Failed to emit Tauri event: {}", e);
            });
        }
    });
}

fn emit_module_config_updated(
    app: &AppHandle,
    origin: Option<String>,
    module_id: String,
    key: String,
    value: Value,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct ModuleConfigUpdatedMessage {
        origin: Option<String>,
        module_id: String,
        key: String,
        value: Value,
    }

    app.emit(
        EMIT_MODULE_CONFIG_UPDATED,
        ModuleConfigUpdatedMessage {
            origin,
            module_id,
            key,
            value,
        },
    )
    .context("Failed to emit module config updated message")
}

fn emit_module_error(
    app: &AppHandle,
    origin: Option<String>,
    module_id: String,
    message: String,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct ModuleErrorMessage {
        origin: Option<String>,
        module_id: String,
        message: String,
    }

    app.emit(
        EMIT_MODULE_ERROR,
        ModuleErrorMessage {
            origin,
            module_id,
            message,
        },
    )
    .context("Failed to emit module error message")
}

fn emit_module_status_changed(
    app: &AppHandle,
    origin: Option<String>,
    module_id: String,
    status: ModuleStatus,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct ModuleStatusChangedMessage {
        origin: Option<String>,
        module_id: String,
        status: ModuleStatus,
    }

    app.emit(
        EMIT_MODULE_STATUS_CHANGED,
        ModuleStatusChangedMessage {
            origin,
            module_id,
            status,
        },
    )
    .context("Failed to emit module status changed message")
}

fn emit_module_in(
    app: &AppHandle,
    origin: Option<String>,
    module_id: String,
    port: String,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct ModuleInMessage {
        origin: Option<String>,
        module_id: String,
        port: String,
    }

    app.emit(
        EMIT_MODULE_IN,
        ModuleInMessage {
            origin,
            module_id,
            port,
        },
    )
    .context("Failed to emit module-in message")
}

fn emit_module_spec_updated(
    app: &AppHandle,
    origin: Option<String>,
    module_id: String,
) -> Result<()> {
    #[derive(Clone, Serialize)]
    struct ModuleSpecUpdatedMessage {
        origin: Option<String>,
        module_id: String,
    }

    app.emit(
        EMIT_MODULE_SPEC_UPDATED,
        ModuleSpecUpdatedMessage { origin, module_id },
    )
    .context("Failed to emit module spec updated message")
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
