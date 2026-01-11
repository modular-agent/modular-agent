use std::vec;

use async_trait::async_trait;

use askit_macros::askit_agent;

use crate::agent::{Agent, AgentData, AsAgent};
use crate::askit::ASKit;
use crate::context::AgentContext;
use crate::error::AgentError;
use crate::spec::AgentSpec;
use crate::value::AgentValue;

static PIN_VALUE: &str = "value";

static CONFIG_NAME: &str = "name";

#[askit_agent(
    kind = "Board",
    title = "->Board",
    category = "Core",
    inputs = [PIN_VALUE],
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
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let board_name = spec
            .configs
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_NAME).ok());
        Ok(Self {
            data: AgentData::new(askit, id, spec),
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
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let board_name = self.board_name.clone().unwrap_or_default();
        if board_name.is_empty() {
            // if board_name is not set, stop processing
            return Ok(());
        }
        let askit = self.askit();
        askit
            .send_board_out(board_name.clone(), ctx, value.clone())
            .await?;

        Ok(())
    }
}

#[askit_agent(
    kind = "Board",
    title = "Board->",
    category = "Core",
    outputs = [PIN_VALUE],
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
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let board_name = spec
            .configs
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_NAME).ok());
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            board_name,
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        if let Some(board_name) = &self.board_name {
            let askit = self.askit();
            let mut board_out_agents = askit.board_out_agents.lock().unwrap();
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
            let askit = self.askit();
            let mut board_out_agents = askit.board_out_agents.lock().unwrap();
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
                let askit = self.askit();
                let mut board_out_agents = askit.board_out_agents.lock().unwrap();
                if let Some(nodes) = board_out_agents.get_mut(board_name) {
                    nodes.retain(|x| x != &self.data.id);
                }
            }
            if let Some(board_name) = &board_name {
                let askit = self.askit();
                let mut board_out_agents = askit.board_out_agents.lock().unwrap();
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

#[askit_agent(
    kind = "Board",
    title = "->Var",
    category = "Core",
    inputs = [PIN_VALUE],
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
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let var_name = spec
            .configs
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_NAME).ok());
        Ok(Self {
            data: AgentData::new(askit, id, spec),
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
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let var_name = self.var_name.clone().unwrap_or_default();
        if var_name.is_empty() {
            // if var_name is not set, stop processing
            return Ok(());
        }
        let board_name = board_name_for_var(self.stream_id(), &var_name);
        let askit = self.askit();
        askit
            .send_board_out(board_name.clone(), ctx, value.clone())
            .await?;

        Ok(())
    }
}

#[askit_agent(
    kind = "Board",
    title = "Var->",
    category = "Core",
    outputs = [PIN_VALUE],
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
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let var_name = spec
            .configs
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_NAME).ok());
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            var_name,
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        if let Some(var_name) = &self.var_name {
            let board_name = board_name_for_var(self.stream_id(), var_name);
            let askit = self.askit();
            let mut board_out_agents = askit.board_out_agents.lock().unwrap();
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
            let board_name = board_name_for_var(self.stream_id(), var_name);
            let askit = self.askit();
            let mut board_out_agents = askit.board_out_agents.lock().unwrap();
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
                let board_name = board_name_for_var(self.stream_id(), var_name);
                let askit = self.askit();
                let mut board_out_agents = askit.board_out_agents.lock().unwrap();
                if let Some(nodes) = board_out_agents.get_mut(&board_name) {
                    nodes.retain(|x| x != &self.data.id);
                }
            }
            if let Some(var_name) = &new_var_name {
                let board_name = board_name_for_var(self.stream_id(), var_name);
                let askit = self.askit();
                let mut board_out_agents = askit.board_out_agents.lock().unwrap();
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
