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

/// One block of structured [`Message`] content.
///
/// Serialized as an internally tagged object (`{"type": "text", ...}`) so
/// block arrays in preset JSON stay self-describing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text content.
    Text {
        /// The text.
        text: String,
    },

    /// Reasoning/thinking trace from an extended thinking model.
    Thinking {
        /// The thinking text. For redacted blocks this holds the provider's
        /// opaque encrypted payload verbatim.
        thinking: String,

        /// Provider signature that must be replayed together with the
        /// thinking text when re-sending assistant history (Claude requires
        /// it for extended thinking + tool use continuations).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        signature: Option<String>,

        /// True when the provider returned an encrypted `redacted_thinking`
        /// payload instead of readable text.
        #[serde(skip_serializing_if = "std::ops::Not::not", default)]
        redacted: bool,
    },

    /// Inline image content (requires "image" feature).
    #[cfg(feature = "image")]
    Image {
        /// Base64-encoded image data.
        data: String,

        /// MIME type of the image, e.g. "image/png".
        mime_type: String,
    },
}

/// Content of a [`Message`]: plain text or a sequence of [`ContentBlock`]s.
///
/// Plain text serializes as a JSON string — the pre-block format — so
/// text-only histories written by this version can still be read by older
/// versions. Block content serializes as a tagged array and is only
/// produced when a message actually carries thinking or image blocks.
#[derive(Debug, Clone, PartialEq)]
pub enum MessageContent {
    /// Plain text content (the common case).
    Text(String),

    /// Structured content preserving provider block order.
    Blocks(Vec<ContentBlock>),
}

impl Default for MessageContent {
    fn default() -> Self {
        MessageContent::Text(String::new())
    }
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        MessageContent::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        MessageContent::Text(s.to_string())
    }
}

impl MessageContent {
    /// Concatenated text of all text content.
    pub fn text(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect(),
        }
    }
}

