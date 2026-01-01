use std::{sync::Arc, vec};

use agent_stream_kit::{AgentError, AgentValue};
use serde::{Deserialize, Serialize};

#[cfg(feature = "image")]
use photon_rs::PhotonImage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    pub role: String,

    pub content: String,

    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub thinking: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,

    #[cfg(feature = "image")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<Arc<PhotonImage>>,
}

impl Message {
    pub fn new(role: String, content: String) -> Self {
        Self {
            id: None,
            role,
            content,
            thinking: String::new(),
            tool_calls: None,
            tool_name: None,

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

    pub fn tool(tool_name: String, content: String) -> Self {
        let mut message = Message::new("tool".to_string(), content);
        message.tool_name = Some(tool_name);
        message
    }

    #[cfg(feature = "image")]
    pub fn with_image(mut self, image: Arc<PhotonImage>) -> Self {
        self.image = Some(image);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub parameters: serde_json::Value,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
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

                let thinking = obj
                    .get("thinking")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                message.thinking = thinking;

                if let Some(tool_name) = obj.get("tool_name") {
                    message.tool_name = Some(
                        tool_name
                            .as_str()
                            .ok_or_else(|| {
                                AgentError::InvalidValue(
                                    "'tool_name' field must be a string".to_string(),
                                )
                            })?
                            .to_string(),
                    );
                }

                if let Some(tool_calls) = obj.get("tool_calls") {
                    let mut calls = vec![];
                    for call_value in tool_calls.as_array().ok_or_else(|| {
                        AgentError::InvalidValue("'tool_calls' field must be an array".to_string())
                    })? {
                        let id = call_value
                            .get("id")
                            .and_then(|i| i.as_str())
                            .map(|s| s.to_string());
                        let function = call_value.get("function").ok_or_else(|| {
                            AgentError::InvalidValue(
                                "Tool call missing 'function' field".to_string(),
                            )
                        })?;
                        let tool_name = function.get_str("name").ok_or_else(|| {
                            AgentError::InvalidValue(
                                "Tool call function missing 'name' field".to_string(),
                            )
                        })?;
                        let parameters = function.get("parameters").ok_or_else(|| {
                            AgentError::InvalidValue(
                                "Tool call function missing 'parameters' field".to_string(),
                            )
                        })?;
                        let call = ToolCall {
                            function: ToolCallFunction {
                                id,
                                name: tool_name.to_string(),
                                parameters: parameters.to_json(),
                            },
                        };
                        calls.push(call);
                    }
                    message.tool_calls = Some(calls);
                }

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

impl From<Message> for AgentValue {
    fn from(msg: Message) -> Self {
        let mut fields = vec![
            ("role".to_string(), AgentValue::string(msg.role)),
            ("content".to_string(), AgentValue::string(msg.content)),
        ];
        if let Some(id_str) = msg.id {
            fields.push(("id".to_string(), AgentValue::string(id_str)));
        }
        if !msg.thinking.is_empty() {
            fields.push(("thinking".to_string(), AgentValue::string(msg.thinking)));
        }
        if let Some(tool_calls) = msg.tool_calls {
            let calls_value = AgentValue::array(
                tool_calls
                    .into_iter()
                    .map(|call| {
                        AgentValue::from_serialize(&call).unwrap_or_else(|_| AgentValue::unit())
                    })
                    .collect(),
            );
            fields.push(("tool_calls".to_string(), calls_value));
        }
        if let Some(tool_name) = msg.tool_name {
            fields.push(("tool_name".to_string(), AgentValue::string(tool_name)));
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
    max_size: usize,
    system_message: Option<Message>,
    include_system: bool,
}

impl MessageHistory {
    pub fn new(messages: Vec<Message>, max_size: usize) -> Self {
        let mut hist = Self {
            messages,
            max_size: 0,
            system_message: None,
            include_system: false,
        };
        hist.set_max_size(max_size);
        hist
    }

    pub fn from_value(value: AgentValue) -> Result<Self, AgentError> {
        let mut messages = vec![];

        if value.is_array() {
            let Some(arr) = value.as_array() else {
                return Ok(MessageHistory::new(messages, 0));
            };
            for v in arr {
                let msg: Message = v.clone().try_into()?;
                messages.push(msg);
            }
            return Ok(MessageHistory::new(messages, 0));
        }

        if let Ok(msg) = value.clone().try_into() {
            messages.push(msg);
            return Ok(MessageHistory::new(messages, 0));
        }

        if value.is_object() {
            if let Some(arr) = value.get_array("history") {
                for v in arr {
                    let msg: Message = v.clone().try_into()?;
                    messages.push(msg);
                }
            }
            if let Some(msg) = value.get("message") {
                let msg: Message = msg.clone().try_into()?;
                messages.push(msg);
            }
            if !messages.is_empty() {
                return Ok(MessageHistory::new(messages, 0));
            }
        }

        Err(AgentError::InvalidValue(
            "Cannot convert AgentValue to MessageHistory".to_string(),
        ))
    }

    /// Create MessageHistory from a JSON value
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

    /// Parse MessageHistory from a JSON string
    pub fn parse(s: &str) -> Result<Self, AgentError> {
        let value: serde_json::Value = serde_json::from_str(s).map_err(|e| {
            AgentError::InvalidValue(format!("Failed to parse JSON for MessageHistory: {}", e))
        })?;
        Self::from_json(value)
    }

    /// Get the messages in the history, including system message if configured.
    pub fn messages(&self) -> Vec<Message> {
        let mut msgs = Vec::new();
        if self.include_system {
            if let Some(sys_msg) = &self.system_message {
                msgs.push(sys_msg.clone());
            }
            msgs.extend(self.messages.clone());
            msgs
        } else {
            self.messages.clone()
        }
    }

    /// Get the messages for prompt, excluding thinking.
    pub fn messages_for_prompt(&self) -> Vec<Message> {
        let mut msgs = Vec::new();
        if self.include_system {
            if let Some(sys_msg) = &self.system_message {
                msgs.push(sys_msg.clone());
            }
            for msg in &self.messages {
                let mut m = msg.clone();
                m.thinking = String::new();
                msgs.push(m);
            }
            msgs
        } else {
            self.messages.clone()
        }
    }

    pub fn include_system(&self) -> bool {
        self.include_system
    }

    pub fn set_include_system(&mut self, include: bool) {
        self.include_system = include;
    }

    /// Set the maximum size of the message history.
    /// If max_size is 0, there is no limit.
    /// If the current size exceeds the new maximum, the oldest messages will be removed.
    /// If include_system is true, the system message will be preserved.
    pub fn set_max_size(&mut self, size: usize) {
        self.max_size = size;
        if self.max_size > 0 && self.messages.len() > self.max_size {
            if self.include_system {
                // find system message if it will be excluded from history
                for i in 0..(self.messages.len() - self.max_size) {
                    if self.messages[i].role == "system" {
                        self.system_message = Some(self.messages[i].clone());
                        break;
                    }
                }
            }
            self.messages = self.messages[self.messages.len() - self.max_size..].to_vec();
        }
    }

    pub fn set_preamble(&mut self, preamble: Vec<Message>) {
        if preamble.is_empty() {
            return;
        }
        let mut msgs = vec![];
        msgs.extend(preamble.clone());
        msgs.extend(self.messages.clone());
        self.messages = msgs;
        self.system_message = None;
        self.set_max_size(self.max_size);
    }

    /// Push a new message to the history.
    /// If the message has the same ID as the last message, it will update the last message instead.
    /// If the history exceeds max_size, the oldest message will be removed.
    /// If include_system is true and the removed message is a system message, it will be preserved.
    pub fn push(&mut self, message: Message) {
        // If the message is the same as the last one, update it instead of adding a new one
        if message.id.is_some() && !self.messages.is_empty() {
            let last_index = self.messages.len() - 1;
            let last_message = &mut self.messages[last_index];
            if last_message.id.is_some() && last_message.id == message.id {
                last_message.content = message.content;
                last_message.thinking = message.thinking;
                last_message.tool_calls = message.tool_calls;
                return;
            }
        }

        if self.max_size > 0 && self.messages.len() >= self.max_size {
            let m = self.messages.remove(0);
            if m.role == "system" {
                self.system_message = Some(m);
            }
        }
        self.messages.push(message);
    }

    /// Push multiple messages to the history.
    pub fn push_all(&mut self, messages: Vec<Message>) {
        for msg in messages {
            self.push(msg);
        }
    }
}

impl From<MessageHistory> for AgentValue {
    fn from(history: MessageHistory) -> Self {
        AgentValue::array(history.messages.into_iter().map(|m| m.into()).collect())
    }
}

impl TryFrom<AgentValue> for MessageHistory {
    type Error = AgentError;

    fn try_from(value: AgentValue) -> Result<Self, Self::Error> {
        MessageHistory::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use im::{hashmap, vector};

    use super::*;

    // Message tests

    #[test]
    fn test_message_to_from_agent_value() {
        let msg = Message::user("What is the weather today?".to_string());

        let value: AgentValue = msg.into();
        assert_eq!(value.as_object().is_some(), true);
        assert_eq!(value.get_str("role").unwrap(), "user");
        assert_eq!(
            value.get_str("content").unwrap(),
            "What is the weather today?"
        );

        let msg_converted: Message = value.try_into().unwrap();
        assert_eq!(msg_converted.role, "user");
        assert_eq!(msg_converted.content, "What is the weather today?");
    }

    #[test]
    fn test_message_with_tool_calls_to_from_agent_value() {
        let mut msg = Message::assistant("".to_string());
        msg.tool_calls = Some(vec![ToolCall {
            function: ToolCallFunction {
                id: Some("call1".to_string()),
                name: "get_weather".to_string(),
                parameters: serde_json::json!({"location": "San Francisco"}),
            },
        }]);

        let value: AgentValue = msg.into();
        assert_eq!(value.as_object().is_some(), true);
        assert_eq!(value.get_str("role").unwrap(), "assistant");
        assert_eq!(value.get_str("content").unwrap(), "");
        let tool_calls = value.get_array("tool_calls").unwrap();
        assert_eq!(tool_calls.len(), 1);
        let first_call = tool_calls[0].as_object().unwrap();
        let function = first_call.get("function").unwrap();
        assert_eq!(function.get_str("name").unwrap(), "get_weather");
        let parameters = function.get("parameters").unwrap();
        assert_eq!(parameters.get_str("location").unwrap(), "San Francisco");

        let msg_converted: Message = value.try_into().unwrap();
        dbg!(&msg_converted);
        assert_eq!(msg_converted.role, "assistant");
        assert_eq!(msg_converted.content, "");
        let tool_calls = msg_converted.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].function.name, "get_weather");
        assert_eq!(
            tool_calls[0].function.parameters,
            serde_json::json!({"location": "San Francisco"})
        );
    }

    #[test]
    fn test_tool_message_to_from_agent_value() {
        let msg = Message::tool("get_time".to_string(), "2025-01-02 03:04:05".to_string());

        let value: AgentValue = msg.clone().into();
        assert_eq!(value.get_str("role").unwrap(), "tool");
        assert_eq!(value.get_str("tool_name").unwrap(), "get_time");
        assert_eq!(value.get_str("content").unwrap(), "2025-01-02 03:04:05");

        let msg_converted: Message = value.try_into().unwrap();
        assert_eq!(msg_converted.role, "tool");
        assert_eq!(msg_converted.tool_name.unwrap(), "get_time");
        assert_eq!(msg_converted.content, "2025-01-02 03:04:05");
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
        let value = AgentValue::object(hashmap! {
            "role".into() => AgentValue::string("assistant"),
                "content".into() =>
                AgentValue::string("Here is some information."),
        });
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
            AgentValue::object(hashmap! {"some_key".into() => AgentValue::string("some_value")});
        let result: Result<Message, AgentError> = value.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn test_message_to_agent_value_with_tool_calls() {
        let message = Message {
            role: "assistant".to_string(),
            content: "".to_string(),
            thinking: "".to_string(),
            tool_calls: Some(vec![ToolCall {
                function: ToolCallFunction {
                    id: Some("call1".to_string()),
                    name: "active_applications".to_string(),
                    parameters: serde_json::json!({}),
                },
            }]),
            id: None,
            tool_name: None,
            #[cfg(feature = "image")]
            image: None,
        };

        let value: AgentValue = message.into();
        let value_obj = value
            .as_object()
            .expect("message converts to object AgentValue");

        assert_eq!(
            value_obj.get("role").and_then(|v| v.as_str()),
            Some("assistant")
        );
        assert_eq!(value_obj.get("content").and_then(|v| v.as_str()), Some(""));

        let tool_calls = value_obj
            .get("tool_calls")
            .and_then(|v| v.as_array())
            .expect("tool_calls should be serialized");
        assert_eq!(tool_calls.len(), 1);

        let first_call = tool_calls[0]
            .as_object()
            .expect("tool call should serialize as object");
        let function_obj = first_call
            .get("function")
            .and_then(|v| v.as_object())
            .expect("function should be serialized");

        assert_eq!(
            function_obj.get("name").and_then(|v| v.as_str()),
            Some("active_applications")
        );
        let parameters = function_obj
            .get("parameters")
            .and_then(|v| v.as_object())
            .expect("parameters should serialize as object");
        assert!(parameters.is_empty());
    }

    // MessageHistory tests

    const SAMPLE_HISTORY: &str = r#"
    [
        { "role": "system", "content": "You are a helpful assistant." },
        { "role": "user", "content": "Hello" },
        { "role": "assistant", "content": "Hi there!" }
    ]"#;

    #[test]
    fn test_message_history_new() {
        let history = MessageHistory::new(vec![], 0);
        assert_eq!(history.messages.len(), 0);
        assert_eq!(history.max_size, 0);
        assert_eq!(history.include_system, false);
        assert!(history.system_message.is_none());
    }

    #[test]
    fn test_message_history_from_value_array() {
        let value = AgentValue::array(vector![
            AgentValue::object(hashmap! {
                "role".into() => AgentValue::string("user"),
                "content".into() => AgentValue::string("Hello"),
            }),
            AgentValue::object(hashmap! {
                "role".into() => AgentValue::string("assistant"),
                "content".into() => AgentValue::string("Hi there!"),
            }),
        ]);

        let history = MessageHistory::from_value(value).unwrap();
        assert_eq!(history.messages.len(), 2);
        assert_eq!(history.messages[0].role, "user");
        assert_eq!(history.messages[1].role, "assistant");
    }

    #[test]
    fn test_message_history_from_value_single_message_object() {
        let value = AgentValue::object(hashmap! {
            "role".into() => AgentValue::string("user"),
            "content".into() => AgentValue::string("Solo message"),
        });

        let history = MessageHistory::from_value(value).unwrap();
        assert_eq!(history.messages.len(), 1);
        assert_eq!(history.messages[0].role, "user");
        assert_eq!(history.messages[0].content, "Solo message");
    }

    #[test]
    fn test_message_history_from_value_history_and_message_fields() {
        let value = AgentValue::object(hashmap! {
            "history".into() =>
            AgentValue::array(vector![AgentValue::object(hashmap! {
                "role".into() => AgentValue::string("system"),
                "content".into() => AgentValue::string("You are a helpful assistant."),
            })]),
            "message".into() =>
            AgentValue::object(hashmap! {
                "role".into() => AgentValue::string("user"),
                "content".into() => AgentValue::string("Hello"),
            }),
        });

        let history = MessageHistory::from_value(value).unwrap();
        assert_eq!(history.messages.len(), 2);
        assert_eq!(history.messages[0].role, "system");
        assert_eq!(history.messages[1].role, "user");
        assert_eq!(history.messages[1].content, "Hello");
    }

    #[test]
    fn test_message_history_from_value_invalid() {
        let result = MessageHistory::from_value(AgentValue::integer(42));
        assert!(result.is_err());
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
        let history = MessageHistory::parse(SAMPLE_HISTORY).unwrap();
        assert_eq!(history.messages.len(), 3);
        assert_eq!(history.messages[0].role, "system");
        assert_eq!(history.messages[0].content, "You are a helpful assistant.");
        assert_eq!(history.messages[1].role, "user");
        assert_eq!(history.messages[1].content, "Hello");
        assert_eq!(history.messages[2].role, "assistant");
        assert_eq!(history.messages[2].content, "Hi there!");
    }

    #[test]
    fn test_message_history_include_system() {
        let mut history = MessageHistory::new(vec![], 0);
        assert_eq!(history.include_system(), false);
        history.set_include_system(true);
        assert_eq!(history.include_system(), true);
    }

    #[test]
    fn test_message_history_set_max_size() {
        let mut history = MessageHistory::parse(SAMPLE_HISTORY).unwrap();
        assert_eq!(history.max_size, 0);
        assert_eq!(history.messages.len(), 3);

        history.set_max_size(5);
        assert_eq!(history.max_size, 5);
        assert_eq!(history.messages.len(), 3);

        history.set_max_size(1);
        assert_eq!(history.max_size, 1);
        let msgs = history.messages();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, "assistant");
    }

    #[test]
    fn test_message_history_set_max_size_with_include_system() {
        let mut history = MessageHistory::parse(SAMPLE_HISTORY).unwrap();
        history.set_include_system(true);
        assert_eq!(history.max_size, 0);
        assert_eq!(history.messages.len(), 3);

        history.set_max_size(5);
        assert_eq!(history.max_size, 5);
        assert_eq!(history.messages.len(), 3);

        history.set_max_size(1);
        assert_eq!(history.max_size, 1);
        assert_eq!(history.messages.len(), 1);
        assert!(history.system_message.is_some());
        let msgs = history.messages();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "system");
        assert_eq!(msgs[1].role, "assistant");
    }

    #[test]
    fn test_message_history_push() {
        let mut history = MessageHistory::parse(SAMPLE_HISTORY).unwrap();
        let new_msg = Message::user("How are you?".to_string());
        history.push(new_msg);
        let msgs = history.messages();
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[3].role, "user");
        assert_eq!(msgs[3].content, "How are you?");
    }

    #[test]
    fn test_message_history_push_with_include_system() {
        let mut history = MessageHistory::parse(SAMPLE_HISTORY).unwrap();
        history.set_include_system(true);
        history.set_max_size(3);
        assert_eq!(history.messages.len(), 3);
        assert!(history.system_message.is_none());

        let new_msg = Message::user("How are you?".to_string());
        history.push(new_msg);
        assert_eq!(history.messages.len(), 3);
        assert!(history.system_message.is_some());

        let msgs = history.messages();
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[0].role, "system");
        assert_eq!(msgs[3].role, "user");

        let new_msg = Message::assistant("Good!".to_string());
        history.push(new_msg);
        assert_eq!(history.messages.len(), 3);
        assert!(history.system_message.is_some());

        let msgs = history.messages();
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[0].role, "system");
        assert_eq!(msgs[3].role, "assistant");
    }

    #[test]
    fn test_message_history_push_update_last() {
        let mut history =
            MessageHistory::parse(r#"[{"role": "user", "content": "Hello", "id": "msg1"}]"#)
                .unwrap();
        let updated_msg = Message {
            role: "user".to_string(),
            content: "Hello, updated!".to_string(),
            id: Some("msg1".to_string()),
            thinking: "".to_string(),
            tool_calls: None,
            tool_name: None,
            #[cfg(feature = "image")]
            image: None,
        };
        history.push(updated_msg);
        let msgs = history.messages();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].content, "Hello, updated!");
    }
}
