//! External and local I/O agents for the agent network.
//!
//! This module provides agents that bridge external input/output with the internal
//! agent network through named channels.
//!
//! # External I/O Overview
//!
//! External agents provide named channels for external communication:
//!
//! ```text
//! External Input                         Agent Network                        External Output
//!       │                                                                           ▲
//!       │  write_external_input("input", value)                                     │
//!       ▼                                                                           │
//! ┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐      │
//! │ ExtInput    │────▶│   Agent A   │────▶│   Agent B   │────▶│ ExtOutput   │──────┘
//! │ (ExtIn->)   │     │             │     │             │     │ (->ExtOut)  │
//! │ name="input"│     └─────────────┘     └─────────────┘     │ name="output│
//! └─────────────┘                                             └─────────────┘
//!                                                                    │
//!                                                                    ▼
//!                                               ModularAgentEvent::ExternalOutput("output", value)
//! ```
//!
//! # Agent Types
//!
//! ## External Agents (Global scope)
//!
//! - [`ExternalInputAgent`] (`ExtIn->`): Entry point for external input. Listens to
//!   [`ModularAgent::write_external_input`](crate::ModularAgent::write_external_input) calls
//!   and forwards values to connected agents.
//!
//! - [`ExternalOutputAgent`] (`->ExtOut`): Exit point for external output. When it receives
//!   a value, it broadcasts to the named channel, triggering a
//!   [`ModularAgentEvent::ExternalOutput`](crate::ModularAgentEvent::ExternalOutput) event.
//!
//! ## Local Agents (Patch scope)
//!
//! - [`LocalInputAgent`] (`LocalIn->`): Similar to `ExternalInputAgent`, but scoped to the patch.
//!
//! - [`LocalOutputAgent`] (`->LocalOut`): Similar to `ExternalOutputAgent`, but scoped to the patch.
//!
//! # Patch Example
//!
//! ```json
//! {
//!   "agents": [
//!     {
//!       "id": "in",
//!       "def_name": "modular_agent_core::external_agent::ExternalInputAgent",
//!       "outputs": ["value"],
//!       "configs": { "name": "input" }
//!     },
//!     {
//!       "id": "out",
//!       "def_name": "modular_agent_core::external_agent::ExternalOutputAgent",
//!       "inputs": ["value"],
//!       "configs": { "name": "output" }
//!     }
//!   ],
//!   "connections": [
//!     { "source": "in", "source_handle": "value", "target": "out", "target_handle": "value" }
//!   ]
//! }
//! ```

use std::vec;

use async_trait::async_trait;

use modular_agent_macros::modular_agent;

use crate::agent::{Agent, AgentData, AgentStatus, AsAgent};
use crate::context::AgentContext;
use crate::error::AgentError;
use crate::modular_agent::ModularAgent;
use crate::spec::AgentSpec;
use crate::value::AgentValue;

const CATEGORY: &str = "Core/IO";

const PORT_VALUE: &str = "value";

const CONFIG_NAME: &str = "name";

/// Receives values FROM connected agents and outputs them externally.
///
/// When this agent receives a value on its input port, it broadcasts the value
/// to the named channel, which:
/// 1. Stores the value in the channel's value cache
/// 2. Emits a [`ModularAgentEvent::ExternalOutput`](crate::ModularAgentEvent::ExternalOutput) event
/// 3. Forwards the value to any [`ExternalInputAgent`] instances listening to the same channel
///
/// # Configuration
///
/// - `name`: The channel name to write to (required)
///
/// # Data Flow
///
/// ```text
/// Agent Output ──▶ ExternalOutputAgent ──▶ Channel "output" ──▶ ModularAgentEvent::ExternalOutput
/// ```
#[modular_agent(
    kind = "External",
    title = "->ExtOut",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    string_config(
        name = CONFIG_NAME,
    )
)]
struct ExternalOutputAgent {
    data: AgentData,
    channel_name: Option<String>,
}

#[async_trait]
impl AsAgent for ExternalOutputAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let channel_name = spec
            .configs
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_NAME).ok());
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            channel_name,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        self.channel_name = self.configs()?.get_string(CONFIG_NAME).ok();
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let channel_name = self.channel_name.clone().unwrap_or_default();
        if channel_name.is_empty() {
            // if channel_name is not set, stop processing
            return Ok(());
        }
        let ma = self.ma();
        ma.send_external_output(channel_name.clone(), ctx, value.clone())
            .await?;

        Ok(())
    }
}

/// Receives external input and outputs values TO connected agents.
///
/// This agent is the entry point for external input into the agent network.
/// When [`ModularAgent::write_external_input`](crate::ModularAgent::write_external_input)
/// is called with a matching channel name, this agent receives the value and
/// forwards it to all connected agents via its output port.
///
/// # Configuration
///
/// - `name`: The channel name to listen to (required)
///
/// # Data Flow
///
/// ```text
/// write_external_input("input", value) ──▶ ExternalInputAgent ──▶ Connected Agents
/// ```
#[modular_agent(
    kind = "External",
    title = "ExtIn->",
    category = CATEGORY,
    outputs = [PORT_VALUE],
    string_config(
        name = CONFIG_NAME,
    )
)]
struct ExternalInputAgent {
    data: AgentData,
    channel_name: Option<String>,
}

