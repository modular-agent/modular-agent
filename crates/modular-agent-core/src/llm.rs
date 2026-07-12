//! LLM message types for agent-based workflows.
//!
//! This module provides types for representing chat messages in LLM conversations,
//! including support for tool calls, streaming responses, and multimodal content.

#![cfg(feature = "llm")]

use std::{sync::Arc, vec};

use im::Vector;
use serde::{Deserialize, Serialize};

use crate::error::AgentError;
use crate::value::AgentValue;

#[cfg(feature = "image")]
use photon_rs::PhotonImage;

/// A chat message in an LLM conversation.
///
/// Represents messages exchanged between users, assistants, and tools in a conversation.
/// Supports various roles (user, assistant, system, tool) and optional features like
/// streaming, thinking traces, and attached images.
///
/// # Fields
///
/// * `id` - Optional unique identifier for the message
/// * `role` - The role of the message sender ("user", "assistant", "system", "tool")
/// * `content` - The text content of the message
/// * `tokens` - Optional token count for the message
/// * `thinking` - Optional reasoning/thinking trace (for extended thinking models)
/// * `streaming` - Whether this is a partial streaming response
/// * `tool_calls` - Tool invocations requested by the assistant
/// * `tool_name` - Name of the tool (for tool role messages)
/// * `is_error` - Marks a tool-result message as an error
/// * `image` - Optional attached image (requires "image" feature)
///
/// # Example
///
/// ```
/// use modular_agent_core::Message;
///
/// let user_msg = Message::user("What is the weather?".to_string());
/// let assistant_msg = Message::assistant("The weather is sunny.".to_string());
/// let system_msg = Message::system("You are a helpful assistant.".to_string());
/// ```
#[derive(Debug, Default, Clone)]
pub struct Message {
    /// Unique identifier for this message.
    pub id: Option<String>,

    /// Role of the message sender: "user", "assistant", "system", or "tool".
    pub role: String,

    /// Text content of the message.
    pub content: String,

    /// Token count for this message (if available).
    pub tokens: Option<usize>,

    /// Reasoning/thinking trace for extended thinking models.
    pub thinking: Option<String>,

    /// Whether this is a partial streaming response.
    pub streaming: bool,

    /// Tool calls requested by the assistant in this message.
    pub tool_calls: Option<Vector<ToolCall>>,

    /// Name of the tool (for tool role messages containing tool results).
    pub tool_name: Option<String>,

    /// Marks a tool-result message as an error, per Claude's `tool_result` `is_error`.
    pub is_error: Option<bool>,

    /// Attached image for multimodal messages (requires "image" feature).
    #[cfg(feature = "image")]
    pub image: Option<Arc<PhotonImage>>,
}

impl Message {
    /// Creates a new message with the specified role and content.
    ///
    /// # Arguments
    ///
    /// * `role` - The role of the message sender
    /// * `content` - The text content of the message
    pub fn new(role: String, content: String) -> Self {
        Self {
            id: None,
            role,
            content,
            tokens: None,
            streaming: false,
            thinking: None,
            tool_calls: None,
            tool_name: None,
            is_error: None,

            #[cfg(feature = "image")]
            image: None,
        }
    }

    /// Creates an assistant message with the given content.
    pub fn assistant(content: String) -> Self {
        Message::new("assistant".to_string(), content)
    }

    /// Creates a system message with the given content.
    ///
    /// System messages typically set the behavior or context for the assistant.
    pub fn system(content: String) -> Self {
        Message::new("system".to_string(), content)
    }

    /// Creates a user message with the given content.
    pub fn user(content: String) -> Self {
        Message::new("user".to_string(), content)
    }

    /// Creates a tool response message.
    ///
    /// Tool messages contain the result of a tool call and are associated
    /// with a specific tool by name.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - The name of the tool that produced this result
    /// * `content` - The tool's output/result as a string
    pub fn tool(tool_name: String, content: String) -> Self {
        let mut message = Message::new("tool".to_string(), content);
        message.tool_name = Some(tool_name);
        message
    }

