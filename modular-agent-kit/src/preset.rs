use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AgentError;
use crate::id::{new_id, update_ids};
use crate::mak::MAK;
use crate::spec::PresetSpec;
use crate::{AgentSpec, ConnectionSpec};

pub struct Preset {
    id: String,

    name: Option<String>,

    running: bool,

    spec: PresetSpec,
}

impl Preset {
    /// Create a new preset with the given spec.
    ///
    /// The ids of the given spec, including agents and connections, are changed to new unique ids.
    pub fn new(mut spec: PresetSpec) -> Self {
        let (agents, connections) = update_ids(&spec.agents, &spec.connections);
        spec.agents = agents;
        spec.connections = connections;

        Self {
            id: new_id(),
            name: None,
            running: false,
            spec,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn spec(&self) -> &PresetSpec {
        &self.spec
    }

    pub fn update_spec(&mut self, value: &Value) -> Result<(), AgentError> {
        let update_map = value
            .as_object()
            .ok_or_else(|| AgentError::SerializationError("Expected JSON object".to_string()))?;

        for (k, v) in update_map {
            match k.as_str() {
                "agents" => {
                    // just ignore
                }
                "connections" => {
                    // just ignore
                }
                _ => {
                    // Update extensions
                    self.spec.extensions.insert(k.clone(), v.clone());
                }
            }
        }
        Ok(())
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn set_name(&mut self, name: String) {
        self.name = Some(name);
    }

    pub fn clear_name(&mut self) {
        self.name = None;
    }

    pub fn add_agent(&mut self, agent: AgentSpec) {
        self.spec.add_agent(agent);
    }

    pub fn remove_agent(&mut self, agent_id: &str) {
        self.spec.remove_agent(agent_id);
    }

    pub fn add_connection(&mut self, connection: ConnectionSpec) {
        self.spec.add_connection(connection);
    }

    pub fn remove_connection(&mut self, connection: &ConnectionSpec) -> Option<ConnectionSpec> {
        self.spec.remove_connection(connection)
    }

    pub async fn start(&mut self, mak: &MAK) -> Result<(), AgentError> {
        if self.running {
            // Already running
            return Ok(());
        }
        self.running = true;

        for agent in self.spec.agents.iter() {
            if agent.disabled {
                continue;
            }
            mak.start_agent(&agent.id).await.unwrap_or_else(|e| {
                log::error!("Failed to start agent {}: {}", agent.id, e);
            });
        }

        Ok(())
    }

    pub async fn stop(&mut self, mak: &MAK) -> Result<(), AgentError> {
        for agent in self.spec.agents.iter() {
            mak.stop_agent(&agent.id).await.unwrap_or_else(|e| {
                log::error!("Failed to stop agent {}: {}", agent.id, e);
            });
        }
        self.running = false;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresetInfo {
    pub id: String,
    pub name: Option<String>,
    pub running: bool,
}

impl From<&Preset> for PresetInfo {
    fn from(preset: &Preset) -> Self {
        Self {
            id: preset.id.clone(),
            name: preset.name.clone(),
            running: preset.running,
        }
    }
}
