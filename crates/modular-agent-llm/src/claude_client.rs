use std::sync::{Arc, Mutex};

use modular_agent_core::tool;
use modular_agent_core::{
    AgentError, AgentValue, Message, ModularAgent, ToolCall, ToolCallFunction,
};

use crate::chat::ChatAgent;
use crate::provider::{CONFIG_CLAUDE_API_BASE, CONFIG_CLAUDE_API_KEY, DEFAULT_CLAUDE_API_BASE};
const ANTHROPIC_VERSION: &str = "2023-06-01";

// ============================================================================
// Client management
// ============================================================================

#[derive(Clone)]
pub(crate) struct ClaudeClient {
    http: reqwest::Client,
    api_key: String,
    api_base: String,
}

pub(crate) struct ClaudeManager {
    client: Arc<Mutex<Option<ClaudeClient>>>,
}

impl ClaudeManager {
    pub(crate) fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn get_client(&self, ma: &ModularAgent) -> Result<ClaudeClient, AgentError> {
        let mut client_guard = self.client.lock().unwrap();

        if let Some(client) = client_guard.as_ref() {
            return Ok(client.clone());
        }

        // Resolve API key: config → CLAUDE_API_KEY → ANTHROPIC_API_KEY
        let api_key = ma
            .get_global_configs(ChatAgent::DEF_NAME)
            .and_then(|cfg| cfg.get_string(CONFIG_CLAUDE_API_KEY).ok())
            .filter(|key| !key.is_empty())
            .or_else(|| {
                std::env::var("CLAUDE_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
            })
            .or_else(|| {
                std::env::var("ANTHROPIC_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
            })
            .unwrap_or_default();

        // Resolve API base: config → CLAUDE_API_BASE → ANTHROPIC_API_BASE → default
        let api_base = ma
            .get_global_configs(ChatAgent::DEF_NAME)
            .and_then(|cfg| cfg.get_string(CONFIG_CLAUDE_API_BASE).ok())
            .filter(|url| !url.is_empty())
            .or_else(|| {
                std::env::var("CLAUDE_API_BASE")
                    .ok()
                    .filter(|u| !u.is_empty())
            })
            .or_else(|| {
                std::env::var("ANTHROPIC_API_BASE")
                    .ok()
                    .filter(|u| !u.is_empty())
            })
            .unwrap_or_else(|| DEFAULT_CLAUDE_API_BASE.to_string());

        let new_client = ClaudeClient {
            http: reqwest::Client::new(),
            api_key,
            api_base,
        };
        *client_guard = Some(new_client.clone());

        Ok(new_client)
    }
}

impl Default for ClaudeManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// HTTP request methods
// ============================================================================

impl ClaudeClient {
    fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.api_base.trim_end_matches('/'))
    }

    pub(crate) async fn create_message(
        &self,
        request: &ClaudeRequest,
    ) -> Result<ClaudeResponse, AgentError> {
        let resp = self
            .http
            .post(self.messages_url())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(request)
            .send()
            .await
            .map_err(|e| AgentError::IoError(format!("Claude request error: {}", e)))?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }

        let response: ClaudeResponse = resp
            .json()
            .await
            .map_err(|e| AgentError::IoError(format!("Claude response parse error: {}", e)))?;

        Ok(response)
    }

    pub(crate) async fn create_message_stream(
        &self,
        request: &ClaudeRequest,
    ) -> Result<impl futures::Stream<Item = Result<ClaudeStreamEvent, AgentError>>, AgentError>
    {
        use eventsource_stream::Eventsource;
        use futures::StreamExt;

        let resp = self
            .http
            .post(self.messages_url())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(request)
            .send()
            .await
            .map_err(|e| AgentError::IoError(format!("Claude stream request error: {}", e)))?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }

        let stream = resp
            .bytes_stream()
            .eventsource()
            .map(|result| match result {
                Ok(event) => {
                    if event.data == "[DONE]" {
                        return Ok(ClaudeStreamEvent::MessageStop {});
                    }
                    serde_json::from_str::<ClaudeStreamEvent>(&event.data).map_err(|e| {
                        AgentError::IoError(format!("Claude stream parse error: {}", e))
                    })
                }
                Err(e) => Err(AgentError::IoError(format!("Claude stream error: {}", e))),
            });

        Ok(stream)
    }
}

