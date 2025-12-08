use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    async_trait,
};
use askit_macros::askit_agent;

use crate::message::{Message, MessageHistory};

static CATEGORY: &str = "LLM";

static PIN_MESSAGE: &str = "message";
static PIN_MESSAGES: &str = "messages";
static PIN_MESSAGE_HISTORY: &str = "message_history";
static PIN_HISTORY: &str = "history";
static PIN_RESET: &str = "reset";

static CONFIG_HISTORY_SIZE: &str = "history_size";
static CONFIG_MESSAGE: &str = "message";
static CONFIG_PREAMBLE: &str = "preamble";
static CONFIG_INCLUDE_SYSTZEM: &str = "include_system";

// Assistant Message Agent
#[askit_agent(
    title="Assistant Message",
    category=CATEGORY,
    inputs=[PIN_MESSAGES],
    outputs=[PIN_MESSAGES],
    text_config(name=CONFIG_MESSAGE)
)]
pub struct AssistantMessageAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for AssistantMessageAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
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
        self.try_output(ctx, PIN_MESSAGES, messages)?;
        Ok(())
    }
}

// System Message Agent
#[askit_agent(
    title="System Message",
    category=CATEGORY,
    inputs=[PIN_MESSAGES],
    outputs=[PIN_MESSAGES],
    text_config(name=CONFIG_MESSAGE)
)]
pub struct SystemMessageAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for SystemMessageAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
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
        self.try_output(ctx, PIN_MESSAGES, messages)?;
        Ok(())
    }
}

// User Message Agent
#[askit_agent(
    title="User Message",
    category=CATEGORY,
    inputs=[PIN_MESSAGES],
    outputs=[PIN_MESSAGES],
    text_config(name=CONFIG_MESSAGE)
)]
pub struct UserMessageAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for UserMessageAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
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
        self.try_output(ctx, PIN_MESSAGES, messages)?;
        Ok(())
    }
}

fn add_message(value: AgentValue, message: Message) -> AgentValue {
    if value.is_string() {
        let value = value.as_str().unwrap_or("");
        if !value.is_empty() {
            let in_message = Message::user(value.to_string());
            return AgentValue::array(vec![in_message.into(), message.into()]);
        }
    }

    #[cfg(feature = "image")]
    if let AgentValue::Image(img) = value {
        let message = message.with_image(img);
        return message.into();
    }

    if value.is_object() {
        // Append the object without checking whether it is a message
        let mut arr = vec![value];
        arr.push(message.into());
        return AgentValue::array(arr);
    }

    if value.is_array() {
        // Append without verifying the array items are messages
        let mut arr = value.as_array().unwrap_or(&vec![]).to_owned();
        arr.push(message.into());
        return AgentValue::array(arr);
    }

    message.into()
}

// Message History Agent
#[askit_agent(
    title="Message History",
    category=CATEGORY,
    inputs=[PIN_MESSAGE, PIN_RESET],
    outputs=[PIN_MESSAGE_HISTORY, PIN_HISTORY],
    boolean_config(
        name=CONFIG_INCLUDE_SYSTZEM,
        title="Include System"
    ),
    text_config(name=CONFIG_PREAMBLE),
    integer_config(name=CONFIG_HISTORY_SIZE)
)]
pub struct MessageHistoryAgent {
    data: AgentData,
    history: MessageHistory,
    preamble_included: bool,
}

#[async_trait]
impl AsAgent for MessageHistoryAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            history: MessageHistory::new(vec![], 0),
            preamble_included: false,
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if pin == PIN_RESET {
            self.history = MessageHistory::new(vec![], 0);
            self.preamble_included = false;
            return Ok(());
        }

        let history_size = self.configs()?.get_integer_or_default(CONFIG_HISTORY_SIZE) as usize;
        self.history.set_max_size(history_size);

        if !self.preamble_included {
            self.preamble_included = true;
            let preamble_str = self.configs()?.get_string_or_default(CONFIG_PREAMBLE);
            if !preamble_str.is_empty() {
                let preamble_history = MessageHistory::parse(&preamble_str).map_err(|e| {
                    AgentError::InvalidValue(format!("Failed to parse preamble messages: {}", e))
                })?;
                self.history.set_preamble(preamble_history.messages());
            }
        }

        let message: Message = value.try_into().map_err(|e| {
            AgentError::InvalidValue(format!("Failed to convert data to Message: {}", e))
        })?;

        self.history.push(message.clone());
        self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;

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
        self.try_output(ctx, PIN_MESSAGE_HISTORY, messages)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_message() {
        // () + user
        // result should be the user message
        let value = AgentValue::unit();
        let msg = Message::user("Hello".to_string());
        let result = add_message(value, msg);
        assert!(result.is_object());
        assert_eq!(result.get_str("role").unwrap(), "user");
        assert_eq!(result.get_str("content").unwrap(), "Hello");

        // string + assistant
        // result should be an array with user and assistant messages
        let value = AgentValue::string("How are you?");
        let msg = Message::assistant("Hello".to_string());
        let result = add_message(value, msg);
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].get_str("role").unwrap(), "user");
        assert_eq!(arr[0].get_str("content").unwrap(), "How are you?");
        assert_eq!(arr[1].get_str("role").unwrap(), "assistant");
        assert_eq!(arr[1].get_str("content").unwrap(), "Hello");

        // object + user
        // result should be an array with the original object and the new user message
        let value = AgentValue::object(
            [
                ("role".to_string(), AgentValue::string("system")),
                ("content".to_string(), AgentValue::string("I am fine.")),
            ]
            .into(),
        );
        let msg = Message::user("Hello".to_string());
        let result = add_message(value, msg);
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].get_str("role").unwrap(), "system");
        assert_eq!(arr[0].get_str("content").unwrap(), "I am fine.");
        assert_eq!(arr[1].get_str("role").unwrap(), "user");
        assert_eq!(arr[1].get_str("content").unwrap(), "Hello");

        // array + user
        // result should be the original array with the new user message appended
        let value = AgentValue::array(vec![
            AgentValue::object(
                [
                    ("role".to_string(), AgentValue::string("system")),
                    ("content".to_string(), AgentValue::string("Welcome!")),
                ]
                .into(),
            ),
            AgentValue::object(
                [
                    ("role".to_string(), AgentValue::string("assistant")),
                    ("content".to_string(), AgentValue::string("Hello!")),
                ]
                .into(),
            ),
        ]);
        let msg = Message::user("How are you?".to_string());
        let result = add_message(value, msg);
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0].get_str("role").unwrap(), "system");
        assert_eq!(arr[0].get_str("content").unwrap(), "Welcome!");
        assert_eq!(arr[1].get_str("role").unwrap(), "assistant");
        assert_eq!(arr[1].get_str("content").unwrap(), "Hello!");
        assert_eq!(arr[2].get_str("role").unwrap(), "user");
        assert_eq!(arr[2].get_str("content").unwrap(), "How are you?");

        // image + user
        #[cfg(feature = "image")]
        let img = AgentValue::image(agent_stream_kit::PhotonImage::new(vec![0u8; 4], 1, 1));
        {
            let msg = Message::user("Check this image".to_string());
            let result = add_message(img, msg);
            assert!(result.is_object());
            assert_eq!(result.get_str("role").unwrap(), "user");
            assert_eq!(result.get_str("content").unwrap(), "Check this image");
            assert!(result.get_image("image").is_some());
        }
    }
}
