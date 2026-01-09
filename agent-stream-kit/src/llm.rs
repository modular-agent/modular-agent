use std::{ops::Not, sync::Arc, vec};

use im::Vector;
use serde::{Deserialize, Serialize};

use crate::error::AgentError;
use crate::value::AgentValue;

#[cfg(feature = "image")]
use photon_rs::PhotonImage;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    pub role: String,

    pub content: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,

    #[serde(skip_serializing_if = "<&bool>::not")]
    pub streaming: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vector<ToolCall>>,

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
            streaming: false,
            thinking: None,
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

impl PartialEq for Message {
    fn eq(&self, other: &Self) -> bool {
        let is_eq = self.id == other.id
            && self.role == other.role
            && self.content == other.content
            && self.thinking == other.thinking
            && self.streaming == other.streaming
            && self.tool_calls == other.tool_calls
            && self.tool_name == other.tool_name;

        #[cfg(feature = "image")]
        {
            if !is_eq {
                return false;
            }
            match (&self.image, &other.image) {
                (Some(img1), Some(img2)) => {
                    img1.get_width() == img2.get_width()
                        && img1.get_height() == img2.get_height()
                        && img1.get_raw_pixels() == img2.get_raw_pixels()
                }
                (None, None) => true,
                _ => false,
            }
        }
        #[cfg(not(feature = "image"))]
        is_eq
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[cfg(test)]
mod tests {
    use im::{hashmap, vector};

    use super::*;

    // Message tests

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
            thinking: None,
            streaming: false,
            tool_calls: Some(vector![ToolCall {
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
