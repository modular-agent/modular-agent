//! Board agents for external I/O with the agent network.
//!
//! This module provides agents that bridge external input/output with the internal
//! agent network through named "boards".
//!
//! # Board System Overview
//!
//! Boards are named channels for external communication:
//!
//! ```text
//! External Input                         Agent Network                        External Output
//!       │                                                                           ▲
//!       │  write_board_value("input", value)                                        │
//!       ▼                                                                           │
//! ┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐      │
//! │ BoardOut    │────▶│   Agent A   │────▶│   Agent B   │────▶│ BoardIn     │──────┘
//! │ (Board->)   │     │             │     │             │     │ (->Board)   │
//! │ name="input"│     └─────────────┘     └─────────────┘     │ name="output│
//! └─────────────┘                                             └─────────────┘
//!                                                                    │
//!                                                                    ▼
//!                                                    ModularAgentEvent::Board("output", value)
//! ```
//!
//! # Agent Types
//!
//! - [`BoardOutAgent`] (`Board->`): Entry point for external input. Listens to
//!   [`ModularAgent::write_board_value`](crate::ModularAgent::write_board_value) calls
//!   and forwards values to connected agents.
//!
//! - [`BoardInAgent`] (`->Board`): Exit point for external output. When it receives
//!   a value, it broadcasts to the named board, triggering a
//!   [`ModularAgentEvent::Board`](crate::ModularAgentEvent::Board) event.
//!
//! # Preset Example
//!
//! ```json
//! {
//!   "agents": [
//!     {
//!       "id": "in",
//!       "def_name": "modular_agent_core::board_agent::BoardOutAgent",
//!       "outputs": ["value"],
//!       "configs": { "name": "input" }
//!     },
//!     {
//!       "id": "out",
//!       "def_name": "modular_agent_core::board_agent::BoardInAgent",
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

use crate::agent::{Agent, AgentData, AsAgent};
use crate::context::AgentContext;
use crate::error::AgentError;
use crate::modular_agent::ModularAgent;
use crate::spec::AgentSpec;
use crate::value::AgentValue;

const CATEGORY: &str = "Core/Board";

const PORT_VALUE: &str = "value";

const CONFIG_NAME: &str = "name";

/// Receives values INTO a named board FROM connected agents.
///
/// When this agent receives a value on its input port, it broadcasts the value
/// to the named board, which:
/// 1. Stores the value in the board's value cache
/// 2. Emits a [`ModularAgentEvent::Board`](crate::ModularAgentEvent::Board) event
/// 3. Forwards the value to any [`BoardOutAgent`] instances listening to the same board
///
/// # Configuration
///
/// - `name`: The board name to write to (required)
///
/// # Data Flow
///
/// ```text
/// Agent Output ──▶ BoardInAgent ──▶ Board "output" ──▶ ModularAgentEvent::Board
/// ```
#[modular_agent(
    kind = "Board",
    title = "->Board",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    string_config(
        name = CONFIG_NAME,
    )
)]
struct BoardInAgent {
    data: AgentData,
    board_name: Option<String>,
}

#[async_trait]
impl AsAgent for BoardInAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let board_name = spec
            .configs
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_NAME).ok());
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            board_name,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        self.board_name = self.configs()?.get_string(CONFIG_NAME).ok();
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let board_name = self.board_name.clone().unwrap_or_default();
        if board_name.is_empty() {
            // if board_name is not set, stop processing
            return Ok(());
        }
        let ma = self.ma();
        ma.send_board_out(board_name.clone(), ctx, value.clone())
            .await?;

        Ok(())
    }
}

/// Outputs values FROM a named board TO connected agents.
///
/// This agent is the entry point for external input into the agent network.
/// When [`ModularAgent::write_board_value`](crate::ModularAgent::write_board_value)
/// is called with a matching board name, this agent receives the value and
/// forwards it to all connected agents via its output port.
///
/// # Configuration
///
/// - `name`: The board name to listen to (required)
///
/// # Data Flow
///
/// ```text
/// write_board_value("input", value) ──▶ BoardOutAgent ──▶ Connected Agents
/// ```
///
/// # Note on Naming
///
/// The name "BoardOutAgent" refers to data flowing OUT of the board system
/// INTO the agent network. Think of it as "Board -> Agents".
#[modular_agent(
    kind = "Board",
    title = "Board->",
    category = CATEGORY,
    outputs = [PORT_VALUE],
    string_config(
        name = CONFIG_NAME,
    )
)]
struct BoardOutAgent {
    data: AgentData,
    board_name: Option<String>,
}