    /// Attaches an image to this message (builder pattern).
    ///
    /// Only available when the "image" feature is enabled.
    #[cfg(feature = "image")]
    pub fn with_image(mut self, image: Arc<PhotonImage>) -> Self {
        self.image = Some(image);
        self
    }
}

impl PartialEq for Message {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.role == other.role && self.content == other.content
    }
}

impl Serialize for Message {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serde_json::Map::new();
        if let Some(id) = &self.id {
            map.insert("id".to_string(), serde_json::Value::String(id.clone()));
        }
        map.insert(
            "role".to_string(),
            serde_json::Value::String(self.role.clone()),
        );
        map.insert(
            "content".to_string(),
            serde_json::Value::String(self.content.clone()),
        );
        if let Some(tokens) = &self.tokens {
            map.insert(
                "tokens".to_string(),
                serde_json::Value::Number((*tokens).into()),
            );
        }
        if let Some(thinking) = &self.thinking {
            map.insert(
                "thinking".to_string(),
                serde_json::Value::String(thinking.clone()),
            );
        }
        if self.streaming {
            map.insert("streaming".to_string(), serde_json::Value::Bool(true));
        }
        if let Some(tool_calls) = &self.tool_calls {
            let mut tool_calls_vec = vec![];
            for call in tool_calls {
                tool_calls_vec.push(serde_json::to_value(call).map_err(serde::ser::Error::custom)?);
            }
            map.insert(
                "tool_calls".to_string(),
                serde_json::Value::Array(tool_calls_vec),
            );
        }
        if let Some(tool_name) = &self.tool_name {
            map.insert(
                "tool_name".to_string(),
                serde_json::Value::String(tool_name.clone()),
            );
        }
        // Only emitted when set, so presets saved before this field existed
        // round-trip unchanged.
        if let Some(is_error) = &self.is_error {
            map.insert("is_error".to_string(), serde_json::Value::Bool(*is_error));
        }
        #[cfg(feature = "image")]
        {
            if let Some(image) = &self.image {
                map.insert(
                    "image".to_string(),
                    serde_json::Value::String(image.get_base64()),
                );
            }
        }
        map.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Message {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut message = Message::user(String::default());
        let map = serde_json::Map::deserialize(deserializer)?;

        if let Some(id) = map.get("id") {
            message.id = id.as_str().map(|s| s.to_string());
        }
        if let Some(role) = map.get("role") {
            message.role = role
                .as_str()
                .ok_or_else(|| serde::de::Error::custom("role must be a string"))?
                .to_string();
        }
        if let Some(content) = map.get("content") {
            message.content = content
                .as_str()
                .ok_or_else(|| serde::de::Error::custom("content must be a string"))?
                .to_string();
        }
        if let Some(tokens) = map.get("tokens") {
            message.tokens = tokens.as_u64().map(|u| u as usize);
        }
        if let Some(thinking) = map.get("thinking") {
            message.thinking = thinking.as_str().map(|s| s.to_string());
        }
        if let Some(streaming) = map.get("streaming") {
            message.streaming = streaming.as_bool().unwrap_or(false);
        }
        if let Some(tool_calls) = map.get("tool_calls") {
            let tool_calls = serde_json::from_value::<Vec<ToolCall>>(tool_calls.clone())
                .map_err(|e| serde::de::Error::custom(e.to_string()))?;
            message.tool_calls = Some(tool_calls.into());
        }
        if let Some(tool_name) = map.get("tool_name") {
            message.tool_name = tool_name.as_str().map(|s| s.to_string());
        }
        message.is_error = map.get("is_error").and_then(|v| v.as_bool());
        #[cfg(feature = "image")]
        if let Some(image) = map.get("image") {
            let image_str = image
                .as_str()
                .ok_or_else(|| serde::de::Error::custom("image must be a string"))?;
            let image = Arc::new(PhotonImage::new_from_base64(image_str));
            message.image = Some(image);
        }
        Ok(message)
    }
}

