use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentError, AgentOutput, AgentValue, AsAgent,
    AsAgentData, async_trait,
};
use askit_macros::askit_agent;

use crate::message::{Message, MessageHistory};

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

// Assistant Message Agent
#[askit_agent(
    title="Assistant Message",
    category=CATEGORY,
    inputs=[PORT_MESSAGES],
    outputs=[PORT_MESSAGES],
    text_config(name=CONFIG_MESSAGE)
)]
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

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let message = self.configs()?.get_string(CONFIG_MESSAGE)?;
        let message = Message::assistant(message);
        let messages = add_message(value, message);
        self.try_output(ctx, PORT_MESSAGES, messages)?;
        Ok(())
    }
}

// System Message Agent
#[askit_agent(
    title="System Message",
    category=CATEGORY,
    inputs=[PORT_MESSAGES],
    outputs=[PORT_MESSAGES],
    text_config(name=CONFIG_MESSAGE)
)]
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

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let message = self.configs()?.get_string(CONFIG_MESSAGE)?;
        let message = Message::system(message);
        let messages = add_message(value, message);
        self.try_output(ctx, PORT_MESSAGES, messages)?;
        Ok(())
    }
}

// User Message Agent
#[askit_agent(
    title="User Message",
    category=CATEGORY,
    inputs=[PORT_MESSAGES],
    outputs=[PORT_MESSAGES],
    text_config(name=CONFIG_MESSAGE)
)]
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

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let message = self.configs()?.get_string(CONFIG_MESSAGE)?;
        let message = Message::user(message);
        let messages = add_message(value, message);
        self.try_output(ctx, PORT_MESSAGES, messages)?;
        Ok(())
    }
}

fn add_message(value: AgentValue, message: Message) -> AgentValue {
    if value.is_array() {
        let mut arr = value.as_array().unwrap_or(&vec![]).to_owned();
        arr.push(message.into());
        return AgentValue::array(arr);
    }

    if value.is_string() {
        let value = value.as_str().unwrap_or("");
        if !value.is_empty() {
            let in_message = Message::user(value.to_string());
            return AgentValue::array(vec![message.into(), in_message.into()]);
        }
    }

    #[cfg(feature = "image")]
    if let AgentValue::Image(img) = value {
        let message = message.with_image(img);
        return message.into();
    }

    message.into()
}

// Message History Agent
#[askit_agent(
    title="Message History",
    category=CATEGORY,
    inputs=[PORT_MESSAGE, PORT_RESET],
    outputs=[PORT_MESSAGE_HISTORY, PORT_HISTORY],
    boolean_config(
        name=CONFIG_INCLUDE_SYSTZEM,
        title="Include System"
    ),
    text_config(name=CONFIG_PREAMBLE),
    integer_config(name=CONFIG_HISTORY_SIZE)
)]
pub struct MessageHistoryAgent {
    data: AsAgentData,
    history: MessageHistory,
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
            history: MessageHistory::new(vec![], 0),
            first_run: true,
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if pin == PORT_RESET {
            self.first_run = true;
            self.history.reset();
            return Ok(());
        }

        let history_size = self.configs()?.get_integer_or_default(CONFIG_HISTORY_SIZE);

        self.history.set_size(history_size);

        if self.first_run {
            // On first run, load preamble messages if any
            self.first_run = false;
            let preamble_str = self.configs()?.get_string_or_default(CONFIG_PREAMBLE);
            if !preamble_str.is_empty() {
                let preamble_history = MessageHistory::parse(&preamble_str).map_err(|e| {
                    AgentError::InvalidValue(format!("Failed to parse preamble messages: {}", e))
                })?;
                self.history = preamble_history;
            }
        }

        let message: Message = value.try_into().map_err(|e| {
            AgentError::InvalidValue(format!("Failed to convert data to Message: {}", e))
        })?;

        self.history.push(message.clone());
        self.try_output(ctx.clone(), PORT_HISTORY, self.history.clone().into())?;

        if message.role != "user" {
            return Ok(());
        }

        let messages: AgentValue = AgentValue::object(
            [
                ("message".to_string(), message.into()),
                (
                    "history".to_string(),
                    AgentValue::array(
                        self.history
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

pub fn is_message(value: &AgentValue) -> bool {
    if value.is_object() {
        let obj = value.as_object().unwrap();
        return obj.contains_key("role") && obj.contains_key("content");
    }
    false
}

pub fn is_message_history(value: &AgentValue) -> bool {
    if value.is_object() {
        let obj = value.as_object().unwrap();
        return obj.contains_key("message") && obj.contains_key("history");
    }
    false
}
