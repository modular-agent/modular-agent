use std::ops::Not;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::FnvIndexMap;
use crate::config::AgentConfigs;
use crate::definition::AgentConfigSpecs;
use crate::error::AgentError;

/// A map of preset names to their specifications.
pub type PresetSpecs = FnvIndexMap<String, PresetSpec>;

/// The serializable specification of a preset (workflow).
///
/// A preset defines a complete workflow configuration including all agents
/// and their connections. This struct is designed for JSON serialization
/// and can be loaded from or saved to preset files.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PresetSpec {
    /// List of agent specifications in this preset.
    pub agents: Vec<AgentSpec>,

    /// List of connections between agents.
    pub connections: Vec<ConnectionSpec>,

    /// Extension fields for custom data.
    ///
    /// Any JSON fields not matching defined fields are captured here.
    #[serde(flatten)]
    pub extensions: FnvIndexMap<String, Value>,
}

impl PresetSpec {
    /// Adds an agent to this preset.
    pub fn add_agent(&mut self, agent: AgentSpec) {
        self.agents.push(agent);
    }

    /// Removes an agent from this preset by its ID.
    pub fn remove_agent(&mut self, agent_id: &str) {
        self.agents.retain(|agent| agent.id != agent_id);
    }

    /// Adds a connection to this preset.
    pub fn add_connection(&mut self, connection: ConnectionSpec) {
        self.connections.push(connection);
    }

    /// Removes a connection from this preset.
    ///
    /// Returns `Some(ConnectionSpec)` if the connection was found and removed,
    /// or `None` if it was not found.
    pub fn remove_connection(&mut self, connection: &ConnectionSpec) -> Option<ConnectionSpec> {
        let Some(index) = self.connections.iter().position(|c| c == connection) else {
            return None;
        };
        Some(self.connections.remove(index))
    }

    /// Serializes this preset to a pretty-printed JSON string.
    pub fn to_json(&self) -> Result<String, AgentError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| AgentError::SerializationError(e.to_string()))?;
        Ok(json)
    }

    /// Deserializes a preset from a JSON string.
    pub fn from_json(json_str: &str) -> Result<Self, AgentError> {
        let preset: PresetSpec = serde_json::from_str(json_str)
            .map_err(|e| AgentError::SerializationError(e.to_string()))?;
        Ok(preset)
    }
}

/// The runtime specification of an agent instance.
///
/// Contains all the information needed to instantiate and configure an agent,
/// including its ID, definition reference, ports, and configuration values.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentSpec {
    /// Unique identifier for this agent instance.
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub id: String,

    /// Name of the AgentDefinition this agent is based on.
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub def_name: String,

    /// List of input port names (overrides definition if set).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub inputs: Option<Vec<String>>,

    /// List of output port names (overrides definition if set).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub outputs: Option<Vec<String>>,

    /// Configuration values for this agent instance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configs: Option<AgentConfigs>,

    /// Configuration specifications (metadata about configs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_specs: Option<AgentConfigSpecs>,

    /// Whether this agent is disabled (will not be started).
    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub disabled: bool,

    /// Extension fields for custom data.
    #[serde(flatten)]
    pub extensions: FnvIndexMap<String, serde_json::Value>,
}

impl AgentSpec {
    /// Updates this agent spec from a JSON value.
    ///
    /// Known fields (id, def_name, inputs, outputs, configs, disabled) are parsed
    /// and applied. Unknown fields are stored in the extensions map.
    pub fn update(&mut self, value: &Value) -> Result<(), AgentError> {
        let update_map = value
            .as_object()
            .ok_or_else(|| AgentError::SerializationError("Expected JSON object".to_string()))?;

        for (k, v) in update_map {
            match k.as_str() {
                "id" => {
                    if let Some(id_str) = v.as_str() {
                        self.id = id_str.to_string();
                    }
                }
                "def_name" => {
                    if let Some(def_name_str) = v.as_str() {
                        self.def_name = def_name_str.to_string();
                    }
                }
                "inputs" => {
                    if let Some(inputs_array) = v.as_array() {
                        self.inputs = Some(
                            inputs_array
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect(),
                        );
                    }
                }
                "outputs" => {
                    if let Some(outputs_array) = v.as_array() {
                        self.outputs = Some(
                            outputs_array
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect(),
                        );
                    }
                }
                "configs" => {
                    let configs: AgentConfigs = serde_json::from_value(v.clone())
                        .map_err(|e| AgentError::SerializationError(e.to_string()))?;
                    self.configs = Some(configs);
                }
                "disabled" => {
                    if let Some(disabled_bool) = v.as_bool() {
                        self.disabled = disabled_bool;
                    }
                }
                _ => {
                    // Update extensions: null removes the key
                    if v.is_null() {
                        self.extensions.shift_remove(k);
                    } else {
                        self.extensions.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        Ok(())
    }
}

/// A connection between two agent ports.
///
/// Defines a directed edge in the agent graph, connecting an output port
/// of a source agent to an input port of a target agent.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConnectionSpec {
    /// ID of the source agent.
    pub source: String,

    /// Output port name on the source agent.
    pub source_handle: String,

    /// ID of the target agent.
    pub target: String,

    /// Input port name on the target agent.
    pub target_handle: String,
}
