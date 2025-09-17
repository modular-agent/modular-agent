use async_trait::async_trait;
use std::vec;

use super::agent::{Agent, AsAgent, AsAgentData, new_boxed};
use super::askit::ASKit;
use super::config::AgentConfig;
use super::context::AgentContext;
use super::data::{AgentData, AgentValue};
use super::definition::{AgentConfigEntry, AgentDefinition};
use super::error::AgentError;

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
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        let board_name = config
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_BOARD_NAME).ok());
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            board_name,
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    fn set_config(&mut self, config: AgentConfig) -> Result<(), AgentError> {
        self.board_name = config.get_string(CONFIG_BOARD_NAME).ok();
        Ok(())
    }

    fn start(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let mut board_name = self.board_name.clone().unwrap_or_default();
        if board_name.is_empty() {
            // if board_name is not set, stop processing
            return Ok(());
        }
        if board_name == "*" {
            if ctx.port().is_empty() {
                // port should not be empty, but just in case
                return Ok(());
            }
            board_name = ctx.port().to_string();
        }
        let askit = self.askit();
        {
            let mut board_data = askit.board_data.lock().unwrap();
            board_data.insert(board_name.clone(), data.clone());
        }
        askit.try_send_board_out(board_name.clone(), ctx, data.clone())?;

        Ok(())
    }
}

struct BoardOutAgent {
    data: AsAgentData,
    board_name: Option<String>,
}

impl AsAgent for BoardOutAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        let board_name = config
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_BOARD_NAME).ok());
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            board_name,
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    fn start(&mut self) -> Result<(), AgentError> {
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

    fn stop(&mut self) -> Result<(), AgentError> {
        if let Some(board_name) = &self.board_name {
            let askit = self.askit();
            let mut board_out_agents = askit.board_out_agents.lock().unwrap();
            if let Some(nodes) = board_out_agents.get_mut(board_name) {
                nodes.retain(|x| x != &self.data.id);
            }
        }
        Ok(())
    }

    fn set_config(&mut self, config: AgentConfig) -> Result<(), AgentError> {
        let board_name = config.get_string(CONFIG_BOARD_NAME).ok();
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

static CONFIG_BOARD_NAME: &str = "$board";

pub fn register_agents(askit: &ASKit) {
    // BoardInAgent
    askit.register_agent(
        AgentDefinition::new("Board", "core_board_in", Some(new_boxed::<BoardInAgent>))
            .with_title("Board In")
            .with_category("Core")
            .with_inputs(vec!["*"])
            .with_default_config(vec![(
                CONFIG_BOARD_NAME.into(),
                AgentConfigEntry::new(AgentValue::new_string(""), "string")
                    .with_title("Board Name")
                    .with_description("* = source kind"),
            )]),
    );

    // BoardOutAgent
    askit.register_agent(
        AgentDefinition::new("Board", "core_board_out", Some(new_boxed::<BoardOutAgent>))
            .with_title("Board Out")
            .with_category("Core")
            .with_outputs(vec!["*"])
            .with_default_config(vec![(
                CONFIG_BOARD_NAME.into(),
                AgentConfigEntry::new(AgentValue::new_string(""), "string")
                    .with_title("Board Name"),
            )]),
    );
}