/// A tool call requested by the assistant.
///
/// Represents a single tool invocation as part of an LLM response.
/// The assistant may request multiple tool calls in a single message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// The function to be called.
    pub function: ToolCallFunction,
}

/// Details of a function call within a tool invocation.
///
/// Contains the function name, parameters, and optional call ID
/// for correlating tool calls with their results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    /// Name of the function/tool to invoke.
    pub name: String,

    /// Parameters to pass to the function as a JSON value.
    pub parameters: serde_json::Value,

    /// Optional unique identifier for this tool call (for correlation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Set when the provider-sent argument string could not be parsed as
    /// JSON even after repair; call_tools turns this into an is_error
    /// tool result instead of executing the call.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub parse_error: Option<String>,
}

impl TryFrom<AgentValue> for Message {
    type Error = AgentError;

    fn try_from(value: AgentValue) -> Result<Self, Self::Error> {
        match value {
            AgentValue::Message(msg) => Ok((*msg).clone()),
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
                let mut message = Message::new(role, content);

                let id = obj
                    .get("id")
                    .and_then(|i| i.as_str())
                    .map(|s| s.to_string());
                message.id = id;

                message.thinking = obj
                    .get("thinking")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string());

                message.streaming = obj
                    .get("streaming")
                    .and_then(|st| st.as_bool())
                    .unwrap_or_default();

                message.is_error = obj.get("is_error").and_then(|v| v.as_bool());

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
                                parse_error: None,
                            },
                        };
                        calls.push(call);
                    }
                    message.tool_calls = Some(calls.into());
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
        AgentValue::Message(Arc::new(msg))
    }
}

impl From<Vec<Message>> for AgentValue {
    fn from(msgs: Vec<Message>) -> Self {
        let agent_msgs: Vector<AgentValue> = msgs.into_iter().map(|m| m.into()).collect();
        AgentValue::Array(agent_msgs)
    }
}

#[cfg(test)]
mod tests {
    use im::{hashmap, vector};

    use super::*;

    // Message tests

    #[test]
    fn test_tool_call_function_parse_error_serde() {
        // None must not emit the key, so presets saved before this field
        // existed round-trip unchanged.
        let func = ToolCallFunction {
            name: "t".to_string(),
            parameters: serde_json::json!({}),
            id: Some("call1".to_string()),
            parse_error: None,
        };
        let json = serde_json::to_value(&func).unwrap();
        assert!(json.get("parse_error").is_none());
        let restored: ToolCallFunction = serde_json::from_value(json).unwrap();
        assert_eq!(restored.parse_error, None);

        // Some round-trips.
        let func = ToolCallFunction {
            name: "t".to_string(),
            parameters: serde_json::json!({}),
            id: Some("call1".to_string()),
            parse_error: Some("bad json".to_string()),
        };
        let json = serde_json::to_value(&func).unwrap();
        assert_eq!(
            json.get("parse_error").and_then(|v| v.as_str()),
            Some("bad json")
        );
        let restored: ToolCallFunction = serde_json::from_value(json).unwrap();
        assert_eq!(restored.parse_error.as_deref(), Some("bad json"));
    }

    #[test]
    fn test_message_to_from_agent_value() {
        let msg = Message::user("What is the weather today?".to_string());

        let value: AgentValue = msg.into();
        assert!(value.is_message());
        let msg_ref = value.as_message().unwrap();
        assert_eq!(msg_ref.role, "user");
        assert_eq!(msg_ref.content, "What is the weather today?");

        let msg_converted: Message = value.try_into().unwrap();
        assert_eq!(msg_converted.role, "user");
        assert_eq!(msg_converted.content, "What is the weather today?");
    }

