use std::{
    sync::{Arc, Mutex},
    vec,
};

use agent_stream_kit::{
    ASKit, Agent, AgentConfigEntry, AgentConfigs, AgentContext, AgentData, AgentDefinition,
    AgentError, AgentOutput, AsAgent, AsAgentData, async_trait, new_agent_boxed,
};

use crate::message::{Message, MessageHistory};

// Assistant Message Agent
pub struct AssistantMessageAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for AssistantMessageAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        data: AgentData,
    ) -> Result<(), AgentError> {
        let value = self.configs()?.get_string(CONFIG_MESSAGE)?;
        let message = Message::assistant(value);
        let messages = add_message(data, message);
        self.try_output(ctx, PORT_MESSAGES, messages)?;
        Ok(())
    }
}

// System Message Agent
pub struct SystemMessageAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for SystemMessageAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        data: AgentData,
    ) -> Result<(), AgentError> {
        let value = self.configs()?.get_string(CONFIG_MESSAGE)?;
        let message = Message::system(value);
        let messages = add_message(data, message);
        self.try_output(ctx, PORT_MESSAGES, messages)?;
        Ok(())
    }
}

// User Message Agent
pub struct UserMessageAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for UserMessageAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        data: AgentData,
    ) -> Result<(), AgentError> {
        let value = self.configs()?.get_string(CONFIG_MESSAGE)?;
        let message = Message::user(value);
        let messages = add_message(data, message);
        self.try_output(ctx, PORT_MESSAGES, messages)?;
        Ok(())
    }
}

fn add_message(data: AgentData, message: Message) -> AgentData {
    if data.is_array() {
        let mut arr = data.as_array().unwrap_or(&vec![]).to_owned();
        arr.push(message.into());
        return AgentData::array("message", arr);
    }

    let value = data.as_str().unwrap_or("");
    if !value.is_empty() {
        let in_message = Message::user(value.to_string());
        return AgentData::array("message", vec![message.into(), in_message.into()]);
    }

    AgentData::array("message", vec![message.into()])
}

// Message History Agent
pub struct MessageHistoryAgent {
    data: AsAgentData,
    history: Arc<Mutex<MessageHistory>>,
    first_run: bool,
}

#[async_trait]
impl AsAgent for MessageHistoryAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            history: Arc::new(Mutex::new(MessageHistory::new(vec![], 0))),
            first_run: true,
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        data: AgentData,
    ) -> Result<(), AgentError> {
        if pin == PORT_RESET {
            let mut history = self.history.lock().unwrap();
            history.reset();
        }

        let history_size = self.configs()?.get_integer_or_default(CONFIG_HISTORY_SIZE);

        let mut history = self.history.lock().unwrap();
        history.set_size(history_size);

        if self.first_run {
            // On first run, load preamble messages if any
            self.first_run = false;
            let preamble_str = self.configs()?.get_string_or_default(CONFIG_PREAMBLE);
            if !preamble_str.is_empty() {
                let preamble_history = MessageHistory::parse(&preamble_str).map_err(|e| {
                    AgentError::InvalidValue(format!("Failed to parse preamble messages: {}", e))
                })?;
                for message in preamble_history.messages() {
                    history.push(message);
                }
            }
        }

        let message: Message = data.try_into().map_err(|e| {
            AgentError::InvalidValue(format!("Failed to convert data to Message: {}", e))
        })?;

        history.push(message);

        let messages: AgentData = history.clone().into();
        self.try_output(ctx, PORT_MESSAGES, messages)?;
        Ok(())
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "LLM";

static PORT_MESSAGE: &str = "message";
static PORT_MESSAGES: &str = "messages";
static PORT_RESET: &str = "reset";

static CONFIG_HISTORY_SIZE: &str = "history_size";
static CONFIG_MESSAGE: &str = "message";
static CONFIG_PREAMBLE: &str = "preamble";
static CONFIG_INCLUDE_SYSTZEM: &str = "include_system";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "llm_assistant_message",
            Some(new_agent_boxed::<AssistantMessageAgent>),
        )
        .with_title("Assistant Message")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_MESSAGES])
        .with_outputs(vec![PORT_MESSAGES])
        .with_default_configs(vec![(CONFIG_MESSAGE, AgentConfigEntry::new("", "string"))]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "llm_system_message",
            Some(new_agent_boxed::<SystemMessageAgent>),
        )
        .with_title("System Message")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_MESSAGES])
        .with_outputs(vec![PORT_MESSAGES])
        .with_default_configs(vec![(CONFIG_MESSAGE, AgentConfigEntry::new("", "string"))]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "llm_user_message",
            Some(new_agent_boxed::<UserMessageAgent>),
        )
        .with_title("User Message")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_MESSAGES])
        .with_outputs(vec![PORT_MESSAGES])
        .with_default_configs(vec![(CONFIG_MESSAGE, AgentConfigEntry::new("", "string"))]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "llm_message_history",
            Some(new_agent_boxed::<MessageHistoryAgent>),
        )
        .with_title("Message History")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_MESSAGE, PORT_RESET])
        .with_outputs(vec![PORT_MESSAGES])
        .with_default_configs(vec![
            (CONFIG_PREAMBLE, AgentConfigEntry::new("", "text")),
            (CONFIG_HISTORY_SIZE, AgentConfigEntry::new(0, "integer")),
            (
                CONFIG_INCLUDE_SYSTZEM,
                AgentConfigEntry::new(false, "boolean").with_title("Include System"),
            ),
        ]),
    );
}
