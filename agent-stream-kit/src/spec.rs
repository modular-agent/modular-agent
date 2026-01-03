use std::ops::Not;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::FnvIndexMap;
use crate::config::AgentConfigs;
use crate::definition::AgentConfigSpecs;
use crate::error::AgentError;

pub type AgentStreamSpecs = FnvIndexMap<String, AgentStreamSpec>;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AgentStreamSpec {
    pub agents: im::Vector<AgentSpec>,

    pub channels: im::Vector<ChannelSpec>,

    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub run_on_start: bool,

    #[serde(flatten)]
    pub extensions: im::HashMap<String, Value>,
}

impl AgentStreamSpec {
    pub fn add_agent(&mut self, agent: AgentSpec) {
        self.agents.push_back(agent);
    }

    pub fn remove_agent(&mut self, agent_id: &str) {
        self.agents.retain(|agent| agent.id != agent_id);
    }

    pub fn add_channels(&mut self, channel: ChannelSpec) {
        self.channels.push_back(channel);
    }

    pub fn remove_channel(&mut self, channel: &ChannelSpec) -> Option<ChannelSpec> {
        let Some(index) = self.channels.index_of(channel) else {
            return None;
        };
        Some(self.channels.remove(index))
    }

    pub fn to_json(&self) -> Result<String, AgentError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| AgentError::SerializationError(e.to_string()))?;
        Ok(json)
    }

    pub fn from_json(json_str: &str) -> Result<Self, AgentError> {
        let stream: AgentStreamSpec = serde_json::from_str(json_str)
            .map_err(|e| AgentError::SerializationError(e.to_string()))?;
        Ok(stream)
    }
}

/// Information held by each agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub id: String,

    /// Name of the AgentDefinition.
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub def_name: String,

    /// List of input pin names.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub inputs: Option<Vec<String>>,

    /// List of output pin names.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub outputs: Option<Vec<String>>,

    /// Config values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configs: Option<AgentConfigs>,

    /// Config specs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_specs: Option<AgentConfigSpecs>,

    #[deprecated(note = "Use `disabled` instead")]
    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub enabled: bool,

    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub disabled: bool,

    #[serde(flatten)]
    pub extensions: FnvIndexMap<String, serde_json::Value>,
}

// ChannelSpec

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChannelSpec {
    pub source: String,
    pub source_handle: String,
    pub target: String,
    pub target_handle: String,
}
