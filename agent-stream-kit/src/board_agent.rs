use askit_macros::askit_agent;
use async_trait::async_trait;
use std::vec;

use super::agent::{Agent, AsAgent, AsAgentData};
use super::askit::ASKit;
use super::config::AgentConfigs;
use super::context::AgentContext;
use super::error::AgentError;
use super::value::AgentValue;

static CONFIG_BOARD_NAME: &str = "board";
static CONFIG_VAR_NAME: &str = "var";

#[askit_agent(
    kind = "Board",
    title = "Board In",
    category = "Core",
    inputs = ["*"],
    string_config(
        name = CONFIG_BOARD_NAME,
        title = "Board Name",
    )
)]
struct BoardInAgent {
    data: AsAgentData,
    board_name: Option<String>,
}

#[async_trait]
impl AsAgent for BoardInAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        let board_name = config
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_BOARD_NAME).ok());
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            board_name,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        self.board_name = self.configs()?.get_string(CONFIG_BOARD_NAME).ok();
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
        askit.try_send_board_out(board_name.clone(), ctx, value.clone())?;

        Ok(())
    }
}

#[askit_agent(
    kind = "Board",
    title = "Board Out",
    category = "Core",
    outputs = ["*"],
    string_config(
        name = CONFIG_BOARD_NAME,
        title = "Board Name"
    )
)]
struct BoardOutAgent {
    data: AsAgentData,
    board_name: Option<String>,
}

#[async_trait]
impl AsAgent for BoardOutAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        let board_name = config
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_BOARD_NAME).ok());
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
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
        let board_name = self.configs()?.get_string(CONFIG_BOARD_NAME).ok();
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
    title = "Var In",
    category = "Core",
    inputs = ["*"],
    string_config(
        name = CONFIG_VAR_NAME,
        title = "Var Name",
    )
)]
struct VarInAgent {
    data: AsAgentData,
    var_name: Option<String>,
}

#[async_trait]
impl AsAgent for VarInAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        let var_name = config
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_VAR_NAME).ok());
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            var_name,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        self.var_name = self.configs()?.get_string(CONFIG_VAR_NAME).ok();
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
        let board_name = board_name_for_var(self.flow_name(), &var_name);
        let askit = self.askit();
        askit.try_send_board_out(board_name.clone(), ctx, value.clone())?;

        Ok(())
    }
}

#[askit_agent(
    kind = "Board",
    title = "Var Out",
    category = "Core",
    outputs = ["*"],
    string_config(
        name = CONFIG_VAR_NAME,
        title = "Var Name"
    )
)]
struct VarOutAgent {
    data: AsAgentData,
    var_name: Option<String>,
}

#[async_trait]
impl AsAgent for VarOutAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        let var_name = config
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_VAR_NAME).ok());
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            var_name,
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        if let Some(var_name) = &self.var_name {
            let board_name = board_name_for_var(self.flow_name(), var_name);
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
            let board_name = board_name_for_var(self.flow_name(), var_name);
            let askit = self.askit();
            let mut board_out_agents = askit.board_out_agents.lock().unwrap();
            if let Some(nodes) = board_out_agents.get_mut(&board_name) {
                nodes.retain(|x| x != &self.data.id);
            }
        }
        Ok(())
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let new_var_name = self.configs()?.get_string(CONFIG_VAR_NAME).ok();
        if self.var_name != new_var_name {
            if let Some(var_name) = &self.var_name {
                let board_name = board_name_for_var(self.flow_name(), var_name);
                let askit = self.askit();
                let mut board_out_agents = askit.board_out_agents.lock().unwrap();
                if let Some(nodes) = board_out_agents.get_mut(&board_name) {
                    nodes.retain(|x| x != &self.data.id);
                }
            }
            if let Some(var_name) = &new_var_name {
                let board_name = board_name_for_var(self.flow_name(), var_name);
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

fn board_name_for_var(flow_name: &str, var_name: &str) -> String {
    format!("%{}%{}", flow_name, var_name)
}
