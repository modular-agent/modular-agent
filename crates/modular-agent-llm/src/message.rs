use std::sync::Arc;

use agent_stream_kit::{AgentData, AgentError, AgentValue};
use serde::{Deserialize, Serialize};

#[cfg(feature = "image")]
use photon_rs::PhotonImage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,

    pub content: String,

    pub id: Option<String>,

    #[cfg(feature = "image")]
    pub image: Option<Arc<PhotonImage>>,
}

impl Message {
    pub fn new(role: String, content: String) -> Self {
        Self {
            role,
            content,
            id: None,

            #[cfg(feature = "image")]
            image: None,
        }
    }

    pub fn assistant(content: String) -> Self {
        Message::new("assistant".to_string(), content)
    }

    pub fn system(content: String) -> Self {
        Message::new("system".to_string(), content)
    }

    pub fn user(content: String) -> Self {
        Message::new("user".to_string(), content)
    }

    #[cfg(feature = "image")]
    pub fn with_image(mut self, image: Arc<PhotonImage>) -> Self {
        self.image = Some(image);
        self
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
        match value {
            AgentValue::String(s) => Ok(Message::user(s.to_string())),

            #[cfg(feature = "image")]
            AgentValue::Image(img) => {
                let mut message = Message::user("".to_string());
                message.image = Some(img.clone());
                Ok(message)
            }
            AgentValue::Object(obj) => {
                let role = obj
                    .get("role")
                    .and_then(|r| r.as_str())
                    .unwrap_or("user")
                    .to_string();
                let content = obj
                    .get("content")
                    .and_then(|c| c.as_str())
                    .ok_or_else(|| {
                        AgentError::InvalidValue(
                            "Message object missing 'content' field".to_string(),
                        )
                    })?
                    .to_string();
                let id = obj
                    .get("id")
                    .and_then(|i| i.as_str())
                    .map(|s| s.to_string());
                let mut message = Message::new(role, content);
                message.id = id;

                #[cfg(feature = "image")]
                {
                    if let Some(image_value) = obj.get("image") {
                        match image_value {
                            AgentValue::String(s) => {
                                message.image = Some(Arc::new(PhotonImage::new_from_base64(
                                    s.trim_start_matches("data:image/png;base64,"),
                                )));
                            }
                            AgentValue::Image(img) => {
                                message.image = Some(img.clone());
                            }
                            _ => {}
                        }
                    }
                }

                Ok(message)
            }
            _ => Err(AgentError::InvalidValue(
                "Cannot convert AgentValue to Message".to_string(),
            )),
        }
    }
}

impl From<Message> for AgentData {
    fn from(msg: Message) -> Self {
        let mut fields = vec![
            ("role".to_string(), AgentValue::string(msg.role)),
            ("content".to_string(), AgentValue::string(msg.content)),
        ];
        if let Some(id_str) = msg.id {
            fields.push(("id".to_string(), AgentValue::string(id_str)));
        }
        #[cfg(feature = "image")]
        {
            if let Some(img) = msg.image {
                fields.push(("image".to_string(), AgentValue::image((*img).clone())));
            }
        }
        AgentData::object_with_kind("message", fields.into_iter().collect())
    }
}

impl From<Message> for AgentValue {
    fn from(msg: Message) -> Self {
        let mut fields = vec![
            ("role".to_string(), AgentValue::string(msg.role)),
            ("content".to_string(), AgentValue::string(msg.content)),
        ];
        if let Some(id_str) = msg.id {
            fields.push(("id".to_string(), AgentValue::string(id_str)));
        }
        #[cfg(feature = "image")]
        {
            if let Some(img) = msg.image {
                fields.push(("image".to_string(), AgentValue::image((*img).clone())));
            }
        }
        AgentValue::object(fields.into_iter().collect())
    }
}

#[derive(Clone, Default, Debug)]
pub struct MessageHistory {
    messages: Vec<Message>,
    max_size: i64,
    system_message: Option<Message>,
    include_system: bool,
}

