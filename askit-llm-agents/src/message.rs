use agent_stream_kit::{AgentData, AgentError, AgentValue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,

    pub content: String,
}

impl Message {
    pub fn new(role: String, content: String) -> Self {
        Self { role, content }
    }

    pub fn user(content: String) -> Self {
        Self {
            role: "user".to_string(),
            content,
        }
    }

    pub fn assistant(content: String) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
        }
    }

    pub fn system(content: String) -> Self {
        Self {
            role: "system".to_string(),
            content,
        }
    }
}

impl TryFrom<AgentData> for Message {
    type Error = AgentError;

    fn try_from(data: AgentData) -> Result<Self, Self::Error> {
        let message = data.value.try_into()?;
        Ok(message)
    }
}

impl TryFrom<AgentValue> for Message {
    type Error = AgentError;

    fn try_from(value: AgentValue) -> Result<Self, Self::Error> {
        if value.is_string() {
            let text = value.as_str().unwrap(); // Safe unwrap
            Ok(Message::user(text.to_string()))
        } else if value.is_object() {
            let role = value.get_str("role").unwrap_or("user").to_string();
            let content = value
                .get_str("content")
                .ok_or_else(|| {
                    AgentError::InvalidValue("Message object missing 'content' field".to_string())
                })?
                .to_string();
            Ok(Message::new(role, content))
        } else {
            Err(AgentError::InvalidValue(
                "Cannot convert AgentValue to Message".to_string(),
            ))
        }
    }
}

impl From<Message> for AgentData {
    fn from(msg: Message) -> Self {
        AgentData::object_with_kind(
            "message",
            [
                ("role".to_string(), AgentValue::string(msg.role)),
                ("content".to_string(), AgentValue::string(msg.content)),
            ]
            .into(),
        )
    }
}

impl From<Message> for AgentValue {
    fn from(msg: Message) -> Self {
        AgentValue::object(
            [
                ("role".to_string(), AgentValue::string(msg.role)),
                ("content".to_string(), AgentValue::string(msg.content)),
            ]
            .into(),
        )
    }
}

#[derive(Clone, Default)]
pub struct MessageHistory {
    pub messages: Vec<Message>,
    max_size: i64,
}

impl MessageHistory {
    pub fn new(messages: Vec<Message>, max_size: i64) -> Self {
        let mut messages = messages;
        if max_size > 0 {
            if messages.len() > max_size as usize {
                messages = messages[messages.len() - max_size as usize..].to_vec();
            }
        } else {
            messages = Vec::new();
        }
        Self { messages, max_size }
    }

    pub fn push(&mut self, message: Message) {
        if self.max_size > 0 && self.messages.len() >= self.max_size as usize {
            self.messages.remove(0);
        }
        self.messages.push(message);
    }
}

impl From<MessageHistory> for AgentData {
    fn from(history: MessageHistory) -> Self {
        AgentData::array(
            "message",
            history.messages.into_iter().map(|m| m.into()).collect(),
        )
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_message_to_from_agent_data() {
        let msg = Message::assistant("Hello, how can I help you?".to_string());
        let data: AgentData = msg.clone().into();
        let msg_converted: Message = data.try_into().unwrap();
        assert_eq!(msg.role, msg_converted.role);
        assert_eq!(msg.content, msg_converted.content);
    }

    #[test]
    fn test_message_to_from_agent_value() {
        let msg = Message::user("What is the weather today?".to_string());
        let value: AgentValue = msg.clone().into();
        let msg_converted: Message = value.try_into().unwrap();
        assert_eq!(msg.role, msg_converted.role);
        assert_eq!(msg.content, msg_converted.content);
    }

    #[test]
    fn test_message_from_string_value() {
        let value = AgentValue::string("Just a simple message");
        let msg: Message = value.try_into().unwrap();
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Just a simple message");
    }

    #[test]
    fn test_message_from_object_value() {
        let value = AgentValue::object(
            [
                ("role".to_string(), AgentValue::string("assistant")),
                (
                    "content".to_string(),
                    AgentValue::string("Here is some information."),
                ),
            ]
            .into(),
        );
        let msg: Message = value.try_into().unwrap();
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "Here is some information.");
    }

    #[test]
    fn test_message_from_invalid_value() {
        let value = AgentValue::integer(42);
        let result: Result<Message, AgentError> = value.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn test_message_invalid_object() {
        let value =
            AgentValue::object([("some_key".to_string(), AgentValue::string("some_value"))].into());
        let result: Result<Message, AgentError> = value.try_into();
        assert!(result.is_err());
    }
}