#[async_trait]
impl AsAgent for ExternalInputAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let channel_name = spec
            .configs
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_NAME).ok());
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            channel_name,
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        if let Some(channel_name) = &self.channel_name {
            let ma = self.ma();
            let mut external_input_agents = ma.external_input_agents.lock();
            if let Some(nodes) = external_input_agents.get_mut(channel_name) {
                nodes.push(self.data.id.clone());
            } else {
                external_input_agents.insert(channel_name.clone(), vec![self.data.id.clone()]);
            }
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        if let Some(channel_name) = &self.channel_name {
            let ma = self.ma();
            let mut external_input_agents = ma.external_input_agents.lock();
            if let Some(nodes) = external_input_agents.get_mut(channel_name) {
                nodes.retain(|x| x != &self.data.id);
            }
        }
        Ok(())
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let channel_name = self.configs()?.get_string(CONFIG_NAME).ok();
        if self.channel_name != channel_name {
            // Re-point the registration only while running: start() registers
            // and stop() unregisters, so touching the map on a stopped agent
            // would leave a duplicate entry once start() runs, delivering
            // every external input twice.
            if self.data.status == AgentStatus::Start {
                if let Some(channel_name) = &self.channel_name {
                    let ma = self.ma();
                    let mut external_input_agents = ma.external_input_agents.lock();
                    if let Some(nodes) = external_input_agents.get_mut(channel_name) {
                        nodes.retain(|x| x != &self.data.id);
                    }
                }
                if let Some(channel_name) = &channel_name {
                    let ma = self.ma();
                    let mut external_input_agents = ma.external_input_agents.lock();
                    if let Some(nodes) = external_input_agents.get_mut(channel_name) {
                        nodes.push(self.data.id.clone());
                    } else {
                        external_input_agents
                            .insert(channel_name.clone(), vec![self.data.id.clone()]);
                    }
                }
            }
            self.channel_name = channel_name;
        }
        Ok(())
    }
}

/// Receives values FROM connected agents and outputs them to a patch-scoped local variable.
///
/// Similar to [`ExternalOutputAgent`], but the channel name is scoped to the patch,
/// using the format `%{patch_id}/{var_name}`. This allows variables to be
/// isolated between different patch instances.
///
/// # Configuration
///
/// - `name`: The variable name (required)
#[modular_agent(
    kind = "Local",
    title = "->LocalOut",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    string_config(
        name = CONFIG_NAME,
    )
)]
struct LocalOutputAgent {
    data: AgentData,
    var_name: Option<String>,
}

#[async_trait]
impl AsAgent for LocalOutputAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let var_name = spec
            .configs
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_NAME).ok());
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            var_name,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        self.var_name = self.configs()?.get_string(CONFIG_NAME).ok();
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let var_name = self.var_name.clone().unwrap_or_default();
        if var_name.is_empty() {
            // if var_name is not set, stop processing
            return Ok(());
        }
        let channel_name = channel_name_for_local(self.patch_id(), &var_name);
        let ma = self.ma();
        ma.send_external_output(channel_name.clone(), ctx, value.clone())
            .await?;

        Ok(())
    }
}

/// Receives values FROM a patch-scoped local variable and outputs them TO connected agents.
///
/// Similar to [`ExternalInputAgent`], but the channel name is scoped to the patch,
/// using the format `%{patch_id}/{var_name}`. This allows variables to be
/// isolated between different patch instances.
///
/// # Configuration
///
/// - `name`: The variable name (required)
#[modular_agent(
    kind = "Local",
    title = "LocalIn->",
    category = CATEGORY,
    outputs = [PORT_VALUE],
    string_config(
        name = CONFIG_NAME,
    )
)]
struct LocalInputAgent {
    data: AgentData,
    var_name: Option<String>,
}

#[async_trait]
impl AsAgent for LocalInputAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let var_name = spec
            .configs
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_NAME).ok());
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            var_name,
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        if let Some(var_name) = &self.var_name {
            let channel_name = channel_name_for_local(self.patch_id(), var_name);
            let ma = self.ma();
            let mut external_input_agents = ma.external_input_agents.lock();
            if let Some(nodes) = external_input_agents.get_mut(&channel_name) {
                nodes.push(self.data.id.clone());
            } else {
                external_input_agents.insert(channel_name.clone(), vec![self.data.id.clone()]);
            }
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        if let Some(var_name) = &self.var_name {
            let channel_name = channel_name_for_local(self.patch_id(), var_name);
            let ma = self.ma();
            let mut external_input_agents = ma.external_input_agents.lock();
            if let Some(nodes) = external_input_agents.get_mut(&channel_name) {
                nodes.retain(|x| x != &self.data.id);
            }
        }
        Ok(())
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let new_var_name = self.configs()?.get_string(CONFIG_NAME).ok();
        if self.var_name != new_var_name {
            // Same as ExternalInputAgent: the registration belongs to the
            // running state, so only re-point it while running.
            if self.data.status == AgentStatus::Start {
                if let Some(var_name) = &self.var_name {
                    let channel_name = channel_name_for_local(self.patch_id(), var_name);
                    let ma = self.ma();
                    let mut external_input_agents = ma.external_input_agents.lock();
                    if let Some(nodes) = external_input_agents.get_mut(&channel_name) {
                        nodes.retain(|x| x != &self.data.id);
                    }
                }
                if let Some(var_name) = &new_var_name {
                    let channel_name = channel_name_for_local(self.patch_id(), var_name);
                    let ma = self.ma();
                    let mut external_input_agents = ma.external_input_agents.lock();
                    if let Some(nodes) = external_input_agents.get_mut(&channel_name) {
                        nodes.push(self.data.id.clone());
                    } else {
                        external_input_agents
                            .insert(channel_name.clone(), vec![self.data.id.clone()]);
                    }
                }
            }
            self.var_name = new_var_name;
        }
        Ok(())
    }
}

fn channel_name_for_local(flow_id: &str, var_name: &str) -> String {
    format!("%{}/{}", flow_id, var_name)
}
