use serde::{Deserialize, Serialize};

use crate::FnvIndexMap;
use crate::askit::ASKit;
use crate::error::AgentError;
use crate::id::{new_id, update_ids};
use crate::spec::AgentStreamSpec;

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

    pub fn spec_mut(&mut self) -> &mut AgentStreamSpec {
        &mut self.spec
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
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
