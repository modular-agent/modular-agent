#![cfg(feature = "openai")]

use std::sync::{Arc, Mutex};

use modular_agent_core::tool;
use modular_agent_core::{
    AgentError, AgentValue, AgentValueMap, Message, ModularAgent, ToolCall, ToolCallFunction,
};

use crate::chat::ChatAgent;

const CONFIG_OPENAI_API_KEY: &str = "openai_api_key";
const CONFIG_OPENAI_API_BASE: &str = "openai_api_base";
const DEFAULT_OPENAI_API_BASE: &str = "https://api.openai.com/v1";

// ============================================================================
// Client management
// ============================================================================

#[derive(Clone)]
pub(crate) struct OpenAIClient {
    http: reqwest::Client,
    api_key: String,
    api_base: String,
}

pub struct OpenAIManager {
    client: Arc<Mutex<Option<OpenAIClient>>>,
}

impl OpenAIManager {
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_client(&self, ma: &ModularAgent) -> Result<OpenAIClient, AgentError> {
        let mut client_guard = self.client.lock().unwrap();

        if let Some(client) = client_guard.as_ref() {
            return Ok(client.clone());
        }

        // API key: config → OPENAI_API_KEY env var → empty
        let api_key = ma
            .get_global_configs(ChatAgent::DEF_NAME)
            .and_then(|cfg| cfg.get_string(CONFIG_OPENAI_API_KEY).ok())
            .filter(|key| !key.is_empty())
            .or_else(|| {
                std::env::var("OPENAI_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
            })
            .unwrap_or_default();

        // API base: config → OPENAI_API_BASE env var → default
        let api_base = ma
            .get_global_configs(ChatAgent::DEF_NAME)
            .and_then(|cfg| cfg.get_string(CONFIG_OPENAI_API_BASE).ok())
            .filter(|url| !url.is_empty())
            .or_else(|| {
                std::env::var("OPENAI_API_BASE")
                    .ok()
                    .filter(|u| !u.is_empty())
            })
            .unwrap_or_else(|| DEFAULT_OPENAI_API_BASE.to_string());

        let new_client = OpenAIClient {
            http: reqwest::Client::new(),
            api_key,
            api_base,
        };
        *client_guard = Some(new_client.clone());

        Ok(new_client)
    }
}

impl Default for OpenAIManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// HTTP request methods
// ============================================================================

impl OpenAIClient {
    pub(crate) fn chat_completions_url(&self) -> String {
        format!("{}/chat/completions", self.api_base.trim_end_matches('/'))
    }

    pub(crate) fn completions_url(&self) -> String {
        format!("{}/completions", self.api_base.trim_end_matches('/'))
    }

    pub(crate) fn embeddings_url(&self) -> String {
        format!("{}/embeddings", self.api_base.trim_end_matches('/'))
    }

    pub(crate) fn responses_url(&self) -> String {
        format!("{}/responses", self.api_base.trim_end_matches('/'))
    }

    /// POST JSON and parse typed response.
    pub(crate) async fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<T, AgentError> {
        let resp = self
            .http
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| AgentError::IoError(format!("OpenAI request error: {}", e)))?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }

        resp.json()
            .await
            .map_err(|e| AgentError::IoError(format!("OpenAI response parse error: {}", e)))
    }

    /// POST and return an SSE stream of raw JSON data strings.
    ///
    /// `[DONE]` sentinel is filtered out. Callers deserialize each string
    /// into the appropriate type (e.g. `ChatStreamChunk` or `ResponseStreamEvent`).
    pub(crate) async fn post_stream(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<impl futures::Stream<Item = Result<Option<String>, AgentError>> + use<>, AgentError>
    {
        use eventsource_stream::Eventsource;
        use futures::StreamExt;

        let resp = self
            .http
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| AgentError::IoError(format!("OpenAI stream request error: {}", e)))?;

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
                        Ok(None)
                    } else {
                        Ok(Some(event.data))
                    }
                }
                Err(e) => Err(AgentError::IoError(format!("OpenAI stream error: {}", e))),
            });

        Ok(stream)
    }
}

