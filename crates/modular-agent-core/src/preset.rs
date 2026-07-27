use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AgentError;
use crate::id::{new_id, update_ids};
use crate::modular_agent::ModularAgent;
use crate::spec::PresetSpec;
use crate::{AgentSpec, ConnectionSpec};

/// A runtime instance of a workflow preset.
///
/// A preset represents a running or runnable workflow, containing agents
/// and their connections. It manages the lifecycle (start/stop) of all
/// agents within the workflow.
pub struct Preset {
    /// Unique identifier for this preset instance.
    id: String,

    /// Optional user-defined name for this preset.
    name: Option<String>,

    /// Whether this preset is currently running.
    running: bool,

    /// The specification containing agents and connections.
    spec: PresetSpec,
}

impl Preset {
    /// Creates a new preset with the given specification.
    ///
    /// All IDs in the spec (agents and connections) are regenerated to ensure uniqueness.
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

    /// Returns the unique identifier of this preset.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns a reference to the preset specification.
    pub fn spec(&self) -> &PresetSpec {
        &self.spec
    }

    /// Updates the preset specification from a JSON value.
    ///
    /// Note: The "agents" and "connections" fields are ignored;
    /// only extension fields are updated.
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

    /// Returns whether this preset is currently running.
    pub fn running(&self) -> bool {
        self.running
    }

    /// Returns the user-defined name of this preset, if set.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Sets the user-defined name of this preset.
    pub fn set_name(&mut self, name: String) {
        self.name = Some(name);
    }

    /// Clears the user-defined name of this preset.
    pub fn clear_name(&mut self) {
        self.name = None;
    }

    /// Adds an agent to this preset.
    pub fn add_agent(&mut self, agent: AgentSpec) {
        self.spec.add_agent(agent);
    }

    /// Removes an agent from this preset by its ID.
    pub fn remove_agent(&mut self, agent_id: &str) {
        self.spec.remove_agent(agent_id);
    }

    /// Adds a connection to this preset.
    pub fn add_connection(&mut self, connection: ConnectionSpec) {
        self.spec.add_connection(connection);
    }

    /// Removes a connection from this preset.
    pub fn remove_connection(&mut self, connection: &ConnectionSpec) -> Option<ConnectionSpec> {
        self.spec.remove_connection(connection)
    }

    /// Starts all enabled agents in this preset.
    ///
    /// If the preset is already running, this method returns immediately.
    /// Disabled agents are skipped.
    pub async fn start(&mut self, ma: &ModularAgent) -> Result<(), AgentError> {
        if self.running {
            // Already running
            return Ok(());
        }
        self.running = true;

        // A previous stop left the preset's parent cancellation token fired;
        // install a fresh one before agents derive their child tokens.
        ma.reset_preset_token(&self.id);

        for agent in self.spec.agents.iter() {
            if agent.disabled {
                continue;
            }
            ma.start_agent(&agent.id).await.unwrap_or_else(|e| {
                log::error!("Failed to start agent {}: {}", agent.id, e);
            });
        }

        Ok(())
    }

    /// Stops all agents in this preset.
    pub async fn stop(&mut self, ma: &ModularAgent) -> Result<(), AgentError> {
        // Cancel every agent's in-flight process() up front so the
        // per-agent stops below are not serialized behind long-running work.
        ma.cancel_preset_token(&self.id);

        for agent in self.spec.agents.iter() {
            ma.stop_agent(&agent.id).await.unwrap_or_else(|e| {
                log::error!("Failed to stop agent {}: {}", agent.id, e);
            });
        }
        // Every agent has stopped; drop the fired parent token so a later
        // start_agent derives a live token instead of a born-cancelled child
        // that would silently skip all inputs.
        ma.remove_preset_token(&self.id);
        self.running = false;
        Ok(())
    }
}

/// Summary information about a preset.
///
/// A lightweight struct containing only essential preset metadata,
/// useful for listing presets without loading full specifications.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresetInfo {
    /// Unique identifier of the preset.
    pub id: String,

    /// User-defined name of the preset, if set.
    pub name: Option<String>,

    /// Whether the preset is currently running.
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