fn map_http_error(status: u16, body: &str) -> AgentError {
    match status {
        401 => AgentError::InvalidConfig(format!("Invalid Claude API key: {}", body)),
        429 => AgentError::IoError(format!("Claude rate limited: {}", body)),
        400 => AgentError::InvalidValue(format!("Claude Bad Request: {}", body)),
        529 => AgentError::IoError(format!("Claude overloaded: {}", body)),
        _ => AgentError::IoError(format!("Claude API Error ({}): {}", status, body)),
    }
}

// ============================================================================
// Serde type definitions
// ============================================================================

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct ClaudeRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<ClaudeMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ClaudeTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ClaudeThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct ClaudeMessage {
    pub role: String,
    pub content: ClaudeContent,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(untagged)]
pub(crate) enum ClaudeContent {
    Text(String),
    Blocks(Vec<ClaudeContentBlock>),
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(tag = "type")]
pub(crate) enum ClaudeContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ClaudeImageSource },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
    #[serde(rename = "thinking")]
    Thinking { thinking: String, signature: String },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking { data: String },
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct ClaudeImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct ClaudeTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub(crate) enum ClaudeThinkingConfig {
    #[serde(rename = "enabled")]
    Enabled { budget_tokens: u32 },
    #[serde(rename = "disabled")]
    Disabled {},
}

// Response types

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct ClaudeResponse {
    pub id: String,
    pub content: Vec<ClaudeResponseBlock>,
    pub stop_reason: Option<String>,
    pub usage: ClaudeUsage,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(tag = "type")]
pub(crate) enum ClaudeResponseBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "thinking")]
    Thinking { thinking: String, signature: String },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking { data: String },
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct ClaudeUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// Streaming types

#[derive(serde::Deserialize)]
#[serde(tag = "type")]
pub(crate) enum ClaudeStreamEvent {
    #[serde(rename = "message_start")]
    MessageStart {
        #[allow(dead_code)]
        message: serde_json::Value,
    },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: ClaudeResponseBlock,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: usize, delta: ClaudeDelta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop {
        #[allow(dead_code)]
        index: usize,
    },
    #[serde(rename = "message_delta")]
    MessageDelta {
        #[allow(dead_code)]
        delta: ClaudeMessageDelta,
        #[allow(dead_code)]
        usage: ClaudeUsage,
    },
    #[serde(rename = "message_stop")]
    MessageStop {},
    #[serde(rename = "ping")]
    Ping {},
    #[serde(rename = "error")]
    Error { error: ClaudeApiError },
}

#[derive(serde::Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::enum_variant_names)]
pub(crate) enum ClaudeDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },
    #[serde(rename = "signature_delta")]
    SignatureDelta {
        #[allow(dead_code)]
        signature: String,
    },
}

#[derive(serde::Deserialize)]
pub(crate) struct ClaudeMessageDelta {
    #[allow(dead_code)]
    pub stop_reason: Option<String>,
}

#[derive(serde::Deserialize)]
pub(crate) struct ClaudeApiError {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub error_type: String,
    pub message: String,
}

// ============================================================================
// Message conversion functions
// ============================================================================

/// Convert internal messages to Claude API format.
///
/// Returns (system_prompt, messages) where system messages are extracted
/// as a separate top-level field (Claude API requirement).
pub(crate) fn messages_to_claude(
    messages: &im::Vector<AgentValue>,
) -> (Option<String>, Vec<ClaudeMessage>) {
    let mut system_parts: Vec<String> = Vec::new();
    let mut claude_messages: Vec<ClaudeMessage> = Vec::new();

    for msg_value in messages.iter() {
        let Some(msg) = msg_value.as_message() else {
            continue;
        };

        match msg.role.as_str() {
            "system" => {
                if !msg.content.is_empty() {
                    system_parts.push(msg.content.clone());
                }
            }
            "user" => {
                let content = build_user_content(msg);
                claude_messages.push(ClaudeMessage {
                    role: "user".to_string(),
                    content,
                });
            }
            "assistant" => {
                let content = build_assistant_content(msg);
                claude_messages.push(ClaudeMessage {
                    role: "assistant".to_string(),
                    content,
                });
            }
            "tool" => {
                let tool_use_id = msg
                    .id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                claude_messages.push(ClaudeMessage {
                    role: "user".to_string(),
                    content: ClaudeContent::Blocks(vec![ClaudeContentBlock::ToolResult {
                        tool_use_id,
                        content: msg.content.clone(),
                    }]),
                });
            }
            _ => {
                // Treat unknown roles as user messages
                claude_messages.push(ClaudeMessage {
                    role: "user".to_string(),
                    content: ClaudeContent::Text(msg.content.clone()),
                });
            }
        }
    }

    let system = if system_parts.is_empty() {
        None
    } else {
        Some(system_parts.join("\n\n"))
    };

    (system, claude_messages)
}