#[async_trait]
impl AsAgent for BoardOutAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let board_name = spec
            .configs
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_NAME).ok());
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            board_name,
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        if let Some(board_name) = &self.board_name {
            let ma = self.ma();
            let mut board_out_agents = ma.board_out_agents.lock().unwrap();
            if let Some(nodes) = board_out_agents.get_mut(board_name) {
                nodes.push(self.data.id.clone());
            } else {
                board_out_agents.insert(board_name.clone(), vec![self.data.id.clone()]);
            }
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        if let Some(board_name) = &self.board_name {
            let ma = self.ma();
            let mut board_out_agents = ma.board_out_agents.lock().unwrap();
            if let Some(nodes) = board_out_agents.get_mut(board_name) {
                nodes.retain(|x| x != &self.data.id);
            }
        }
        Ok(())
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let board_name = self.configs()?.get_string(CONFIG_NAME).ok();
        if self.board_name != board_name {
            if let Some(board_name) = &self.board_name {
                let ma = self.ma();
                let mut board_out_agents = ma.board_out_agents.lock().unwrap();
                if let Some(nodes) = board_out_agents.get_mut(board_name) {
                    nodes.retain(|x| x != &self.data.id);
                }
            }
            if let Some(board_name) = &board_name {
                let ma = self.ma();
                let mut board_out_agents = ma.board_out_agents.lock().unwrap();
                if let Some(nodes) = board_out_agents.get_mut(board_name) {
                    nodes.push(self.data.id.clone());
                } else {
                    board_out_agents.insert(board_name.clone(), vec![self.data.id.clone()]);
                }
            }
            self.board_name = board_name;
        }
        Ok(())
    }
}

/// Receives values INTO a preset-scoped variable.
///
/// Similar to [`BoardInAgent`], but the board name is scoped to the preset,
/// using the format `%{preset_id}/{var_name}`. This allows variables to be
/// isolated between different preset instances.
///
/// # Configuration
///
/// - `name`: The variable name (required)
#[modular_agent(
    kind = "Board",
    title = "->Var",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    string_config(
        name = CONFIG_NAME,
    )
)]
struct VarInAgent {
    data: AgentData,
    var_name: Option<String>,
}

#[async_trait]
impl AsAgent for VarInAgent {
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
        let board_name = board_name_for_var(self.preset_id(), &var_name);
        let ma = self.ma();
        ma.send_board_out(board_name.clone(), ctx, value.clone())
            .await?;

        Ok(())
    }
}

/// Outputs values FROM a preset-scoped variable TO connected agents.
///
/// Similar to [`BoardOutAgent`], but the board name is scoped to the preset,
/// using the format `%{preset_id}/{var_name}`. This allows variables to be
/// isolated between different preset instances.
///
/// # Configuration
///
/// - `name`: The variable name (required)
#[modular_agent(
    kind = "Board",
    title = "Var->",
    category = CATEGORY,
    outputs = [PORT_VALUE],
    string_config(
        name = CONFIG_NAME,
    )
)]
struct VarOutAgent {
    data: AgentData,
    var_name: Option<String>,
}

#[async_trait]
impl AsAgent for VarOutAgent {
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
            let board_name = board_name_for_var(self.preset_id(), var_name);
            let ma = self.ma();
            let mut board_out_agents = ma.board_out_agents.lock().unwrap();
            if let Some(nodes) = board_out_agents.get_mut(&board_name) {
                nodes.push(self.data.id.clone());
            } else {
                board_out_agents.insert(board_name.clone(), vec![self.data.id.clone()]);
            }
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        if let Some(var_name) = &self.var_name {
            let board_name = board_name_for_var(self.preset_id(), var_name);
            let ma = self.ma();
            let mut board_out_agents = ma.board_out_agents.lock().unwrap();
            if let Some(nodes) = board_out_agents.get_mut(&board_name) {
                nodes.retain(|x| x != &self.data.id);
            }
        }
        Ok(())
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let new_var_name = self.configs()?.get_string(CONFIG_NAME).ok();
        if self.var_name != new_var_name {
            if let Some(var_name) = &self.var_name {
                let board_name = board_name_for_var(self.preset_id(), var_name);
                let ma = self.ma();
                let mut board_out_agents = ma.board_out_agents.lock().unwrap();
                if let Some(nodes) = board_out_agents.get_mut(&board_name) {
                    nodes.retain(|x| x != &self.data.id);
                }
            }
            if let Some(var_name) = &new_var_name {
                let board_name = board_name_for_var(self.preset_id(), var_name);
                let ma = self.ma();
                let mut board_out_agents = ma.board_out_agents.lock().unwrap();
                if let Some(nodes) = board_out_agents.get_mut(&board_name) {
                    nodes.push(self.data.id.clone());
                } else {
                    board_out_agents.insert(board_name.clone(), vec![self.data.id.clone()]);
                }
            }
            self.var_name = new_var_name;
        }
        Ok(())
    }
}

fn board_name_for_var(flow_id: &str, var_name: &str) -> String {
    format!("%{}/{}", flow_id, var_name)
}
