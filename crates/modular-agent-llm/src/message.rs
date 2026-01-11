use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    Message, askit_agent, async_trait,
};
use im::{Vector, vector};

const CATEGORY: &str = "LLM/Message";

const PIN_MESSAGE: &str = "message";
const PIN_MESSAGES: &str = "messages";
const PIN_RESET: &str = "reset";

const CONFIG_MAX_SIZE: &str = "max_size";
const CONFIG_MESSAGE: &str = "message";
const CONFIG_MESSAGES: &str = "messages";
const CONFIG_PREAMBLE: &str = "preamble";

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
        let messages = append_message(value, message);
        self.output(ctx, PIN_MESSAGES, messages).await?;
        Ok(())
    }
}

/// Add a system message to the messages.
///
/// The system message is always prepended to the messages.
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
        let messages = prepend_message(value, message);
        self.output(ctx, PIN_MESSAGES, messages).await?;
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
        let messages = append_message(value, message);
        self.output(ctx, PIN_MESSAGES, messages).await?;
        Ok(())
    }
}

fn append_message(value: AgentValue, message: Message) -> AgentValue {
    #[cfg(feature = "image")]
    if let AgentValue::Image(img) = &value {
        let message = message.with_image(img.clone());
        return AgentValue::array(vector![message.into()]);
    }

    let Some(value) = value.to_message_value() else {
        return message.into();
    };

    if value.is_array() {
        let mut arr = value.into_array().unwrap_or_default();
        arr.push_back(message.into());
        return AgentValue::array(arr);
    }

    AgentValue::array(vector![value, message.into()])
}

fn prepend_message(value: AgentValue, message: Message) -> AgentValue {
    let Some(value) = value.to_message_value() else {
        return message.into();
    };

    if value.is_array() {
        let mut arr = value.into_array().unwrap_or_default();
        arr.push_front(message.into());
        return AgentValue::array(arr);
    }

    AgentValue::array(vector![message.into(), value])
}

/// Prepend a preamble message to the first input message.
///
//// The preamble message is added only once.
#[askit_agent(
    title="Preamble",
    category=CATEGORY,
    inputs=[PIN_MESSAGE, PIN_RESET],
    outputs=[PIN_MESSAGES],
    text_config(name=CONFIG_PREAMBLE),
)]
pub struct PreambleAgent {
    data: AgentData,
    preamble: Option<Vector<AgentValue>>,
    prepended: bool,
}

#[async_trait]
impl AsAgent for PreambleAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let preamble = spec
            .configs
            .as_ref()
            .map(|c| c.get(CONFIG_PREAMBLE))
            .transpose()?
            .and_then(|v| v.to_message_value());
        let preamble = match preamble {
            None => None,
            Some(preamble) => {
                if preamble.is_array() {
                    Some(preamble.into_array().unwrap_or_default())
                } else {
                    Some(vector![preamble])
                }
            }
        };
        let data = AgentData::new(askit, id, spec);
        Ok(Self {
            data,
            preamble,
            prepended: false,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let preamble = self.configs()?.get(CONFIG_PREAMBLE)?.to_message_value();
        self.preamble = match preamble {
            None => None,
            Some(preamble) => {
                if preamble.is_array() {
                    Some(preamble.into_array().unwrap_or_default())
                } else {
                    Some(vector![preamble])
                }
            }
        };
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if pin == PIN_RESET {
            self.prepended = false;
            return Ok(());
        }

        let Some(message) = value.to_message() else {
            return Err(AgentError::InvalidValue(
                "Input value is not a Message".to_string(),
            ));
        };

        if self.prepended {
            return self.output(
                ctx,
                PIN_MESSAGES,
                AgentValue::array(vector![message.into()]),
            )
            .await;
        }

        self.prepended = true;

        let Some(preamble) = &self.preamble else {
            return self.output(
                ctx,
                PIN_MESSAGES,
                AgentValue::array(vector![message.into()]),
            )
            .await;
        };

        let mut messages = preamble.clone();
        messages.push_back(message.into());
        self.output(ctx, PIN_MESSAGES, AgentValue::array(messages)).await?;

        Ok(())
    }
}