fn build_user_content(msg: &Message) -> ClaudeContent {
    #[cfg(feature = "image")]
    {
        if let Some(image) = &msg.image {
            let base64_str = image.get_base64();
            if let Some((media_type, data)) = parse_base64_image(&base64_str) {
                let mut blocks = vec![ClaudeContentBlock::Image {
                    source: ClaudeImageSource {
                        source_type: "base64".to_string(),
                        media_type,
                        data,
                    },
                }];
                if !msg.content.is_empty() {
                    blocks.push(ClaudeContentBlock::Text {
                        text: msg.content.clone(),
                    });
                }
                return ClaudeContent::Blocks(blocks);
            }
        }
    }
    ClaudeContent::Text(msg.content.clone())
}

fn build_assistant_content(msg: &Message) -> ClaudeContent {
    if let Some(tool_calls) = &msg.tool_calls {
        let mut blocks: Vec<ClaudeContentBlock> = Vec::new();
        if !msg.content.is_empty() {
            blocks.push(ClaudeContentBlock::Text {
                text: msg.content.clone(),
            });
        }
        for call in tool_calls.iter() {
            let id = call
                .function
                .id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            blocks.push(ClaudeContentBlock::ToolUse {
                id,
                name: call.function.name.clone(),
                input: call.function.parameters.clone(),
            });
        }
        ClaudeContent::Blocks(blocks)
    } else {
        ClaudeContent::Text(msg.content.clone())
    }
}

/// Parse a data URI (e.g., `data:image/png;base64,<data>`) into (media_type, data).
pub(crate) fn parse_base64_image(data_uri: &str) -> Option<(String, String)> {
    let stripped = data_uri.strip_prefix("data:")?;
    let (header, data) = stripped.split_once(",")?;
    let media_type = header.strip_suffix(";base64")?.to_string();
    Some((media_type, data.to_string()))
}

/// Convert a Claude API response to an internal Message.
pub(crate) fn message_from_claude_response(response: &ClaudeResponse) -> Message {
    let mut content = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let mut thinking = String::new();

    for block in &response.content {
        match block {
            ClaudeResponseBlock::Text { text } => {
                content.push_str(text);
            }
            ClaudeResponseBlock::ToolUse { id, name, input } => {
                tool_calls.push(ToolCall {
                    function: ToolCallFunction {
                        id: Some(id.clone()),
                        name: name.clone(),
                        parameters: input.clone(),
                    },
                });
            }
            ClaudeResponseBlock::Thinking {
                thinking: thought, ..
            } => {
                if !thinking.is_empty() {
                    thinking.push('\n');
                }
                thinking.push_str(thought);
            }
            ClaudeResponseBlock::RedactedThinking { .. } => {
                if !thinking.is_empty() {
                    thinking.push('\n');
                }
                thinking.push_str("[redacted]");
            }
        }
    }

    let mut message = Message::assistant(content);
    if !tool_calls.is_empty() {
        message.tool_calls = Some(tool_calls.into());
    }
    if !thinking.is_empty() {
        message.thinking = Some(thinking);
    }

    message
}

