use std::{
    sync::{Arc, Mutex},
    vec,
};

use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentData, AgentDefinition, AgentError, AgentOutput,
    AgentValue, AsAgent, AsAgentData, async_trait, new_agent_boxed,
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
    if data.is_array() && data.kind == "message" {
        let mut arr = data.as_array().unwrap_or(&vec![]).to_owned();
        arr.push(message.into());
        return AgentData::array("message", arr);
    }

    if data.is_string() {
        let value = data.as_str().unwrap_or("");
        if !value.is_empty() {
            let in_message = Message::user(value.to_string());
            return AgentData::array("message", vec![message.into(), in_message.into()]);
        }
    }

    #[cfg(feature = "image")]
    if let AgentValue::Image(img) = data.value {
        let message = message.with_image(img);
        return message.into();
        // return AgentData::array("message", vec![message.into()]);
    }

    // AgentData::array("message", vec![message.into()])
    message.into()
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
            self.first_run = true;
            let mut history = self.history.lock().unwrap();
            history.reset();
            return Ok(());
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
                *history = preamble_history;
            }
        }

        let message: Message = data.try_into().map_err(|e| {
            AgentError::InvalidValue(format!("Failed to convert data to Message: {}", e))
        })?;

        history.push(message.clone());
        self.try_output(ctx.clone(), PORT_HISTORY, history.clone().into())?;

        if message.role != "user" {
            return Ok(());
        }

        let messages: AgentData = AgentData::object(
            [
                ("message".to_string(), message.into()),
                (
                    "history".to_string(),
                    AgentValue::array(
                        history
                            .messages()
                            .iter()
                            .cloned()
                            .map(|m| m.into())
                            .collect(),
                    ),
                ),
            ]
            .into(),
        );
        self.try_output(ctx, PORT_MESSAGE_HISTORY, messages)?;

        Ok(())
    }
}

pub fn is_message(data: &AgentData) -> bool {
    if data.is_object() {
        let obj = data.as_object().unwrap();
        return obj.contains_key("role") && obj.contains_key("content");
    }
    false
}

pub fn is_message_history(data: &AgentData) -> bool {
    if data.is_object() {
        let obj = data.as_object().unwrap();
        return obj.contains_key("message") && obj.contains_key("history");
    }
    false
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "LLM";

static PORT_MESSAGE: &str = "message";
static PORT_MESSAGES: &str = "messages";
static PORT_MESSAGE_HISTORY: &str = "message_history";
static PORT_HISTORY: &str = "history";
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
        .title("Assistant Message")
        .category(CATEGORY)
        .inputs(vec![PORT_MESSAGES])
        .outputs(vec![PORT_MESSAGES])
        .text_config_default(CONFIG_MESSAGE),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "llm_system_message",
            Some(new_agent_boxed::<SystemMessageAgent>),
        )
        .title("System Message")
        .category(CATEGORY)
        .inputs(vec![PORT_MESSAGES])
        .outputs(vec![PORT_MESSAGES])
        .text_config_default(CONFIG_MESSAGE),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "llm_user_message",
            Some(new_agent_boxed::<UserMessageAgent>),
        )
        .title("User Message")
        .category(CATEGORY)
        .inputs(vec![PORT_MESSAGES])
        .outputs(vec![PORT_MESSAGES])
        .text_config_default(CONFIG_MESSAGE),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "llm_message_history",
            Some(new_agent_boxed::<MessageHistoryAgent>),
        )
        .title("Message History")
        .category(CATEGORY)
        .inputs(vec![PORT_MESSAGE, PORT_RESET])
        .outputs(vec![PORT_MESSAGE_HISTORY, PORT_HISTORY])
        .boolean_config_with(CONFIG_INCLUDE_SYSTZEM, false, |entry| {
            entry.title("Include System")
        })
        .text_config_default(CONFIG_PREAMBLE)
        .integer_config_default(CONFIG_HISTORY_SIZE),
    );
}