/// Store and accumulate messages.
///
/// It stores the received messages internally and outputs them.
/// When max_size > 0, the number of stored messages is limited to max_size.
/// The stored messages are retained even if the agent is stopped.
/// When an input is received on reset, the stored messages are cleared.
#[askit_agent(
    title="Messages",
    category=CATEGORY,
    inputs=[PIN_MESSAGE, PIN_RESET],
    outputs=[PIN_MESSAGES],
    integer_config(name=CONFIG_MAX_SIZE),
    array_config(name=CONFIG_MESSAGES, hidden),
)]
pub struct MessagesAgent {
    data: AgentData,
}

impl MessagesAgent {
    fn reset_messages(&mut self) -> Result<(), AgentError> {
        self.set_config(CONFIG_MESSAGES.to_string(), AgentValue::array_default())
    }
}

#[async_trait]
impl AsAgent for MessagesAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if pin == PIN_RESET {
            self.reset_messages()?;
            self.output(ctx, PIN_MESSAGES, AgentValue::array_default()).await?;
            return Ok(());
        }

        let in_message = value.to_message_value().ok_or_else(|| {
            AgentError::InvalidValue("Input contains non-Message values".to_string())
        })?;
        let in_messages = if in_message.is_array() {
            in_message.into_array().unwrap_or_default()
        } else {
            vector![in_message]
        };
        if in_messages.is_empty() {
            return Ok(());
        }

        let first_in_message_id = in_messages
            .front()
            .unwrap()
            .as_message()
            .ok_or_else(|| {
                AgentError::InvalidValue("Input contains non-Message values".to_string())
            })?
            .id
            .clone();

        let mut messages = self.configs()?.get_array_or_default(CONFIG_MESSAGES);
        if messages.len() > 0 && first_in_message_id.is_some() {
            let last_message = messages.last().unwrap().as_message().ok_or_else(|| {
                AgentError::InvalidValue("Stored messages contain non-Message values".to_string())
            })?;
            if last_message.id == first_in_message_id {
                // Update the last message
                messages.pop_back();
            }
        }
        messages.append(in_messages);

        let mlen = messages.len() as i64;
        let max_size = self.configs()?.get_integer_or_default(CONFIG_MAX_SIZE);
        if max_size > 0 && mlen > max_size {
            messages = messages.skip((mlen - max_size) as usize)
        }

        let arr = AgentValue::array(messages);
        self.set_config(CONFIG_MESSAGES.to_string(), arr.clone())?;
        self.output(ctx, PIN_MESSAGES, arr).await?;

        Ok(())
    }
}

/// Convert to messages for prompt.
///
/// It selects messages to fit within max_size.
/// The prompt order is (system, ) user, (assistant, user)*.
#[askit_agent(
    title="Messages for Prompt",
    category=CATEGORY,
    inputs=[PIN_MESSAGES],
    outputs=[PIN_MESSAGES],
    integer_config(name=CONFIG_MAX_SIZE),
)]
pub struct MessagesForPromptAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for MessagesForPromptAgent {
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
        let max_size = self.configs()?.get_integer_or_default(CONFIG_MAX_SIZE);
        if max_size <= 0 {
            // Just output the input messages
            self.output(ctx, PIN_MESSAGES, value).await?;
            return Ok(());
        }

        let messages_value = value.to_message_value().ok_or_else(|| {
            AgentError::InvalidValue("Input contains non-Message values".to_string())
        })?;
        let mut messages = if messages_value.is_array() {
            messages_value.as_array().unwrap().clone()
        } else {
            vector![messages_value]
        };
        if messages.is_empty() {
            return Ok(());
        }

        let mut total_size = 0;

        // Extract system message if exists
        let mut system_message: Option<AgentValue> = None;
        if messages.front().unwrap().as_message().unwrap().role == "system" {
            let msg = messages.pop_front().unwrap();
            total_size += msg.as_message().unwrap().content.len();
            system_message = Some(msg);
        }