/// Convert a framework ToolInfo to a Claude Tool definition.
pub(crate) fn tool_info_to_claude_tool(info: tool::ToolInfo) -> ClaudeTool {
    let input_schema = info
        .parameters
        .unwrap_or_else(|| serde_json::json!({"type": "object"}));
    ClaudeTool {
        name: info.name,
        description: if info.description.is_empty() {
            None
        } else {
            Some(info.description)
        },
        input_schema,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use im::vector;

    #[test]
    fn test_messages_to_claude_system_separation() {
        let messages = vector![
            AgentValue::from(Message::system("You are helpful.".to_string())),
            AgentValue::from(Message::user("Hello".to_string())),
        ];

        let (system, msgs) = messages_to_claude(&messages);
        assert_eq!(system, Some("You are helpful.".to_string()));
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, "user");
    }

    #[test]
    fn test_messages_to_claude_multiple_system() {
        let messages = vector![
            AgentValue::from(Message::system("System 1".to_string())),
            AgentValue::from(Message::system("System 2".to_string())),
            AgentValue::from(Message::user("Hello".to_string())),
        ];

        let (system, msgs) = messages_to_claude(&messages);
        assert_eq!(system, Some("System 1\n\nSystem 2".to_string()));
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_messages_to_claude_no_system() {
        let messages = vector![AgentValue::from(Message::user("Hello".to_string())),];

        let (system, msgs) = messages_to_claude(&messages);
        assert!(system.is_none());
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_messages_to_claude_tool_result() {
        let mut tool_msg = Message::tool("my_tool".to_string(), r#"{"result": "ok"}"#.to_string());
        tool_msg.id = Some("toolu_123".to_string());

        let messages = vector![AgentValue::from(tool_msg),];

        let (_, msgs) = messages_to_claude(&messages);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, "user");
        if let ClaudeContent::Blocks(blocks) = &msgs[0].content {
            assert_eq!(blocks.len(), 1);
            if let ClaudeContentBlock::ToolResult {
                tool_use_id,
                content,
            } = &blocks[0]
            {
                assert_eq!(tool_use_id, "toolu_123");
                assert_eq!(content, r#"{"result": "ok"}"#);
            } else {
                panic!("Expected ToolResult block");
            }
        } else {
            panic!("Expected Blocks content");
        }
    }

    #[test]
    fn test_messages_to_claude_tool_result_no_id() {
        let tool_msg = Message::tool("my_tool".to_string(), "result".to_string());

        let messages = vector![AgentValue::from(tool_msg),];

        let (_, msgs) = messages_to_claude(&messages);
        if let ClaudeContent::Blocks(blocks) = &msgs[0].content {
            if let ClaudeContentBlock::ToolResult { tool_use_id, .. } = &blocks[0] {
                // Should have generated a UUID
                assert!(!tool_use_id.is_empty());
            } else {
                panic!("Expected ToolResult block");
            }
        } else {
            panic!("Expected Blocks content");
        }
    }

    #[test]
    fn test_messages_to_claude_assistant_with_tool_calls() {
        let mut assistant_msg = Message::assistant("Let me check.".to_string());
        assistant_msg.tool_calls = Some(
            vec![ToolCall {
                function: ToolCallFunction {
                    id: Some("toolu_abc".to_string()),
                    name: "get_weather".to_string(),
                    parameters: serde_json::json!({"location": "Tokyo"}),
                },
            }]
            .into(),
        );

        let messages = vector![AgentValue::from(assistant_msg),];

        let (_, msgs) = messages_to_claude(&messages);
        assert_eq!(msgs[0].role, "assistant");
        if let ClaudeContent::Blocks(blocks) = &msgs[0].content {
            assert_eq!(blocks.len(), 2);
            assert!(
                matches!(&blocks[0], ClaudeContentBlock::Text { text } if text == "Let me check.")
            );
            assert!(
                matches!(&blocks[1], ClaudeContentBlock::ToolUse { id, name, .. } if id == "toolu_abc" && name == "get_weather")
            );
        } else {
            panic!("Expected Blocks content");
        }
    }

    #[test]
    fn test_message_from_claude_response_text() {
        let response = ClaudeResponse {
            id: "msg_123".to_string(),
            content: vec![ClaudeResponseBlock::Text {
                text: "Hello!".to_string(),
            }],
            stop_reason: Some("end_turn".to_string()),
            usage: ClaudeUsage {
                input_tokens: 10,
                output_tokens: 5,
            },
        };

        let msg = message_from_claude_response(&response);
        assert_eq!(msg.content, "Hello!");
        assert!(msg.tool_calls.is_none());
        assert!(msg.thinking.is_none());
    }

    #[test]
    fn test_message_from_claude_response_tool_use() {
        let response = ClaudeResponse {
            id: "msg_123".to_string(),
            content: vec![
                ClaudeResponseBlock::Text {
                    text: "I'll check the weather.".to_string(),
                },
                ClaudeResponseBlock::ToolUse {
                    id: "toolu_abc".to_string(),
                    name: "get_weather".to_string(),
                    input: serde_json::json!({"location": "Tokyo"}),
                },
            ],
            stop_reason: Some("tool_use".to_string()),
            usage: ClaudeUsage {
                input_tokens: 20,
                output_tokens: 15,
            },
        };

        let msg = message_from_claude_response(&response);
        assert_eq!(msg.content, "I'll check the weather.");
        let tool_calls = msg.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].function.name, "get_weather");
        assert_eq!(tool_calls[0].function.id, Some("toolu_abc".to_string()));
    }

    #[test]
    fn test_message_from_claude_response_thinking() {
        let response = ClaudeResponse {
            id: "msg_123".to_string(),
            content: vec![
                ClaudeResponseBlock::Thinking {
                    thinking: "Let me think...".to_string(),
                    signature: "sig123".to_string(),
                },
                ClaudeResponseBlock::Text {
                    text: "The answer is 42.".to_string(),
                },
            ],
            stop_reason: Some("end_turn".to_string()),
            usage: ClaudeUsage {
                input_tokens: 30,
                output_tokens: 20,
            },
        };

        let msg = message_from_claude_response(&response);
        assert_eq!(msg.content, "The answer is 42.");
        assert_eq!(msg.thinking, Some("Let me think...".to_string()));
    }

    #[test]
    fn test_message_from_claude_response_redacted_thinking() {
        let response = ClaudeResponse {
            id: "msg_123".to_string(),
            content: vec![
                ClaudeResponseBlock::RedactedThinking {
                    data: "encrypted_data".to_string(),
                },
                ClaudeResponseBlock::Text {
                    text: "Result.".to_string(),
                },
            ],
            stop_reason: Some("end_turn".to_string()),
            usage: ClaudeUsage {
                input_tokens: 10,
                output_tokens: 5,
            },
        };

        let msg = message_from_claude_response(&response);
        assert_eq!(msg.content, "Result.");
        assert_eq!(msg.thinking, Some("[redacted]".to_string()));
    }

    #[test]
    fn test_tool_info_to_claude_tool() {
        let info = tool::ToolInfo {
            name: "get_weather".to_string(),
            description: "Get current weather".to_string(),
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "location": { "type": "string" }
                },
                "required": ["location"]
            })),
        };

        let tool = tool_info_to_claude_tool(info);
        assert_eq!(tool.name, "get_weather");
        assert_eq!(tool.description, Some("Get current weather".to_string()));
        assert_eq!(tool.input_schema["type"], serde_json::json!("object"));
    }

    #[test]
    fn test_tool_info_to_claude_tool_no_params() {
        let info = tool::ToolInfo {
            name: "list_items".to_string(),
            description: "".to_string(),
            parameters: None,
        };

        let tool = tool_info_to_claude_tool(info);
        assert_eq!(tool.name, "list_items");
        assert!(tool.description.is_none());
        assert_eq!(tool.input_schema, serde_json::json!({"type": "object"}));
    }

    #[test]
    fn test_parse_base64_image_png() {
        let uri = "data:image/png;base64,iVBORw0KGgo=";
        let (media_type, data) = parse_base64_image(uri).unwrap();
        assert_eq!(media_type, "image/png");
        assert_eq!(data, "iVBORw0KGgo=");
    }

    #[test]
    fn test_parse_base64_image_jpeg() {
        let uri = "data:image/jpeg;base64,/9j/4AAQ";
        let (media_type, data) = parse_base64_image(uri).unwrap();
        assert_eq!(media_type, "image/jpeg");
        assert_eq!(data, "/9j/4AAQ");
    }

    #[test]
    fn test_parse_base64_image_gif() {
        let uri = "data:image/gif;base64,R0lGODlh";
        let (media_type, data) = parse_base64_image(uri).unwrap();
        assert_eq!(media_type, "image/gif");
        assert_eq!(data, "R0lGODlh");
    }

    #[test]
    fn test_parse_base64_image_webp() {
        let uri = "data:image/webp;base64,UklGR";
        let (media_type, data) = parse_base64_image(uri).unwrap();
        assert_eq!(media_type, "image/webp");
        assert_eq!(data, "UklGR");
    }

    #[test]
    fn test_parse_base64_image_invalid() {
        assert!(parse_base64_image("not-a-data-uri").is_none());
        assert!(parse_base64_image("data:image/png,nobase64").is_none());
    }

    #[test]
    fn test_serde_roundtrip_request() {
        let request = ClaudeRequest {
            model: "claude-sonnet-4-5-20250514".to_string(),
            max_tokens: 1024,
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: ClaudeContent::Text("Hello".to_string()),
            }],
            system: Some("Be helpful.".to_string()),
            stream: None,
            tools: None,
            thinking: None,
            temperature: None,
            top_p: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        let parsed: ClaudeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.model, "claude-sonnet-4-5-20250514");
        assert_eq!(parsed.max_tokens, 1024);
        assert_eq!(parsed.system, Some("Be helpful.".to_string()));
    }

    #[test]
    fn test_serde_request_skips_none() {
        let request = ClaudeRequest {
            model: "claude-sonnet-4-5-20250514".to_string(),
            max_tokens: 1024,
            messages: vec![],
            system: None,
            stream: None,
            tools: None,
            thinking: None,
            temperature: None,
            top_p: None,
        };

        let json = serde_json::to_value(&request).unwrap();
        assert!(!json.as_object().unwrap().contains_key("system"));
        assert!(!json.as_object().unwrap().contains_key("stream"));
        assert!(!json.as_object().unwrap().contains_key("tools"));
        assert!(!json.as_object().unwrap().contains_key("thinking"));
        assert!(!json.as_object().unwrap().contains_key("temperature"));
        assert!(!json.as_object().unwrap().contains_key("top_p"));
    }

    #[test]
    fn test_serde_response_parse() {
        let json = r#"{
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Hello!"},
                {"type": "tool_use", "id": "toolu_1", "name": "calc", "input": {"x": 1}}
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 10, "output_tokens": 20}
        }"#;

        let response: ClaudeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "msg_123");
        assert_eq!(response.content.len(), 2);
        assert!(
            matches!(&response.content[0], ClaudeResponseBlock::Text { text } if text == "Hello!")
        );
        assert!(
            matches!(&response.content[1], ClaudeResponseBlock::ToolUse { name, .. } if name == "calc")
        );
    }

    #[test]
    fn test_serde_stream_event_text_delta() {
        let json = r#"{"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}"#;
        let event: ClaudeStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(
            event,
            ClaudeStreamEvent::ContentBlockDelta {
                index: 0,
                delta: ClaudeDelta::TextDelta { .. }
            }
        ));
    }

    #[test]
    fn test_serde_stream_event_thinking_delta() {
        let json = r#"{"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "hmm..."}}"#;
        let event: ClaudeStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(
            event,
            ClaudeStreamEvent::ContentBlockDelta {
                delta: ClaudeDelta::ThinkingDelta { .. },
                ..
            }
        ));
    }

    #[test]
    fn test_serde_stream_event_input_json_delta() {
        let json = r#"{"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "{\"loc"}}"#;
        let event: ClaudeStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(
            event,
            ClaudeStreamEvent::ContentBlockDelta {
                index: 1,
                delta: ClaudeDelta::InputJsonDelta { .. }
            }
        ));
    }

    #[test]
    fn test_serde_stream_event_content_block_start() {
        let json = r#"{"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}"#;
        let event: ClaudeStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(
            event,
            ClaudeStreamEvent::ContentBlockStart { index: 0, .. }
        ));
    }

    #[test]
    fn test_serde_stream_event_error() {
        let json = r#"{"type": "error", "error": {"type": "overloaded_error", "message": "Server overloaded"}}"#;
        let event: ClaudeStreamEvent = serde_json::from_str(json).unwrap();
        if let ClaudeStreamEvent::Error { error } = event {
            assert_eq!(error.message, "Server overloaded");
        } else {
            panic!("Expected Error event");
        }
    }

    #[test]
    fn test_map_http_error() {
        assert!(matches!(
            map_http_error(401, "Unauthorized"),
            AgentError::InvalidConfig(_)
        ));
        assert!(matches!(
            map_http_error(429, "Rate limited"),
            AgentError::IoError(_)
        ));
        assert!(matches!(
            map_http_error(400, "Bad request"),
            AgentError::InvalidValue(_)
        ));
        assert!(matches!(
            map_http_error(529, "Overloaded"),
            AgentError::IoError(_)
        ));
        assert!(matches!(
            map_http_error(500, "Server error"),
            AgentError::IoError(_)
        ));
    }
}