impl MessageHistory {
    pub fn new(messages: Vec<Message>, max_size: i64) -> Self {
        let mut messages = messages;
        let mut system_message = None;
        if max_size > 0 {
            if messages.len() > max_size as usize {
                // find system message if it will be excluded from history
                for i in 0..(max_size - 1) as usize {
                    if messages[i].role == "system" {
                        system_message = Some(messages[i].clone());
                        break;
                    }
                }
                messages = messages[messages.len() - max_size as usize..].to_vec();
            }
        }
        Self {
            messages,
            max_size,
            system_message,
            include_system: false,
        }
    }

    pub fn from_json(value: serde_json::Value) -> Result<Self, AgentError> {
        match value {
            serde_json::Value::Array(arr) => {
                let messages: Vec<Message> = arr
                    .into_iter()
                    .map(|v| serde_json::from_value(v))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| {
                        AgentError::InvalidValue(format!("Invalid message format: {}", e))
                    })?;
                Ok(MessageHistory::new(messages, 0))
            }
            _ => Err(AgentError::InvalidValue(
                "Expected JSON array for MessageHistory".to_string(),
            )),
        }
    }

    pub fn parse(s: &str) -> Result<Self, AgentError> {
        let value: serde_json::Value = serde_json::from_str(s).map_err(|e| {
            AgentError::InvalidValue(format!("Failed to parse JSON for MessageHistory: {}", e))
        })?;
        Self::from_json(value)
    }

    pub fn include_system(&mut self, include: bool) {
        self.include_system = include;
    }

    pub fn push(&mut self, message: Message) {
        // If the message is the same as the last one, update it instead of adding a new one
        if !self.messages.is_empty() {
            let last_index = self.messages.len() - 1;
            let last_message = &mut self.messages[last_index];
            if last_message.id == message.id {
                last_message.content = message.content;
                return;
            }
        }

        if self.max_size > 0 && self.messages.len() >= self.max_size as usize {
            self.messages.remove(0);
        }
        self.messages.push(message);
    }

    pub fn reset(&mut self) {
        self.messages.clear();
    }

    pub fn set_size(&mut self, size: i64) {
        self.max_size = size;
        if self.max_size > 0 && self.messages.len() > self.max_size as usize {
            // find system message if it will be excluded from history
            for i in 0..(self.max_size - 1) as usize {
                if self.messages[i].role == "system" {
                    self.system_message = Some(self.messages[i].clone());
                    break;
                }
            }
            self.messages = self.messages[self.messages.len() - self.max_size as usize..].to_vec();
        }
    }

    pub fn messages(&self) -> Vec<Message> {
        if self.include_system {
            let mut msgs = Vec::new();
            if let Some(sys_msg) = &self.system_message {
                msgs.push(sys_msg.clone());
            }
            msgs.extend(self.messages.clone());
            msgs
        } else {
            self.messages.clone()
        }
    }
}

impl From<MessageHistory> for AgentData {
    fn from(history: MessageHistory) -> Self {
        AgentData::array(
            "message",
            history.messages().into_iter().map(|m| m.into()).collect(),
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
    fn test_message_history_from_json() {
        let value: serde_json::Value = serde_json::json!([
            { "role": "user", "content": "Hello" },
            { "role": "assistant", "content": "Hi there!" }
        ]);
        let history = MessageHistory::from_json(value).unwrap();
        assert_eq!(history.messages.len(), 2);
        assert_eq!(history.messages[0].role, "user");
        assert_eq!(history.messages[0].content, "Hello");
        assert_eq!(history.messages[1].role, "assistant");
        assert_eq!(history.messages[1].content, "Hi there!");
    }

    #[test]
    fn test_message_history_parse() {
        let history = MessageHistory::parse(
            r#"[{"role": "user", "content": "Hello"}, {"role": "assistant", "content": "Hi there!"}]"#,
        ).unwrap();
        assert_eq!(history.messages.len(), 2);
        assert_eq!(history.messages[0].role, "user");
        assert_eq!(history.messages[0].content, "Hello");
        assert_eq!(history.messages[1].role, "assistant");
        assert_eq!(history.messages[1].content, "Hi there!");
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