/// Absorbs the legacy top-level `thinking` string field into a leading
/// Thinking block. Providers emit thinking before the answer text, so the
/// absorbed block leads.
fn absorb_legacy_thinking(content: MessageContent, thinking: String) -> MessageContent {
    let mut blocks = match content {
        MessageContent::Text(s) if s.is_empty() => vec![],
        MessageContent::Text(s) => vec![ContentBlock::Text { text: s }],
        MessageContent::Blocks(blocks) => blocks,
    };
    blocks.insert(
        0,
        ContentBlock::Thinking {
            thinking,
            signature: None,
            redacted: false,
        },
    );
    MessageContent::Blocks(blocks)
}

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
/// * `content` - The content of the message: plain text or structured blocks
/// * `tokens` - Optional token count for the message
/// * `streaming` - Whether this is a partial streaming response
/// * `tool_calls` - Tool invocations requested by the assistant
/// * `tool_name` - Name of the tool (for tool role messages)
/// * `is_error` - Marks a tool-result message as an error
/// * `stop_reason` - Normalized reason the LLM stopped generating
/// * `usage` - Token usage reported by the provider (final messages only)
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

    /// Content of the message: plain text or structured blocks. Use
    /// [`Message::text`] for the concatenated text and [`Message::thinking`]
    /// for the thinking trace.
    pub content: MessageContent,

    /// Token count for this message (if available).
    pub tokens: Option<usize>,

    /// Whether this is a partial streaming response.
    pub streaming: bool,

    /// Tool calls requested by the assistant in this message.
    pub tool_calls: Option<Vector<ToolCall>>,

    /// Name of the tool (for tool role messages containing tool results).
    pub tool_name: Option<String>,

    /// Marks a tool-result message as an error, per Claude's `tool_result` `is_error`.
    pub is_error: Option<bool>,

    /// Normalized reason the LLM stopped generating this message:
    /// "stop" | "tool_use" | "length" | "error" | "aborted". Unknown
    /// provider values are passed through unchanged.
    pub stop_reason: Option<String>,

    /// Token usage reported by the provider. Only set on final assistant
    /// messages (`streaming == false`); partial streaming emissions never
    /// carry usage.
    pub usage: Option<Usage>,

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
            content: MessageContent::Text(content),
            tokens: None,
            streaming: false,
            tool_calls: None,
            tool_name: None,
            is_error: None,
            stop_reason: None,
            usage: None,

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

    /// Concatenated text of all text content — the common read path.
    pub fn text(&self) -> String {
        self.content.text()
    }

    /// Concatenated thinking text, or `None` when the message has no
    /// thinking blocks. Replaces the former `thinking` field: redacted
    /// blocks surface as a `"[redacted]"` placeholder (their `thinking`
    /// holds an opaque encrypted payload meant only for provider replay)
    /// and multiple blocks are joined with a newline, preserving the old
    /// field's observable form.
    pub fn thinking(&self) -> Option<String> {
        let MessageContent::Blocks(blocks) = &self.content else {
            return None;
        };
        let parts: Vec<&str> = blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Thinking { redacted: true, .. } => Some("[redacted]"),
                ContentBlock::Thinking { thinking, .. } => Some(thinking.as_str()),
                _ => None,
            })
            .collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n"))
        }
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
        // Text-only content keeps the legacy string form so histories that
        // never used thinking or image blocks stay readable by older
        // versions. Only block content that cannot be flattened to plain
        // text is written as an array.
        let content_value = match &self.content {
            MessageContent::Text(s) => serde_json::Value::String(s.clone()),
            MessageContent::Blocks(blocks)
                if blocks
                    .iter()
                    .all(|b| matches!(b, ContentBlock::Text { .. })) =>
            {
                serde_json::Value::String(self.content.text())
            }
            MessageContent::Blocks(blocks) => {
                serde_json::to_value(blocks).map_err(serde::ser::Error::custom)?
            }
        };
        map.insert("content".to_string(), content_value);
        if let Some(tokens) = &self.tokens {
            map.insert(
                "tokens".to_string(),
                serde_json::Value::Number((*tokens).into()),
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
        if let Some(stop_reason) = &self.stop_reason {
            map.insert(
                "stop_reason".to_string(),
                serde_json::Value::String(stop_reason.clone()),
            );
        }
        if let Some(usage) = &self.usage {
            map.insert(
                "usage".to_string(),
                serde_json::to_value(usage).map_err(serde::ser::Error::custom)?,
            );
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
            message.content = match content {
                serde_json::Value::String(s) => MessageContent::Text(s.clone()),
                serde_json::Value::Array(_) => {
                    let blocks: Vec<ContentBlock> = serde_json::from_value(content.clone())
                        .map_err(|e| {
                            serde::de::Error::custom(format!("invalid content blocks: {e}"))
                        })?;
                    MessageContent::Blocks(blocks)
                }
                _ => {
                    return Err(serde::de::Error::custom(
                        "content must be a string or an array of content blocks",
                    ));
                }
            };
        }
        if let Some(tokens) = map.get("tokens") {
            message.tokens = tokens.as_u64().map(|u| u as usize);
        }
        // Legacy top-level "thinking" field (pre content-block format) is
        // absorbed as a leading Thinking block.
        if let Some(thinking) = map.get("thinking").and_then(|v| v.as_str()) {
            message.content =
                absorb_legacy_thinking(std::mem::take(&mut message.content), thinking.to_string());
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
        message.stop_reason = map
            .get("stop_reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        // Lenient: an unparseable usage value becomes None rather than
        // failing the whole message.
        message.usage = map
            .get("usage")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
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

/// Token usage reported by an LLM provider for one assistant message.
///
/// `input_tokens` EXCLUDES cache_read/cache_write tokens (Anthropic-style
/// accounting; OpenAI prompt_tokens are normalized by subtracting cached).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Non-cached input tokens billed for the request.
    #[serde(default)]
    pub input_tokens: u64,

    /// Output tokens generated by the model.
    #[serde(default)]
    pub output_tokens: u64,

    /// Input tokens read from the provider's prompt cache.
    #[serde(default)]
    pub cache_read_tokens: u64,

    /// Input tokens written to the provider's prompt cache.
    #[serde(default)]
    pub cache_write_tokens: u64,
}

/// A tool call requested by the assistant.
///
/// Represents a single tool invocation as part of an LLM response.
/// The assistant may request multiple tool calls in a single message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// The function to be called.
    pub function: ToolCallFunction,
}

/// Details of a function call within a tool invocation.
///
/// Contains the function name, parameters, and optional call ID
/// for correlating tool calls with their results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// A typed streaming event describing the progress of one assistant [`Message`].
///
/// Providers emit these events while generating a response so downstream
/// agents can distinguish incremental deltas from the final message instead
/// of relying on repeated partial-`Message` re-sends. Each incremental event
/// carries both the `delta` (just the new fragment) and the accumulated
/// `partial` message, so consumers can either append deltas or replace the
/// whole message — a single accumulation loop suffices.
///
/// Serialized as an internally tagged JSON object: the `type` field holds the
/// snake_case variant name (e.g. `"text_delta"`, `"tool_call_end"`, `"done"`)
/// and the variant fields are inlined alongside it.
///
/// # Variants
///
/// * `Start` - Generation began; `partial` is the (usually empty) initial message
/// * `TextDelta` - New text content was appended to `partial`'s text content
/// * `ThinkingDelta` - New thinking text was appended to `partial`'s thinking block
/// * `ToolCallStart` - The assistant began emitting the tool call at `index`
/// * `ToolCallDelta` - New argument text for the tool call at `index`
/// * `ToolCallEnd` - The tool call at `index` is complete and parsed
/// * `Done` - Generation finished; `message` is the final complete message
/// * `Error` - Generation failed; `message` holds what was accumulated so far
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageEvent {
    /// Generation of a new assistant message has started.
    Start {
        /// The initial (typically empty) accumulated message.
        partial: Message,
    },

    /// A fragment of text content was generated.
    TextDelta {
        /// The newly generated text fragment.
        delta: String,
        /// The accumulated message including this delta.
        partial: Message,
    },

    /// A fragment of thinking/reasoning text was generated.
    ThinkingDelta {
        /// The newly generated thinking fragment.
        delta: String,
        /// The accumulated message including this delta.
        partial: Message,
    },

    /// The assistant started emitting a tool call.
    ToolCallStart {
        /// Zero-based position of the tool call within the message.
        index: usize,
        /// The accumulated message so far.
        partial: Message,
    },

    /// A fragment of tool-call arguments was generated.
    ToolCallDelta {
        /// Zero-based position of the tool call within the message.
        index: usize,
        /// The newly generated argument text fragment.
        delta: String,
        /// The accumulated message so far.
        partial: Message,
    },

    /// A tool call is complete and its arguments have been parsed.
    ToolCallEnd {
        /// Zero-based position of the tool call within the message.
        index: usize,
        /// The completed tool call.
        tool_call: ToolCall,
        /// The accumulated message including this tool call.
        partial: Message,
    },

    /// Generation finished successfully.
    Done {
        /// The final complete message.
        message: Message,
    },

    /// Generation failed.
    Error {
        /// The message accumulated before the failure.
        message: Message,
        /// Description of the failure.
        error: String,
    },
}

impl TryFrom<MessageEvent> for AgentValue {
    type Error = AgentError;

    fn try_from(event: MessageEvent) -> Result<Self, AgentError> {
        // Route through serde_json so the tagged representation on a port
        // matches the serialized form exactly (including the "type" field).
        let json = serde_json::to_value(&event).map_err(|e| {
            AgentError::InvalidValue(format!("Failed to serialize MessageEvent: {e}"))
        })?;
        AgentValue::from_json(json)
    }
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
                let content_value = obj.get("content").ok_or_else(|| {
                    AgentError::InvalidValue("Message object missing 'content' field".to_string())
                })?;
                let content = match content_value {
                    AgentValue::String(s) => MessageContent::Text(s.to_string()),
                    AgentValue::Array(_) => {
                        let blocks: Vec<ContentBlock> =
                            serde_json::from_value(content_value.to_json()).map_err(|e| {
                                AgentError::InvalidValue(format!("Invalid content blocks: {e}"))
                            })?;
                        MessageContent::Blocks(blocks)
                    }
                    _ => {
                        return Err(AgentError::InvalidValue(
                            "'content' field must be a string or an array of content blocks"
                                .to_string(),
                        ));
                    }
                };
                let mut message = Message::new(role, String::new());
                message.content = content;

                let id = obj
                    .get("id")
                    .and_then(|i| i.as_str())
                    .map(|s| s.to_string());
                message.id = id;

                // Legacy top-level "thinking" field is absorbed as a leading
                // Thinking block, mirroring the serde path.
                if let Some(thinking) = obj.get("thinking").and_then(|t| t.as_str()) {
                    message.content = absorb_legacy_thinking(
                        std::mem::take(&mut message.content),
                        thinking.to_string(),
                    );
                }

                message.streaming = obj
                    .get("streaming")
                    .and_then(|st| st.as_bool())
                    .unwrap_or_default();

                message.is_error = obj.get("is_error").and_then(|v| v.as_bool());

                message.stop_reason = obj
                    .get("stop_reason")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Lenient: an unparseable usage value becomes None rather
                // than failing the whole conversion.
                message.usage = obj
                    .get("usage")
                    .and_then(|v| serde_json::from_value(v.to_json()).ok());

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
        assert_eq!(msg_ref.text(), "What is the weather today?");

        let msg_converted: Message = value.try_into().unwrap();
        assert_eq!(msg_converted.role, "user");
        assert_eq!(msg_converted.text(), "What is the weather today?");
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
        assert_eq!(msg_ref.text(), "");
        let tool_calls = msg_ref.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        let first_call = &tool_calls[0];
        assert_eq!(first_call.function.name, "get_weather");
        assert_eq!(first_call.function.parameters["location"], "San Francisco");

        let msg_converted: Message = value.try_into().unwrap();
        dbg!(&msg_converted);
        assert_eq!(msg_converted.role, "assistant");
        assert_eq!(msg_converted.text(), "");
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
        assert_eq!(msg_ref.text(), "2025-01-02 03:04:05");

        let msg_converted: Message = value.try_into().unwrap();
        assert_eq!(msg_converted.role, "tool");
        assert_eq!(msg_converted.tool_name.as_deref(), Some("get_time"));
        assert_eq!(msg_converted.text(), "2025-01-02 03:04:05");
    }

    #[test]
    fn test_message_from_string_value() {
        let value = AgentValue::string("Just a simple message");
        let msg: Message = value.try_into().unwrap();
        assert_eq!(msg.role, "user");
        assert_eq!(msg.text(), "Just a simple message");
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
        assert_eq!(msg.text(), "Here is some information.");
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
            content: MessageContent::default(),
            tokens: None,
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
            stop_reason: None,
            usage: None,
            #[cfg(feature = "image")]
            image: None,
        };

        let value: AgentValue = message.into();
        let msg_ref = value.as_message().unwrap();

        assert_eq!(msg_ref.role, "assistant");
        assert_eq!(msg_ref.text(), "");

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
    fn test_message_stop_reason_serde_round_trip() {
        let mut msg = Message::assistant("partial answer".to_string());
        msg.stop_reason = Some("length".to_string());

        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["stop_reason"], serde_json::json!("length"));

        let restored: Message = serde_json::from_value(json).unwrap();
        assert_eq!(restored.stop_reason.as_deref(), Some("length"));
    }

    #[test]
    fn test_message_without_stop_reason_deserializes_to_none() {
        // Presets saved before this field existed must load unchanged.
        let json = serde_json::json!({
            "role": "assistant",
            "content": "ok",
        });
        let msg: Message = serde_json::from_value(json).unwrap();
        assert_eq!(msg.stop_reason, None);
    }

    #[test]
    fn test_message_stop_reason_none_serializes_without_key() {
        let msg = Message::assistant("ok".to_string());
        assert_eq!(msg.stop_reason, None);

        let json = serde_json::to_value(&msg).unwrap();
        assert!(json.as_object().unwrap().get("stop_reason").is_none());
    }

    #[test]
    fn test_message_from_object_value_reads_stop_reason() {
        let value = AgentValue::object(hashmap! {
            "role".into() => AgentValue::string("assistant"),
            "content".into() => AgentValue::string("truncated"),
            "stop_reason".into() => AgentValue::string("length"),
        });
        let msg: Message = value.try_into().unwrap();
        assert_eq!(msg.stop_reason.as_deref(), Some("length"));
    }

    #[test]
    fn test_message_usage_serde_round_trip() {
        let mut msg = Message::assistant("ok".to_string());
        msg.usage = Some(Usage {
            input_tokens: 100,
            output_tokens: 20,
            cache_read_tokens: 50,
            cache_write_tokens: 10,
        });

        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(
            json["usage"],
            serde_json::json!({
                "input_tokens": 100,
                "output_tokens": 20,
                "cache_read_tokens": 50,
                "cache_write_tokens": 10,
            })
        );

        let restored: Message = serde_json::from_value(json).unwrap();
        assert_eq!(restored.usage, msg.usage);
    }

    #[test]
    fn test_message_without_usage_deserializes_to_none() {
        // Presets saved before this field existed must load unchanged.
        let json = serde_json::json!({
            "role": "assistant",
            "content": "ok",
        });
        let msg: Message = serde_json::from_value(json).unwrap();
        assert_eq!(msg.usage, None);
    }

    #[test]
    fn test_message_usage_none_serializes_without_key() {
        let msg = Message::assistant("ok".to_string());
        assert_eq!(msg.usage, None);

        let json = serde_json::to_value(&msg).unwrap();
        assert!(json.as_object().unwrap().get("usage").is_none());
    }

    #[test]
    fn test_message_from_object_value_reads_usage() {
        let value = AgentValue::object(hashmap! {
            "role".into() => AgentValue::string("assistant"),
            "content".into() => AgentValue::string("ok"),
            "usage".into() => AgentValue::object(hashmap! {
                "input_tokens".into() => AgentValue::integer(7),
                "output_tokens".into() => AgentValue::integer(3),
            }),
        });
        let msg: Message = value.try_into().unwrap();
        assert_eq!(
            msg.usage,
            Some(Usage {
                input_tokens: 7,
                output_tokens: 3,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            })
        );
    }

    #[test]
    fn test_message_partial_usage_object_deserializes_with_defaults() {
        let json = serde_json::json!({
            "role": "assistant",
            "content": "ok",
            "usage": { "input_tokens": 42 },
        });
        let msg: Message = serde_json::from_value(json).unwrap();
        assert_eq!(
            msg.usage,
            Some(Usage {
                input_tokens: 42,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            })
        );
    }

    #[test]
    fn test_message_unparseable_usage_deserializes_to_none() {
        let json = serde_json::json!({
            "role": "assistant",
            "content": "ok",
            "usage": "not an object",
        });
        let msg: Message = serde_json::from_value(json).unwrap();
        assert_eq!(msg.usage, None);
    }

    // MessageEvent tests

    #[test]
    fn test_message_event_text_delta_serde_round_trip() {
        let mut partial = Message::assistant("Hel".to_string());
        partial.streaming = true;
        let event = MessageEvent::TextDelta {
            delta: "l".to_string(),
            partial,
        };

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], serde_json::json!("text_delta"));
        assert_eq!(json["delta"], serde_json::json!("l"));
        assert_eq!(json["partial"]["content"], serde_json::json!("Hel"));

        let restored: MessageEvent = serde_json::from_value(json).unwrap();
        assert_eq!(restored, event);
        // Message's PartialEq covers only id/role/content, so fields the
        // handwritten serde must preserve are asserted directly.
        let MessageEvent::TextDelta { delta, partial } = restored else {
            panic!("wrong variant");
        };
        assert_eq!(delta, "l");
        assert!(partial.streaming);
    }

    #[test]
    fn test_message_event_done_serde_round_trip() {
        let mut msg = Message::assistant("Hello".to_string());
        msg.id = Some("msg1".to_string());
        msg.stop_reason = Some("stop".to_string());
        msg.usage = Some(Usage {
            input_tokens: 10,
            output_tokens: 5,
            cache_read_tokens: 2,
            cache_write_tokens: 1,
        });
        let event = MessageEvent::Done {
            message: msg.clone(),
        };

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], serde_json::json!("done"));
        assert_eq!(json["message"]["role"], serde_json::json!("assistant"));
        assert_eq!(json["message"]["content"], serde_json::json!("Hello"));

        let restored: MessageEvent = serde_json::from_value(json).unwrap();
        assert_eq!(restored, event);
        // Message's PartialEq covers only id/role/content, so fields the
        // handwritten serde must preserve are asserted directly.
        let MessageEvent::Done { message } = restored else {
            panic!("wrong variant");
        };
        assert!(!message.streaming);
        assert_eq!(message.stop_reason, msg.stop_reason);
        assert_eq!(message.usage, msg.usage);
    }

    #[test]
    fn test_message_event_tool_call_end_serde_round_trip() {
        let tool_call = ToolCall {
            function: ToolCallFunction {
                id: Some("call1".to_string()),
                name: "get_weather".to_string(),
                parameters: serde_json::json!({"location": "Tokyo"}),
                parse_error: None,
            },
        };
        let mut partial = Message::assistant("".to_string());
        partial.streaming = true;
        partial.tool_calls = Some(vector![tool_call.clone()]);
        let event = MessageEvent::ToolCallEnd {
            index: 0,
            tool_call,
            partial,
        };

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], serde_json::json!("tool_call_end"));
        assert_eq!(json["index"], serde_json::json!(0));
        assert_eq!(
            json["tool_call"]["function"]["name"],
            serde_json::json!("get_weather")
        );

        let restored: MessageEvent = serde_json::from_value(json).unwrap();
        assert_eq!(restored, event);
        // Message's PartialEq covers only id/role/content, so fields the
        // handwritten serde must preserve are asserted directly.
        let MessageEvent::ToolCallEnd {
            index,
            tool_call,
            partial,
        } = restored
        else {
            panic!("wrong variant");
        };
        assert_eq!(index, 0);
        assert_eq!(tool_call.function.name, "get_weather");
        assert_eq!(
            tool_call.function.parameters,
            serde_json::json!({"location": "Tokyo"})
        );
        assert!(partial.streaming);
        let restored_calls = partial.tool_calls.unwrap();
        assert_eq!(restored_calls.len(), 1);
        assert_eq!(restored_calls[0].function.id, Some("call1".to_string()));
    }

    #[test]
    fn test_message_event_to_agent_value() {
        let event = MessageEvent::Done {
            message: Message::assistant("Hello".to_string()),
        };

        let value = AgentValue::try_from(event).unwrap();
        assert!(value.is_object());
        assert_eq!(value.get_str("type"), Some("done"));
        let message = value.get("message").unwrap();
        assert_eq!(message.get_str("role"), Some("assistant"));
        assert_eq!(message.get_str("content"), Some("Hello"));
    }

    #[test]
    fn test_message_event_error_to_agent_value() {
        let event = MessageEvent::Error {
            message: Message::assistant("partial".to_string()),
            error: "connection reset".to_string(),
        };

        let value = AgentValue::try_from(event).unwrap();
        assert_eq!(value.get_str("type"), Some("error"));
        assert_eq!(value.get_str("error"), Some("connection reset"));
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

    // Content block tests

    #[test]
    fn test_message_legacy_thinking_field_absorbed_on_deserialize() {
        let json = serde_json::json!({
            "role": "assistant",
            "content": "hi",
            "thinking": "t",
        });
        let msg: Message = serde_json::from_value(json).unwrap();
        assert_eq!(
            msg.content,
            MessageContent::Blocks(vec![
                ContentBlock::Thinking {
                    thinking: "t".to_string(),
                    signature: None,
                    redacted: false,
                },
                ContentBlock::Text {
                    text: "hi".to_string()
                },
            ])
        );
        assert_eq!(msg.text(), "hi");
        assert_eq!(msg.thinking().as_deref(), Some("t"));
    }

    #[test]
    fn test_message_pure_text_serializes_as_plain_string() {
        let msg = Message::assistant("hello".to_string());
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["content"], serde_json::json!("hello"));

        // Text-only block content is also flattened to the legacy string form.
        let mut msg = Message::assistant(String::new());
        msg.content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "hel".to_string(),
            },
            ContentBlock::Text {
                text: "lo".to_string(),
            },
        ]);
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["content"], serde_json::json!("hello"));
    }

    #[test]
    fn test_message_thinking_blocks_serde_round_trip() {
        let mut msg = Message::assistant(String::new());
        msg.content = MessageContent::Blocks(vec![
            ContentBlock::Thinking {
                thinking: "reasoning".to_string(),
                signature: Some("sig123".to_string()),
                redacted: false,
            },
            ContentBlock::Thinking {
                thinking: "opaque-payload".to_string(),
                signature: None,
                redacted: true,
            },
            ContentBlock::Text {
                text: "answer".to_string(),
            },
        ]);

        let json = serde_json::to_value(&msg).unwrap();
        assert!(json["content"].is_array());
        // The legacy top-level "thinking" key is no longer written.
        assert!(json.get("thinking").is_none());

        let restored: Message = serde_json::from_value(json).unwrap();
        assert_eq!(restored.content, msg.content);
    }

    #[test]
    fn test_message_thinking_redacts_and_joins_with_newline() {
        // The former `thinking` field surfaced "[redacted]" for redacted
        // blocks and joined multiple traces with a newline; the accessor
        // must not leak the encrypted payload stored in redacted blocks.
        let mut msg = Message::assistant(String::new());
        msg.content = MessageContent::Blocks(vec![
            ContentBlock::Thinking {
                thinking: "Let me think...".to_string(),
                signature: Some("sig".to_string()),
                redacted: false,
            },
            ContentBlock::Thinking {
                thinking: "EqQBCgIYAg-ciphertext".to_string(),
                signature: None,
                redacted: true,
            },
            ContentBlock::Text {
                text: "answer".to_string(),
            },
        ]);
        assert_eq!(
            msg.thinking().as_deref(),
            Some("Let me think...\n[redacted]")
        );
    }

    #[test]
    fn test_message_mixed_block_order_preserved() {
        let blocks = vec![
            ContentBlock::Text {
                text: "before".to_string(),
            },
            ContentBlock::Thinking {
                thinking: "mid".to_string(),
                signature: Some("s".to_string()),
                redacted: false,
            },
            ContentBlock::Text {
                text: "after".to_string(),
            },
        ];
        let mut msg = Message::assistant(String::new());
        msg.content = MessageContent::Blocks(blocks.clone());

        let json = serde_json::to_value(&msg).unwrap();
        let restored: Message = serde_json::from_value(json).unwrap();
        assert_eq!(restored.content, MessageContent::Blocks(blocks));
        assert_eq!(restored.text(), "beforeafter");
        assert_eq!(restored.thinking().as_deref(), Some("mid"));
    }
}
