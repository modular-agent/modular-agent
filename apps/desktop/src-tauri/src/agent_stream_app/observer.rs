use agent_stream_kit::{ASKitEvent, ASKitObserver, AgentValue};
use anyhow::{Context as _, Result};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

const EMIT_AGENT_CONFIG_UPDATED: &str = "askit:agent_config_updated";
const EMIT_AGENT_ERROR: &str = "askit:agent_error";
const EMIT_AGENT_IN: &str = "askit:agent_in";
const EMIT_AGENT_SPEC_UPDATED: &str = "askit:agent_spec_updated";

#[derive(Clone)]
pub struct ASAppObserver {
    pub app: AppHandle,
}

impl ASAppObserver {
    fn emit_agent_config_updated(
        &self,
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

        self.app
            .emit(
                EMIT_AGENT_CONFIG_UPDATED,
                AgentConfigUpdatedMessage {
                    agent_id,
                    key,
                    value,
                },
            )
            .context("Failed to emit agent config updated message")?;

        Ok(())
    }

    fn emit_agent_error(&self, agent_id: String, message: String) -> Result<()> {
        #[derive(Clone, Serialize)]
        struct AgentErrorMessage {
            agent_id: String,
            message: String,
        }

        self.app
            .emit(EMIT_AGENT_ERROR, AgentErrorMessage { agent_id, message })
            .context("Failed to emit agent error message")?;

        Ok(())
    }

    fn emit_agent_in(&self, agent_id: String, ch: String) -> Result<()> {
        #[derive(Clone, Serialize)]
        struct AgentInMessage {
            agent_id: String,
            ch: String,
        }

        self.app
            .emit(EMIT_AGENT_IN, AgentInMessage { agent_id, ch })
            .context("Failed to emit agent-in message")?;

        Ok(())
    }

    fn emit_agent_spec_updated(&self, agent_id: String) -> Result<()> {
        #[derive(Clone, Serialize)]
        struct AgentSpecUpdatedMessage {
            agent_id: String,
        }

        self.app
            .emit(
                EMIT_AGENT_SPEC_UPDATED,
                AgentSpecUpdatedMessage { agent_id },
            )
            .context("Failed to emit agent spec updated message")?;

        Ok(())
    }
}

impl ASKitObserver for ASAppObserver {
    fn notify(&self, event: &ASKitEvent) {
        match event {
            ASKitEvent::AgentConfigUpdated(agent_id, key, value) => {
                self.emit_agent_config_updated(
                    agent_id.to_string(),
                    key.to_string(),
                    value.clone(),
                )
                .unwrap_or_else(|e| {
                    log::error!("Failed to emit agent config updated message: {}", e);
                });
            }
            ASKitEvent::AgentError(agent_id, message) => {
                self.emit_agent_error(agent_id.to_string(), message.to_string())
                    .unwrap_or_else(|e| {
                        log::error!("Failed to emit agent error message: {}", e);
                    });
            }
            ASKitEvent::AgentIn(agent_id, channel) => {
                self.emit_agent_in(agent_id.to_string(), channel.to_string())
                    .unwrap_or_else(|e| {
                        log::error!("Failed to emit agent-in message: {}", e);
                    });
            }
            ASKitEvent::AgentSpecUpdated(agent_id) => {
                self.emit_agent_spec_updated(agent_id.to_string())
                    .unwrap_or_else(|e| {
                        log::error!("Failed to emit agent spec updated message: {}", e);
                    });
            }
            _ => {}
        }
    }
}