    #[test]
    fn test_message_with_tool_calls_to_from_agent_value() {
        let mut msg = Message::assistant("".to_string());
        msg.tool_calls = Some(vector![ToolCall {
            function: ToolCallFunction {
                id: Some("call1".to_string()),
                name: "get_weather".to_string(),
                parameters: serde_json::json!({"location": "San Francisco"}),
                parse_error: None,
            },
        }]);

        let value: AgentValue = msg.into();
        assert!(value.is_message());
        let msg_ref = value.as_message().unwrap();
        assert_eq!(msg_ref.role, "assistant");
        assert_eq!(msg_ref.content, "");
        let tool_calls = msg_ref.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        let first_call = &tool_calls[0];
        assert_eq!(first_call.function.name, "get_weather");
        assert_eq!(first_call.function.parameters["location"], "San Francisco");

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
        let msg_ref = value.as_message().unwrap();
        assert_eq!(msg_ref.role, "tool");
        assert_eq!(msg_ref.tool_name.as_deref().unwrap(), "get_time");
        assert_eq!(msg_ref.content, "2025-01-02 03:04:05");

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
    fn test_message_from_object_value_reads_is_error() {
        let value = AgentValue::object(hashmap! {
            "role".into() => AgentValue::string("tool"),
            "content".into() => AgentValue::string("boom"),
            "tool_name".into() => AgentValue::string("failing_tool"),
            "is_error".into() => AgentValue::boolean(true),
        });
        let msg: Message = value.try_into().unwrap();
        assert_eq!(msg.is_error, Some(true));
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
            tokens: None,
            thinking: None,
            streaming: false,
            tool_calls: Some(vector![ToolCall {
                function: ToolCallFunction {
                    id: Some("call1".to_string()),
                    name: "active_applications".to_string(),
                    parameters: serde_json::json!({}),
                    parse_error: None,
                },
            }]),
            id: None,
            tool_name: None,
            is_error: None,
            #[cfg(feature = "image")]
            image: None,
        };

        let value: AgentValue = message.into();
        let msg_ref = value.as_message().unwrap();

        assert_eq!(msg_ref.role, "assistant");
        assert_eq!(msg_ref.content, "");

        let tool_calls = msg_ref.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);

        assert_eq!(tool_calls[0].function.name, "active_applications");
        assert!(
            tool_calls[0]
                .function
                .parameters
                .as_object()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn test_message_is_error_serde_round_trip() {
        let mut msg = Message::tool("failing_tool".to_string(), "boom".to_string());
        msg.id = Some("call1".to_string());
        msg.is_error = Some(true);

        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["is_error"], serde_json::json!(true));

        let restored: Message = serde_json::from_value(json).unwrap();
        assert_eq!(restored.is_error, Some(true));
        assert_eq!(restored.id.as_deref(), Some("call1"));
        assert_eq!(restored.tool_name.as_deref(), Some("failing_tool"));
    }

    #[test]
    fn test_message_without_is_error_deserializes_to_none() {
        let json = serde_json::json!({
            "role": "tool",
            "content": "ok",
            "tool_name": "some_tool",
        });
        let msg: Message = serde_json::from_value(json).unwrap();
        assert_eq!(msg.is_error, None);
    }

    #[test]
    fn test_message_is_error_none_serializes_without_key() {
        let msg = Message::tool("some_tool".to_string(), "ok".to_string());
        assert_eq!(msg.is_error, None);

        let json = serde_json::to_value(&msg).unwrap();
        assert!(json.as_object().unwrap().get("is_error").is_none());
    }

    #[test]
    fn test_message_partial_eq() {
        let msg1 = Message::user("hello".to_string());
        let msg2 = Message::user("hello".to_string());
        let msg3 = Message::user("world".to_string());

        assert_eq!(msg1, msg2);
        assert_ne!(msg1, msg3);

        let mut msg4 = Message::user("hello".to_string());
        msg4.id = Some("123".to_string());
        assert_ne!(msg1, msg4);
    }
}
