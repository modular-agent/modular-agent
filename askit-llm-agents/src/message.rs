use agent_stream_kit::{AgentData, AgentError, AgentValue, AgentValueMap};
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
        AgentData::new_custom_object(
            "message",
            AgentValueMap::from([
                ("role".to_string(), AgentValue::new_string(msg.role)),
                ("content".to_string(), AgentValue::new_string(msg.content)),
            ]),
        )
    }
}

impl From<Message> for AgentValue {
    fn from(msg: Message) -> Self {
        AgentValue::new_object(AgentValueMap::from([
            ("role".to_string(), AgentValue::new_string(msg.role)),
            ("content".to_string(), AgentValue::new_string(msg.content)),
        ]))
    }
}

#[cfg(test)]
mod tests {
    use agent_stream_kit::AgentValueMap;

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
        let value = AgentValue::new_string("Just a simple message");
        let msg: Message = value.try_into().unwrap();
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Just a simple message");
    }

    #[test]
    fn test_message_from_object_value() {
        let value = AgentValue::new_object(AgentValueMap::from([
            ("role".to_string(), AgentValue::new_string("assistant")),
            (
                "content".to_string(),
                AgentValue::new_string("Here is some information."),
            ),
        ]));
        let msg: Message = value.try_into().unwrap();
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "Here is some information.");
    }

    #[test]
    fn test_message_from_invalid_value() {
        let value = AgentValue::new_integer(42);
        let result: Result<Message, AgentError> = value.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn test_message_invalid_object() {
        let value = AgentValue::new_object(AgentValueMap::from([(
            "some_key".to_string(),
            AgentValue::new_string("some_value"),
        )]));
        let result: Result<Message, AgentError> = value.try_into();
        assert!(result.is_err());
    }
}
