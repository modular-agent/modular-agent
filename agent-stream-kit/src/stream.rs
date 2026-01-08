use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::askit::ASKit;
use crate::error::AgentError;
use crate::id::{new_id, update_ids};
use crate::spec::AgentStreamSpec;
use crate::{AgentSpec, ChannelSpec, FnvIndexMap};

pub type AgentStreams = FnvIndexMap<String, AgentStream>;

pub struct AgentStream {
    id: String,

    name: String,

    running: bool,

    spec: AgentStreamSpec,
}

impl AgentStream {
    /// Create a new agent stream with the given name and spec.
    ///
    /// The ids of the given spec, including agents and channels, are changed to new unique ids.
    pub fn new(name: String, mut spec: AgentStreamSpec) -> Self {
        let (agents, channels) = update_ids(&spec.agents, &spec.channels);
        spec.agents = agents;
        spec.channels = channels;

        Self {
            id: new_id(),
            name,
            running: false,
            spec,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn spec(&self) -> &AgentStreamSpec {
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
                "channels" => {
                    // just ignore
                }
                "run_on_start" => {
                    if let Some(run_on_start_bool) = v.as_bool() {
                        self.spec.run_on_start = run_on_start_bool;
                    }
                }
                _ => {
                    // Update extensions
                    self.spec.extensions.insert(k.clone(), v.clone());
                }
            }
        }
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn add_agent(&mut self, agent: AgentSpec) {
        self.spec.add_agent(agent);
    }

    pub fn remove_agent(&mut self, agent_id: &str) {
        self.spec.remove_agent(agent_id);
    }

    pub fn add_channel(&mut self, channel: ChannelSpec) {
        self.spec.add_channel(channel);
    }

    pub fn remove_channel(&mut self, channel: &ChannelSpec) -> Option<ChannelSpec> {
        self.spec.remove_channel(channel)
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub async fn start(&mut self, askit: &ASKit) -> Result<(), AgentError> {
        if self.running {
            // Already running
            return Ok(());
        }
        self.running = true;

        for agent in self.spec.agents.iter() {
            if agent.disabled {
                continue;
            }
            askit.start_agent(&agent.id).await.unwrap_or_else(|e| {
                log::error!("Failed to start agent {}: {}", agent.id, e);
            });
        }

        Ok(())
    }

    pub async fn stop(&mut self, askit: &ASKit) -> Result<(), AgentError> {
        for agent in self.spec.agents.iter() {
            askit.stop_agent(&agent.id).await.unwrap_or_else(|e| {
                log::error!("Failed to stop agent {}: {}", agent.id, e);
            });
        }
        self.running = false;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentStreamInfo {
    pub id: String,
    pub name: String,
    pub running: bool,
    pub run_on_start: bool,
}

impl From<&AgentStream> for AgentStreamInfo {
    fn from(stream: &AgentStream) -> Self {
        Self {
            id: stream.id.clone(),
            name: stream.name.clone(),
            running: stream.running,
            run_on_start: stream.spec.run_on_start,
        }
    }
}
