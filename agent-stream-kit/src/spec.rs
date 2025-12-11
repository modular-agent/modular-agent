use std::ops::Not;
use std::sync::atomic::AtomicUsize;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::FnvIndexMap;
use crate::askit::ASKit;
use crate::config::AgentConfigs;
use crate::definition::{AgentConfigSpecs, AgentDefinition};
use crate::error::AgentError;

pub type AgentStreams = FnvIndexMap<String, AgentStream>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentStream {
    #[serde(skip_serializing_if = "String::is_empty")]
    id: String,

    name: String,

    agents: Vec<AgentSpec>,

    channels: Vec<ChannelSpec>,

    #[serde(flatten)]
    pub extensions: FnvIndexMap<String, Value>,
}

impl AgentStream {
    pub fn new(name: String) -> Self {
        Self {
            id: new_id(),
            name,
            agents: Vec::new(),
            channels: Vec::new(),
            extensions: FnvIndexMap::default(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, new_name: String) {
        self.name = new_name;
    }

    pub fn agents(&self) -> &Vec<AgentSpec> {
        &self.agents
    }

    pub fn add_agent(&mut self, agent: AgentSpec) {
        self.agents.push(agent);
    }

    pub fn remove_agent(&mut self, agent_id: &str) {
        self.agents.retain(|agent| agent.id != agent_id);
    }

    pub fn set_agents(&mut self, agents: Vec<AgentSpec>) {
        self.agents = agents;
    }

    pub fn channels(&self) -> &Vec<ChannelSpec> {
        &self.channels
    }

    pub fn add_channels(&mut self, channel: ChannelSpec) {
        self.channels.push(channel);
    }

    pub fn remove_channel(&mut self, channel_id: &str) -> Option<ChannelSpec> {
        if let Some(channel) = self
            .channels
            .iter()
            .find(|channel| channel.id == channel_id)
            .cloned()
        {
            self.channels.retain(|e| e.id != channel_id);
            Some(channel)
        } else {
            None
        }
    }

    pub fn set_channels(&mut self, channels: Vec<ChannelSpec>) {
        self.channels = channels;
    }

    pub async fn start(&self, askit: &ASKit) -> Result<(), AgentError> {
        for agent in self.agents.iter() {
            if !agent.enabled {
                continue;
            }
            askit.start_agent(&agent.id).await.unwrap_or_else(|e| {
                log::error!("Failed to start agent {}: {}", agent.id, e);
            });
        }
        Ok(())
    }

    pub async fn stop(&self, askit: &ASKit) -> Result<(), AgentError> {
        for agent in self.agents.iter() {
            if !agent.enabled {
                continue;
            }
            askit.stop_agent(&agent.id).await.unwrap_or_else(|e| {
                log::error!("Failed to stop agent {}: {}", agent.id, e);
            });
        }
        Ok(())
    }

    pub fn disable_all_nodes(&mut self) {
        for node in self.agents.iter_mut() {
            node.enabled = false;
        }
    }

    pub fn to_json(&self) -> Result<String, AgentError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| AgentError::SerializationError(e.to_string()))?;
        Ok(json)
    }

    pub fn from_json(json_str: &str) -> Result<Self, AgentError> {
        let mut stream: AgentStream = serde_json::from_str(json_str)
            .map_err(|e| AgentError::SerializationError(e.to_string()))?;
        stream.id = new_id();
        Ok(stream)
    }
}

pub fn copy_sub_stream(
    agents: &Vec<AgentSpec>,
    channels: &Vec<ChannelSpec>,
) -> (Vec<AgentSpec>, Vec<ChannelSpec>) {
    let mut new_agents = Vec::new();
    let mut agent_id_map = FnvIndexMap::default();
    for agent in agents {
        let new_id = new_id();
        agent_id_map.insert(agent.id.clone(), new_id.clone());
        let mut new_agent = agent.clone();
        new_agent.id = new_id;
        new_agents.push(new_agent);
    }

    let mut new_channels = Vec::new();
    for channel in channels {
        let Some(source) = agent_id_map.get(&channel.source) else {
            continue;
        };
        let Some(target) = agent_id_map.get(&channel.target) else {
            continue;
        };
        let mut new_channel = channel.clone();
        new_channel.id = new_id();
        new_channel.source = source.clone();
        new_channel.target = target.clone();
        new_channels.push(new_channel);
    }

    (new_agents, new_channels)
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

    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub enabled: bool,

    #[serde(flatten)]
    pub extensions: FnvIndexMap<String, serde_json::Value>,
}

impl AgentSpec {
    pub fn from_def(def: &AgentDefinition) -> Self {
        let mut spec = def.to_spec();
        spec.id = new_id();
        spec
    }
}

static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn new_id() -> String {
    return ID_COUNTER
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .to_string();
}

// ChannelSpec

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ChannelSpec {
    pub id: String,
    pub source: String,
    pub source_handle: String,
    pub target: String,
    pub target_handle: String,
}