fn map_http_error(status: u16, body: &str) -> AgentError {
    match status {
        401 => AgentError::InvalidConfig(format!("Invalid OpenAI API key: {}", body)),
        429 => AgentError::IoError(format!("OpenAI rate limited: {}", body)),
        400 => AgentError::InvalidValue(format!("OpenAI Bad Request: {}", body)),
        _ => AgentError::IoError(format!("OpenAI API Error ({}): {}", status, body)),
    }
}

// ============================================================================
// Serde type definitions — Chat Completions
// ============================================================================

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct ChatCompletionResponse {
    pub choices: Vec<ChatChoice>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct ChatChoice {
    pub index: u32,
    pub message: ChatResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct ChatResponseMessage {
    pub role: String,
    pub content: Option<String>,
    pub refusal: Option<String>,
    pub tool_calls: Option<Vec<ChatToolCall>>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct ChatToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ChatFunctionCall,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct ChatFunctionCall {
    pub name: String,
    pub arguments: String,
}

// Streaming types

#[derive(serde::Deserialize, Clone)]
pub(crate) struct ChatStreamChunk {
    pub choices: Vec<ChatStreamChoice>,
    #[serde(flatten)]
    #[allow(dead_code)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(serde::Deserialize, Clone)]
pub(crate) struct ChatStreamChoice {
    #[allow(dead_code)]
    pub index: u32,
    pub delta: ChatStreamDelta,
    #[allow(dead_code)]
    pub finish_reason: Option<String>,
}

#[derive(serde::Deserialize, Clone)]
pub(crate) struct ChatStreamDelta {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ChatToolCallChunk>>,
    pub refusal: Option<String>,
}

#[derive(serde::Deserialize, Clone)]
pub(crate) struct ChatToolCallChunk {
    #[allow(dead_code)]
    pub index: u32,
    pub id: Option<String>,
    pub function: Option<ChatFunctionCallChunk>,
}

#[derive(serde::Deserialize, Clone)]
pub(crate) struct ChatFunctionCallChunk {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

// ============================================================================
// Serde type definitions — Completions
// ============================================================================

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct CompletionResponse {
    pub choices: Vec<CompletionChoice>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct CompletionChoice {
    pub text: String,
    pub index: u32,
    pub finish_reason: Option<String>,
}

// ============================================================================
// Serde type definitions — Embeddings
// ============================================================================

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct EmbeddingResponse {
    pub data: Vec<EmbeddingData>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct EmbeddingData {
    pub index: u32,
    pub embedding: Vec<f32>,
}

// ============================================================================
// Serde type definitions — Responses API streaming
// ============================================================================

#[derive(serde::Deserialize)]
#[serde(tag = "type")]
pub(crate) enum ResponseStreamEvent {
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta { delta: String },
    #[serde(rename = "response.function_call_arguments.delta")]
    FunctionCallArgumentsDelta { delta: String },
    #[serde(rename = "response.output_item.added")]
    OutputItemAdded { item: serde_json::Value },
    #[serde(rename = "response.output_item.done")]
    OutputItemDone {
        #[allow(dead_code)]
        item: serde_json::Value,
    },
    #[serde(rename = "response.completed")]
    Completed { response: serde_json::Value },
    #[serde(other)]
    Other,
}

// ============================================================================
// Embeddings helper
// ============================================================================

pub async fn generate_embeddings(
    client: &OpenAIClient,
    texts: Vec<String>,
    model_name: &str,
    config_options: &AgentValueMap<String, AgentValue>,
) -> Result<Vec<Vec<f32>>, AgentError> {
    let mut request = serde_json::json!({
        "model": model_name,
        "input": texts,
    });

    merge_options(&mut request, config_options)?;

    let res: EmbeddingResponse = client.post_json(&client.embeddings_url(), &request).await?;

    Ok(res.data.into_iter().map(|d| d.embedding).collect())
}

// ============================================================================
// Message conversion functions — Chat Completions
// ============================================================================

/// Convert internal Message to Chat Completions API request JSON.
pub fn message_to_chat_json(msg: &Message) -> serde_json::Value {
    match msg.role.as_str() {
        "system" => serde_json::json!({
            "role": "system",
            "content": msg.content
        }),
        "user" => {
            #[cfg(feature = "image")]
            {
                if let Some(image) = &msg.image {
                    return serde_json::json!({
                        "role": "user",
                        "content": [
                            { "type": "text", "text": msg.content },
                            {
                                "type": "image_url",
                                "image_url": {
                                    "url": image.get_base64(),
                                    "detail": "auto"
                                }
                            }
                        ]
                    });
                }
            }
            serde_json::json!({
                "role": "user",
                "content": msg.content
            })
        }
        "assistant" => {
            let mut json = serde_json::json!({
                "role": "assistant",
                "content": msg.content
            });
            if let Some(tool_calls) = &msg.tool_calls {
                let tc: Vec<serde_json::Value> = tool_calls
                    .iter()
                    .map(|tc| {
                        serde_json::json!({
                            "type": "function",
                            "id": tc.function.id.clone()
                                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                            "function": {
                                "name": tc.function.name,
                                "arguments": tc.function.parameters.to_string()
                            }
                        })
                    })
                    .collect();
                json["tool_calls"] = serde_json::Value::Array(tc);
            }
            json
        }
        "tool" => serde_json::json!({
            "role": "tool",
            "content": msg.content,
            "tool_call_id": msg.id.clone().unwrap_or_default()
        }),
        _ => serde_json::json!({
            "role": "user",
            "content": msg.content
        }),
    }
}

/// Convert Chat Completions API response message to internal Message.
pub fn message_from_chat_response(msg: &ChatResponseMessage) -> Message {
    let content = msg.content.clone().unwrap_or_default();
    let mut message = Message::new(msg.role.clone(), content);

    let thinking = msg
        .refusal
        .as_ref()
        .map(|r| format!("Refusal: {}", r))
        .unwrap_or_default();
    if !thinking.is_empty() {
        message.thinking = Some(thinking);
    }

    if let Some(tool_calls) = &msg.tool_calls {
        let calls: Vec<ToolCall> = tool_calls
            .iter()
            .map(|call| {
                let parameters = serde_json::from_str(&call.function.arguments).unwrap_or_default();
                ToolCall {
                    function: ToolCallFunction {
                        id: Some(call.id.clone()),
                        name: call.function.name.clone(),
                        parameters,
                    },
                }
            })
            .collect();
        if !calls.is_empty() {
            message.tool_calls = Some(calls.into());
        }
    }

    message
}

/// Convert a ToolInfo to Chat Completions tool definition JSON.
pub fn tool_info_to_chat_tool_json(info: tool::ToolInfo) -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": info.name,
            "description": if info.description.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(info.description)
            },
            "parameters": info.parameters.unwrap_or(serde_json::json!({}))
        }
    })
}

/// Convert a streaming tool call chunk to internal ToolCall.
pub fn tool_call_from_stream_chunk(call: &ChatToolCallChunk) -> Result<ToolCall, AgentError> {
    let function = call
        .function
        .as_ref()
        .ok_or_else(|| AgentError::InvalidValue("ToolCallChunk missing function".to_string()))?;
    let name = function.name.as_ref().ok_or_else(|| {
        AgentError::InvalidValue("ToolCallChunk function missing name".to_string())
    })?;
    let parameters = if let Some(arguments) = &function.arguments {
        serde_json::from_str(arguments).map_err(|e| {
            AgentError::InvalidValue(format!("Failed to parse tool call arguments JSON: {}", e))
        })?
    } else {
        serde_json::json!({})
    };

    let function = ToolCallFunction {
        id: call.id.clone(),
        name: name.clone(),
        parameters,
    };
    Ok(ToolCall { function })
}

// ============================================================================
// Message conversion functions — Responses API
// ============================================================================

/// Convert messages to Responses API input format.
///
/// Maps internal Message types to the correct Responses API InputItem variants:
/// - Assistant messages with tool_calls → FunctionCall items
/// - Tool result messages → FunctionCallOutput items
/// - Other messages → Message items (via serde_json)
pub fn messages_to_response_input(
    messages: &im::Vector<AgentValue>,
) -> Result<Vec<serde_json::Value>, AgentError> {
    let mut input_items = Vec::new();

    for msg_value in messages.iter() {
        let Some(msg) = msg_value.as_message() else {
            continue;
        };

        match msg.role.as_str() {
            "tool" => {
                let call_id = msg
                    .id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                input_items.push(serde_json::json!({
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": msg.content,
                }));
            }
            "assistant" => {
                if let Some(tool_calls) = &msg.tool_calls {
                    if !msg.content.is_empty() {
                        build_response_message_item(&mut input_items, "assistant", msg)?;
                    }
                    for tc in tool_calls.iter() {
                        let call_id = tc
                            .function
                            .id
                            .clone()
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                        input_items.push(serde_json::json!({
                            "type": "function_call",
                            "arguments": tc.function.parameters.to_string(),
                            "call_id": call_id,
                            "name": tc.function.name,
                        }));
                    }
                } else {
                    build_response_message_item(&mut input_items, "assistant", msg)?;
                }
            }
            role => {
                let role_str = match role {
                    "system" | "developer" => "developer",
                    _ => "user",
                };
                build_response_message_item(&mut input_items, role_str, msg)?;
            }
        }
    }

    Ok(input_items)
}

/// Build a Responses API message input item.
fn build_response_message_item(
    input_items: &mut Vec<serde_json::Value>,
    role_str: &str,
    msg: &Message,
) -> Result<(), AgentError> {
    #[cfg(feature = "image")]
    let item = if let Some(image) = &msg.image {
        serde_json::json!({
            "type": "message",
            "role": role_str,
            "content": [
                { "type": "input_text", "text": msg.content },
                { "type": "input_image", "detail": "auto", "image_url": image.get_base64() }
            ]
        })
    } else {
        serde_json::json!({
            "type": "message",
            "role": role_str,
            "content": msg.content
        })
    };

    #[cfg(not(feature = "image"))]
    let item = serde_json::json!({
        "type": "message",
        "role": role_str,
        "content": msg.content
    });

    input_items.push(item);
    Ok(())
}

/// Convert Responses API output items to internal Message.
pub fn response_output_to_message(output: &[serde_json::Value]) -> Result<Message, AgentError> {
    let mut content = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    for item in output {
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match item_type {
            "message" => {
                if let Some(parts) = item.get("content").and_then(|c| c.as_array()) {
                    for part in parts {
                        let part_type = part.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        match part_type {
                            "output_text" => {
                                if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                                    content.push_str(text);
                                }
                            }
                            "refusal" => {
                                if let Some(refusal) = part.get("refusal").and_then(|v| v.as_str())
                                {
                                    content.push_str(&format!("[Refusal: {}]", refusal));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            "function_call" => {
                let name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let call_id = item
                    .get("call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let arguments = item
                    .get("arguments")
                    .and_then(|v| v.as_str())
                    .unwrap_or("{}");
                let parameters: serde_json::Value =
                    serde_json::from_str(arguments).unwrap_or_default();
                tool_calls.push(ToolCall {
                    function: ToolCallFunction {
                        id: Some(call_id),
                        name,
                        parameters,
                    },
                });
            }
            _ => {}
        }
    }

    let mut message = Message::assistant(content);
    if !tool_calls.is_empty() {
        message.tool_calls = Some(tool_calls.into());
    }

    Ok(message)
}

// ============================================================================
// Helpers
// ============================================================================

/// Merge user options JSON into a request JSON object.
pub(crate) fn merge_options(
    request: &mut serde_json::Value,
    config_options: &AgentValueMap<String, AgentValue>,
) -> Result<(), AgentError> {
    if config_options.is_empty() {
        return Ok(());
    }
    let options_json = serde_json::to_value(config_options)
        .map_err(|e| AgentError::InvalidValue(format!("Invalid JSON in options: {}", e)))?;
    if let (Some(req_obj), Some(opt_obj)) = (request.as_object_mut(), options_json.as_object()) {
        for (key, value) in opt_obj {
            req_obj.insert(key.clone(), value.clone());
        }
    }
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use im::vector;

    fn make_tool_call(id: &str, name: &str, params: serde_json::Value) -> ToolCall {
        ToolCall {
            function: ToolCallFunction {
                id: Some(id.to_string()),
                name: name.to_string(),
                parameters: params,
            },
        }
    }

    // =========================================================================
    // Chat Completions: message_to_chat_json
    // =========================================================================

    #[test]
    fn test_chat_completion_assistant_without_tool_calls() {
        let msg = Message::assistant("Hello".to_string());
        let json = message_to_chat_json(&msg);
        assert_eq!(json["role"], "assistant");
        assert_eq!(json["content"], "Hello");
        assert!(json.get("tool_calls").is_none());
    }

    #[test]
    fn test_chat_completion_assistant_with_tool_calls() {
        let mut msg = Message::assistant("".to_string());
        msg.tool_calls = Some(vector![make_tool_call(
            "call_123",
            "get_weather",
            serde_json::json!({"city": "Tokyo"})
        )]);

        let json = message_to_chat_json(&msg);
        assert_eq!(json["role"], "assistant");

        let tool_calls = json["tool_calls"].as_array().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0]["type"], "function");
        assert_eq!(tool_calls[0]["id"], "call_123");
        assert_eq!(tool_calls[0]["function"]["name"], "get_weather");

        let args: serde_json::Value =
            serde_json::from_str(tool_calls[0]["function"]["arguments"].as_str().unwrap()).unwrap();
        assert_eq!(args["city"], "Tokyo");
    }

    #[test]
    fn test_chat_completion_tool_result() {
        let mut msg = Message::tool("get_weather".to_string(), "22°C".to_string());
        msg.id = Some("call_123".to_string());

        let json = message_to_chat_json(&msg);
        assert_eq!(json["role"], "tool");
        assert_eq!(json["content"], "22°C");
        assert_eq!(json["tool_call_id"], "call_123");
    }

    // =========================================================================
    // Chat Completions: message_from_chat_response
    // =========================================================================

    #[test]
    fn test_message_from_chat_response_text() {
        let msg = ChatResponseMessage {
            role: "assistant".to_string(),
            content: Some("Hello!".to_string()),
            refusal: None,
            tool_calls: None,
        };
        let result = message_from_chat_response(&msg);
        assert_eq!(result.content, "Hello!");
        assert!(result.tool_calls.is_none());
        assert!(result.thinking.is_none());
    }

    #[test]
    fn test_message_from_chat_response_tool_use() {
        let msg = ChatResponseMessage {
            role: "assistant".to_string(),
            content: Some("I'll check.".to_string()),
            refusal: None,
            tool_calls: Some(vec![ChatToolCall {
                id: "call_abc".to_string(),
                call_type: "function".to_string(),
                function: ChatFunctionCall {
                    name: "get_weather".to_string(),
                    arguments: r#"{"location":"Tokyo"}"#.to_string(),
                },
            }]),
        };
        let result = message_from_chat_response(&msg);
        assert_eq!(result.content, "I'll check.");
        let tool_calls = result.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].function.name, "get_weather");
        assert_eq!(tool_calls[0].function.id, Some("call_abc".to_string()));
    }

    #[test]
    fn test_message_from_chat_response_refusal() {
        let msg = ChatResponseMessage {
            role: "assistant".to_string(),
            content: Some("".to_string()),
            refusal: Some("I cannot do that.".to_string()),
            tool_calls: None,
        };
        let result = message_from_chat_response(&msg);
        assert_eq!(
            result.thinking,
            Some("Refusal: I cannot do that.".to_string())
        );
    }

    // =========================================================================
    // Serde: response types
    // =========================================================================

    #[test]
    fn test_serde_chat_completion_response() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "model": "gpt-5-mini",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello!",
                        "refusal": null,
                        "tool_calls": null
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        }"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].message.content, Some("Hello!".to_string()));
        assert_eq!(resp.extra.get("id").unwrap(), "chatcmpl-123");
        assert!(resp.extra.contains_key("usage"));
    }

    #[test]
    fn test_serde_chat_stream_chunk() {
        let json = r#"{
            "id": "chatcmpl-123",
            "choices": [
                {
                    "index": 0,
                    "delta": {"content": "Hello"},
                    "finish_reason": null
                }
            ]
        }"#;
        let chunk: ChatStreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.choices.len(), 1);
        assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_serde_chat_stream_chunk_tool_call() {
        let json = r#"{
            "id": "chatcmpl-123",
            "choices": [
                {
                    "index": 0,
                    "delta": {
                        "tool_calls": [
                            {
                                "index": 0,
                                "id": "call_abc",
                                "function": {"name": "get_weather", "arguments": "{\"city\":\"Tokyo\"}"}
                            }
                        ]
                    },
                    "finish_reason": null
                }
            ]
        }"#;
        let chunk: ChatStreamChunk = serde_json::from_str(json).unwrap();
        let tc = &chunk.choices[0].delta.tool_calls.as_ref().unwrap()[0];
        assert_eq!(tc.id, Some("call_abc".to_string()));
        let func = tc.function.as_ref().unwrap();
        assert_eq!(func.name, Some("get_weather".to_string()));
    }

    #[test]
    fn test_serde_completion_response() {
        let json = r#"{
            "id": "cmpl-123",
            "object": "text_completion",
            "model": "gpt-3.5-turbo-instruct",
            "choices": [{"text": "Hello world", "index": 0, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 5, "completion_tokens": 3, "total_tokens": 8}
        }"#;
        let resp: CompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices[0].text, "Hello world");
        assert!(resp.extra.contains_key("usage"));
    }

    #[test]
    fn test_serde_embedding_response() {
        let json = r#"{
            "object": "list",
            "data": [{"index": 0, "embedding": [0.1, 0.2, 0.3]}],
            "model": "text-embedding-3-small",
            "usage": {"prompt_tokens": 5, "total_tokens": 5}
        }"#;
        let resp: EmbeddingResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].embedding, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_serde_response_stream_event_text_delta() {
        let json = r#"{"type": "response.output_text.delta", "delta": "Hello"}"#;
        let event: ResponseStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(
            event,
            ResponseStreamEvent::OutputTextDelta { delta } if delta == "Hello"
        ));
    }

    #[test]
    fn test_serde_response_stream_event_function_call_args() {
        let json = r#"{"type": "response.function_call_arguments.delta", "delta": "{\"city\":"}"#;
        let event: ResponseStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(
            event,
            ResponseStreamEvent::FunctionCallArgumentsDelta { .. }
        ));
    }

    #[test]
    fn test_serde_response_stream_event_completed() {
        let json =
            r#"{"type": "response.completed", "response": {"id": "resp_123", "output": []}}"#;
        let event: ResponseStreamEvent = serde_json::from_str(json).unwrap();
        if let ResponseStreamEvent::Completed { response } = event {
            assert_eq!(response["id"], "resp_123");
        } else {
            panic!("Expected Completed event");
        }
    }

    #[test]
    fn test_serde_response_stream_event_other() {
        let json = r#"{"type": "response.created", "response": {}}"#;
        let event: ResponseStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, ResponseStreamEvent::Other));
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
            map_http_error(500, "Server error"),
            AgentError::IoError(_)
        ));
    }

    // =========================================================================
    // Responses API: messages_to_response_input
    // =========================================================================

    #[test]
    fn test_response_input_user_message() {
        let messages = vector![AgentValue::from(Message::user("Hello".to_string()))];
        let items = messages_to_response_input(&messages).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["role"], "user");
        assert_eq!(items[0]["content"], "Hello");
    }

    #[test]
    fn test_response_input_assistant_without_tool_calls() {
        let messages = vector![AgentValue::from(Message::assistant("Hi there".to_string()))];
        let items = messages_to_response_input(&messages).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["role"], "assistant");
        assert_eq!(items[0]["content"], "Hi there");
    }

    #[test]
    fn test_response_input_assistant_with_tool_calls() {
        let mut msg = Message::assistant("I'll check.".to_string());
        msg.tool_calls = Some(vector![make_tool_call(
            "call_456",
            "get_weather",
            serde_json::json!({"city": "NY"})
        )]);
        let messages = vector![AgentValue::from(msg)];
        let items = messages_to_response_input(&messages).unwrap();

        // Should have: 1 message item (text) + 1 function_call item
        assert_eq!(items.len(), 2);

        assert_eq!(items[0]["role"], "assistant");
        assert_eq!(items[0]["content"], "I'll check.");

        assert_eq!(items[1]["type"], "function_call");
        assert_eq!(items[1]["call_id"], "call_456");
        assert_eq!(items[1]["name"], "get_weather");
    }

    #[test]
    fn test_response_input_assistant_with_tool_calls_no_content() {
        let mut msg = Message::assistant("".to_string());
        msg.tool_calls = Some(vector![make_tool_call(
            "call_789",
            "search",
            serde_json::json!({"q": "test"})
        )]);
        let messages = vector![AgentValue::from(msg)];
        let items = messages_to_response_input(&messages).unwrap();

        // No text content → only function_call item, no message item
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "function_call");
        assert_eq!(items[0]["name"], "search");
    }

    #[test]
    fn test_response_input_tool_result() {
        let mut msg = Message::tool("get_weather".to_string(), "22°C".to_string());
        msg.id = Some("call_456".to_string());
        let messages = vector![AgentValue::from(msg)];
        let items = messages_to_response_input(&messages).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "function_call_output");
        assert_eq!(items[0]["call_id"], "call_456");
        assert_eq!(items[0]["output"], "22°C");
    }

    #[test]
    fn test_response_input_tool_result_no_id() {
        let msg = Message::tool("my_tool".to_string(), "result".to_string());
        let messages = vector![AgentValue::from(msg)];
        let items = messages_to_response_input(&messages).unwrap();

        assert_eq!(items[0]["type"], "function_call_output");
        // Should have a generated UUID, not empty
        let call_id = items[0]["call_id"].as_str().unwrap();
        assert!(!call_id.is_empty());
    }

    #[test]
    fn test_response_input_full_round_trip() {
        // Simulate: user → assistant(tool_call) → tool_result
        let mut assistant_msg = Message::assistant("".to_string());
        assistant_msg.tool_calls = Some(vector![make_tool_call(
            "call_abc",
            "get_horoscope",
            serde_json::json!({"sign": "Virgo"})
        )]);

        let mut tool_msg =
            Message::tool("get_horoscope".to_string(), "Virgo: Good day!".to_string());
        tool_msg.id = Some("call_abc".to_string());

        let messages = vector![
            AgentValue::from(Message::user("What's my horoscope?".to_string())),
            AgentValue::from(assistant_msg),
            AgentValue::from(tool_msg),
        ];

        let items = messages_to_response_input(&messages).unwrap();

        // user message + function_call + function_call_output = 3 items
        assert_eq!(items.len(), 3);

        assert_eq!(items[0]["role"], "user");

        assert_eq!(items[1]["type"], "function_call");
        assert_eq!(items[1]["name"], "get_horoscope");

        assert_eq!(items[2]["type"], "function_call_output");
        assert_eq!(items[2]["call_id"], "call_abc");
        assert_eq!(items[2]["output"], "Virgo: Good day!");
    }

    #[test]
    fn test_response_input_system_message() {
        let messages = vector![AgentValue::from(Message::system(
            "You are helpful.".to_string()
        ))];
        let items = messages_to_response_input(&messages).unwrap();

        assert_eq!(items[0]["role"], "developer");
    }

    // =========================================================================
    // Responses API: response_output_to_message
    // =========================================================================

    #[test]
    fn test_response_output_text() {
        let output = vec![serde_json::json!({
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "output_text", "text": "Hello!"}
            ]
        })];
        let msg = response_output_to_message(&output).unwrap();
        assert_eq!(msg.content, "Hello!");
        assert!(msg.tool_calls.is_none());
    }

    #[test]
    fn test_response_output_function_call() {
        let output = vec![
            serde_json::json!({
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "I'll check."}]
            }),
            serde_json::json!({
                "type": "function_call",
                "name": "get_weather",
                "arguments": "{\"location\":\"Tokyo\"}",
                "call_id": "call_123"
            }),
        ];
        let msg = response_output_to_message(&output).unwrap();
        assert_eq!(msg.content, "I'll check.");
        let tool_calls = msg.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].function.name, "get_weather");
        assert_eq!(tool_calls[0].function.id, Some("call_123".to_string()));
    }

    #[test]
    fn test_response_output_refusal() {
        let output = vec![serde_json::json!({
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "refusal", "refusal": "I cannot do that."}
            ]
        })];
        let msg = response_output_to_message(&output).unwrap();
        assert_eq!(msg.content, "[Refusal: I cannot do that.]");
    }
}
