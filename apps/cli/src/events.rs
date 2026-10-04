//! Turns core events into log records tagged with the patch and module, so a
//! log pipeline can filter by them.

use std::collections::HashMap;

use log::Level;
use modular_agent_core::{
    CancellationToken, EventEnvelope, ModularAgent, ModularAgentEvent, ModuleLogLevel, ModuleStatus,
};
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::task::JoinHandle;

/// Log node records; filter them with `RUST_LOG=modular_agent::patch=...`.
const TARGET_PATCH: &str = "modular_agent::patch";
/// Everything else converted from core events.
const TARGET_EVENTS: &str = "modular_agent::events";

/// Logs core events until `stop` is cancelled, then logs whatever is still
/// queued (the stops emitted by the final shutdown) and exits.
pub(crate) fn spawn(ma: &ModularAgent, stop: CancellationToken) -> JoinHandle<()> {
    let mut rx = ma.subscribe();
    let mut logger = EventLogger::new(ma.clone());
    tokio::spawn(async move {
        loop {
            tokio::select! {
                recv = rx.recv() => match recv {
                    Ok(envelope) => logger.handle(envelope).await,
                    Err(RecvError::Lagged(n)) => {
                        log::warn!(target: TARGET_EVENTS, "dropped {n} events: the log fell behind");
                    }
                    Err(RecvError::Closed) => return,
                },
                _ = stop.cancelled() => break,
            }
        }
        loop {
            match rx.try_recv() {
                Ok(envelope) => logger.handle(envelope).await,
                Err(TryRecvError::Lagged(_)) => continue,
                Err(_) => return,
            }
        }
    })
}

struct EventLogger {
    ma: ModularAgent,
    /// patch id -> name (the id for an unnamed patch)
    patches: HashMap<String, String>,
    /// module id -> patch id
    modules: HashMap<String, String>,
    statuses: HashMap<String, ModuleStatus>,
}

impl EventLogger {
    fn new(ma: ModularAgent) -> Self {
        Self {
            ma,
            patches: HashMap::new(),
            modules: HashMap::new(),
            statuses: HashMap::new(),
        }
    }

    async fn handle(&mut self, envelope: EventEnvelope) {
        match envelope.event {
            ModularAgentEvent::PatchAdded { patch_id, .. }
            | ModularAgentEvent::PatchStructureChanged { patch_id }
            | ModularAgentEvent::PatchRenamed { patch_id, .. } => self.index(&patch_id).await,
            ModularAgentEvent::PatchRemoved { patch_id, .. } => {
                self.patches.remove(&patch_id);
                self.modules.retain(|_, p| *p != patch_id);
            }
            ModularAgentEvent::PatchStarted { patch_id } => {
                log::info!(target: TARGET_EVENTS, patch = self.patch_name(&patch_id); "patch started");
            }
            ModularAgentEvent::PatchStopped { patch_id } => {
                log::info!(target: TARGET_EVENTS, patch = self.patch_name(&patch_id); "patch stopped");
            }
            ModularAgentEvent::ModuleError(module_id, message) => {
                log::error!(
                    target: TARGET_EVENTS,
                    patch = self.module_patch(&module_id), module = module_id.as_str();
                    "{message}"
                );
            }
            ModularAgentEvent::ModuleStatusChanged { module_id, status } => {
                let prev = self.statuses.insert(module_id.clone(), status);
                if let Some(message) = status_message(prev, status) {
                    log::info!(
                        target: TARGET_EVENTS,
                        patch = self.module_patch(&module_id), module = module_id.as_str();
                        "{message}"
                    );
                }
            }
            ModularAgentEvent::ModuleLog {
                module_id,
                level,
                message,
            } => {
                log::log!(
                    target: TARGET_PATCH,
                    log_level(level),
                    patch = self.module_patch(&module_id), module = module_id.as_str();
                    "{message}"
                );
            }
            ModularAgentEvent::ExternalOutput(channel, value) => {
                if log::log_enabled!(target: TARGET_EVENTS, Level::Debug) {
                    let value = serde_json::to_string(&value).unwrap_or_default();
                    log::debug!(target: TARGET_EVENTS, channel = channel.as_str(); "{value}");
                }
            }
            ModularAgentEvent::ModuleIn(module_id, port) => {
                log::trace!(
                    target: TARGET_EVENTS,
                    patch = self.module_patch(&module_id), module = module_id.as_str();
                    "input on {port}"
                );
            }
            _ => {}
        }
    }

    /// Records the patch's name and module ids. Takes the patch lock only:
    /// a module emits events while holding its own lock, so waiting on a
    /// module here could stall the log.
    async fn index(&mut self, patch_id: &str) {
        let Some(patch) = self.ma.get_patch(patch_id) else {
            return;
        };
        let (name, module_ids) = {
            let patch = patch.lock().await;
            let ids: Vec<String> = patch.spec().modules.iter().map(|m| m.id.clone()).collect();
            (patch.name().map(str::to_string), ids)
        };
        self.patches.insert(
            patch_id.to_string(),
            name.unwrap_or_else(|| patch_id.to_string()),
        );
        for id in module_ids {
            self.modules.insert(id, patch_id.to_string());
        }
    }

    fn patch_name<'a>(&'a self, patch_id: &'a str) -> &'a str {
        self.patches.get(patch_id).map_or(patch_id, String::as_str)
    }

    fn module_patch(&self, module_id: &str) -> &str {
        self.modules
            .get(module_id)
            .map_or("", |patch_id| self.patch_name(patch_id))
    }
}

/// Module lifecycle wrappers report `Start` when start() begins and `Init`
/// when it fails, `Stop` when stop() begins and `Init` when it completes.
fn status_message(prev: Option<ModuleStatus>, status: ModuleStatus) -> Option<&'static str> {
    match (prev, status) {
        (_, ModuleStatus::Start) => Some("module starting"),
        (Some(ModuleStatus::Start), ModuleStatus::Init) => Some("module failed to start"),
        (Some(ModuleStatus::Stop), ModuleStatus::Init) => Some("module stopped"),
        _ => None,
    }
}

fn log_level(level: ModuleLogLevel) -> Level {
    match level {
        ModuleLogLevel::Error => Level::Error,
        ModuleLogLevel::Warn => Level::Warn,
        ModuleLogLevel::Info => Level::Info,
        ModuleLogLevel::Debug => Level::Debug,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_transitions() {
        use ModuleStatus::*;
        assert_eq!(status_message(None, Start), Some("module starting"));
        assert_eq!(
            status_message(Some(Start), Init),
            Some("module failed to start")
        );
        assert_eq!(status_message(Some(Start), Stop), None);
        assert_eq!(status_message(Some(Stop), Init), Some("module stopped"));
        assert_eq!(status_message(None, Init), None);
    }
}
