use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AgentError;
use crate::id::{new_id, update_ids};
use crate::modular_agent::ModularAgent;
use crate::spec::PatchSpec;
use crate::{AgentSpec, ConnectionSpec};

/// A runtime instance of a workflow patch.
///
/// A patch represents a running or runnable workflow, containing agents
/// and their connections. It manages the lifecycle (start/stop) of all
/// agents within the workflow.
pub struct Patch {
    /// Unique identifier for this patch instance.
    id: String,

    /// Optional user-defined name for this patch.
    name: Option<String>,

    /// Whether this patch is currently running.
    running: bool,

    /// The specification containing agents and connections.
    spec: PatchSpec,
}

impl Patch {
    /// Creates a new patch with the given specification.
    ///
    /// All IDs in the spec (agents and connections) are regenerated to ensure uniqueness.
    pub fn new(mut spec: PatchSpec) -> Self {
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

    /// Returns the unique identifier of this patch.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns a reference to the patch specification.
    pub fn spec(&self) -> &PatchSpec {
        &self.spec
    }

    /// Updates the patch specification from a JSON value.
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

    /// Returns whether this patch is currently running.
    pub fn running(&self) -> bool {
        self.running
    }

    /// Returns the user-defined name of this patch, if set.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Sets the user-defined name of this patch.
    pub fn set_name(&mut self, name: String) {
        self.name = Some(name);
    }

    /// Clears the user-defined name of this patch.
    pub fn clear_name(&mut self) {
        self.name = None;
    }

    /// Adds an agent to this patch.
    pub fn add_agent(&mut self, agent: AgentSpec) {
        self.spec.add_agent(agent);
    }

    /// Removes an agent from this patch by its ID.
    pub fn remove_agent(&mut self, agent_id: &str) {
        self.spec.remove_agent(agent_id);
    }

    /// Applies a JSON patch to the stored spec entry of an agent.
    ///
    /// This is for spec-only agents (whose definition is not registered in
    /// this build), which have no live instance to receive the patch.
    /// Returns `Ok(false)` when the agent is not part of this patch spec.
    pub fn update_agent_spec(&mut self, agent_id: &str, value: &Value) -> Result<bool, AgentError> {
        let Some(agent) = self.spec.agents.iter_mut().find(|a| a.id == agent_id) else {
            return Ok(false);
        };
        agent.update(value)?;
        Ok(true)
    }

    /// Adds a connection to this patch.
    pub fn add_connection(&mut self, connection: ConnectionSpec) {
        self.spec.add_connection(connection);
    }

    /// Removes a connection from this patch.
    pub fn remove_connection(&mut self, connection: &ConnectionSpec) -> Option<ConnectionSpec> {
        self.spec.remove_connection(connection)
    }

    /// Starts all enabled agents in this patch.
    ///
    /// If the patch is already running, this method returns immediately.
    /// Disabled agents are skipped.
    pub async fn start(&mut self, ma: &ModularAgent) -> Result<(), AgentError> {
        if self.running {
            // Already running
            return Ok(());
        }
        self.running = true;

        // A previous stop left the patch's parent cancellation token fired;
        // install a fresh one before agents derive their child tokens.
        ma.reset_patch_token(&self.id);

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

    /// Stops all agents in this patch.
    pub async fn stop(&mut self, ma: &ModularAgent) -> Result<(), AgentError> {
        // Cancel every agent's in-flight process() up front so the
        // per-agent stops below are not serialized behind long-running work.
        ma.cancel_patch_token(&self.id);

        for agent in self.spec.agents.iter() {
            ma.stop_agent(&agent.id).await.unwrap_or_else(|e| {
                log::error!("Failed to stop agent {}: {}", agent.id, e);
            });
        }
        // Every agent has stopped; drop the fired parent token so a later
        // start_agent derives a live token instead of a born-cancelled child
        // that would silently skip all inputs.
        ma.remove_patch_token(&self.id);
        self.running = false;
        Ok(())
    }
}

/// Summary information about a patch.
///
/// A lightweight struct containing only essential patch metadata,
/// useful for listing patches without loading full specifications.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatchInfo {
    /// Unique identifier of the patch.
    pub id: String,

    /// User-defined name of the patch, if set.
    pub name: Option<String>,

    /// Whether the patch is currently running.
    pub running: bool,
}

impl From<&Patch> for PatchInfo {
    fn from(patch: &Patch) -> Self {
        Self {
            id: patch.id.clone(),
            name: patch.name.clone(),
            running: patch.running,
        }
    }
}