        // Collect messages in reverse order
        let mut selected_messages: Vec<AgentValue> = Vec::with_capacity(messages.len());
        while !messages.is_empty() {
            let value = messages.pop_back().unwrap();
            let msg = value.as_message().unwrap();
            let mut msg_size = msg.content.len();

            #[cfg(feature = "image")]
            {
                if let Some(img) = &msg.image {
                    msg_size += img.get_estimated_filesize() as usize;
                }
            }

            // Do we need to consider tool_calls size?

            if total_size + msg_size > max_size as usize {
                break;
            }
            total_size += msg_size;

            if msg.thinking.is_some() {
                // Remove thinking
                let mut m = msg.clone();
                m.thinking = None;
                selected_messages.push(AgentValue::message(m));
            } else {
                selected_messages.push(value);
            }
        }

        // Ensure the first message is user
        while let Some(last_msg) = selected_messages.last() {
            let role = last_msg.as_message().unwrap().role.as_str();
            if role != "user" {
                selected_messages.pop();
            } else {
                break;
            }
        }

        if let Some(system_message) = system_message {
            selected_messages.push(system_message);
        }

        selected_messages.reverse();
        self.output(ctx, PIN_MESSAGES, selected_messages.into()).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use im::hashmap;

    #[test]
    fn test_add_message() {
        // () + user
        // result should be the user message
        let value = AgentValue::unit();
        let msg = Message::user("Hello".to_string());
        let result = append_message(value, msg);
        assert!(result.is_message());
        let result_msg = result.as_message().unwrap();
        assert_eq!(result_msg.role, "user");
        assert_eq!(result_msg.content, "Hello");

        // string + assistant
        // result should be an array with user and assistant messages
        let value = AgentValue::string("How are you?");
        let msg = Message::assistant("Hello".to_string());
        let result = append_message(value, msg);
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let msg0 = &arr[0].as_message().unwrap();
        assert_eq!(msg0.role, "user");
        assert_eq!(msg0.content, "How are you?");
        let msg1 = &arr[1].as_message().unwrap();
        assert_eq!(msg1.role, "assistant");
        assert_eq!(msg1.content, "Hello");

        // object + user
        // result should be an array with the original object and the new user message
        let value = AgentValue::object(hashmap! {
            "role".into() => AgentValue::string("system"),
            "content".into() => AgentValue::string("I am fine."),
        });
        let msg = Message::user("Hello".to_string());
        let result = append_message(value, msg);
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let msg0 = &arr[0].as_message().unwrap();
        assert_eq!(msg0.role, "system");
        assert_eq!(msg0.content, "I am fine.");
        let msg1 = &arr[1].as_message().unwrap();
        assert_eq!(msg1.role, "user");
        assert_eq!(msg1.content, "Hello");

        // array + user
        // result should be the original array with the new user message appended
        let value = AgentValue::array(vector![
            AgentValue::object(hashmap! {
                "role".into() => AgentValue::string("system"),
                "content".into() => AgentValue::string("Welcome!"),
            }),
            AgentValue::object(hashmap! {
                "role".into() => AgentValue::string("assistant"),
                "content".into() => AgentValue::string("Hello!"),
            }),
        ]);
        let msg = Message::user("How are you?".to_string());
        let result = append_message(value, msg);
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        let msg0 = &arr[0].as_message().unwrap();
        assert_eq!(msg0.role, "system");
        assert_eq!(msg0.content, "Welcome!");
        let msg1 = &arr[1].as_message().unwrap();
        assert_eq!(msg1.role, "assistant");
        assert_eq!(msg1.content, "Hello!");
        let msg2 = &arr[2].as_message().unwrap();
        assert_eq!(msg2.role, "user");
        assert_eq!(msg2.content, "How are you?");

        // image + user
        #[cfg(feature = "image")]
        let img = AgentValue::image(agent_stream_kit::PhotonImage::new(vec![0u8; 4], 1, 1));
        {
            let msg = Message::user("Check this image".to_string());
            let result = append_message(img, msg);
            assert!(result.is_array());
            let arr = result.as_array().unwrap();
            assert_eq!(arr.len(), 1);
            let msg0 = &arr[0].as_message().unwrap();
            assert_eq!(msg0.role, "user");
            assert_eq!(msg0.content, "Check this image");
            assert!(msg0.image.is_some());
        }
    }
}
