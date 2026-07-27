use std::sync::{Arc, Mutex};

use modular_agent_core::tool;
use modular_agent_core::{
    AgentError, AgentValue, AgentValueMap, ContentBlock, Message, MessageContent, ModularAgent,
    ToolCall, ToolCallFunction, Usage,
};

use crate::chat::ChatAgent;
use crate::provider::{CONFIG_OPENAI_API_BASE, CONFIG_OPENAI_API_KEY, DEFAULT_OPENAI_API_BASE};

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

        // Timeouts keep a hung server from blocking process() forever;
        // read_timeout is idle time between reads, so streaming is safe.
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .read_timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| AgentError::IoError(format!("OpenAI client build error: {}", e)))?;
        let new_client = OpenAIClient {
            http,
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
            .map_err(|e| crate::http_error::map_reqwest_error("OpenAI request error", e))?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let retry_after = crate::http_error::parse_retry_after(resp.headers());
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body, retry_after));
        }

        resp.json()
            .await
            .map_err(|e| crate::http_error::map_reqwest_error("OpenAI response parse error", e))
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
            .map_err(|e| crate::http_error::map_reqwest_error("OpenAI stream request error", e))?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let retry_after = crate::http_error::parse_retry_after(resp.headers());
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body, retry_after));
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

fn map_http_error(status: u16, body: &str, retry_after: Option<std::time::Duration>) -> AgentError {
    // 429 takes precedence over overflow detection so throttling responses
    // whose body happens to mention context size stay retryable.
    if status == 429 {
        let lower = body.to_lowercase();
        if crate::http_error::mentions_quota_exhausted(&lower) {
            return AgentError::InvalidConfig(format!("OpenAI quota exhausted: {}", body));
        }
        return AgentError::RateLimited {
            message: format!("OpenAI rate limited: {}", body),
            retry_after,
        };
    }
    if is_context_overflow(body) {
        return AgentError::ContextOverflow(format!("OpenAI context overflow: {}", body));
    }
    match status {
        401 => AgentError::InvalidConfig(format!("Invalid OpenAI API key: {}", body)),
        400 => AgentError::InvalidValue(format!("OpenAI Bad Request: {}", body)),
        500..=599 => AgentError::Overloaded(format!("OpenAI API Error ({}): {}", status, body)),
        _ => AgentError::IoError(format!("OpenAI API Error ({}): {}", status, body)),
    }
}

fn is_context_overflow(body: &str) -> bool {
    let lower = body.to_lowercase();
    if crate::http_error::mentions_rate_limit(&lower) {
        return false;
    }
    lower.contains("exceeds the context window") || lower.contains("maximum context length")
}

// ============================================================================
// Serde type definitions — Chat Completions
// ============================================================================

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct ChatCompletionResponse {
    pub choices: Vec<ChatChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAIUsage>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// All fields are defaulted/Option so OpenAI-compatible servers that report
// less (or no) usage still parse. The flatten maps preserve unmodeled keys
// (total_tokens, completion_tokens_details, ...) across the raw `response`
// port re-serialization.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct OpenAIUsage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<PromptTokensDetails>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct PromptTokensDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u64>,
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

// With `stream_options.include_usage`, the last chunk carries `usage` and an
// EMPTY `choices` array; consumers must not index into choices.
#[derive(serde::Deserialize, Clone)]
pub(crate) struct ChatStreamChunk {
    pub choices: Vec<ChatStreamChoice>,
    #[serde(default)]
    pub usage: Option<OpenAIUsage>,
    #[serde(flatten)]
    #[allow(dead_code)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(serde::Deserialize, Clone)]
pub(crate) struct ChatStreamChoice {
    #[allow(dead_code)]
    pub index: u32,
    pub delta: ChatStreamDelta,
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
    #[serde(rename = "response.incomplete")]
    Incomplete { response: serde_json::Value },
    #[serde(rename = "response.failed")]
    Failed { response: serde_json::Value },
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
            "content": msg.text()
        }),
        "user" => {
            #[cfg(feature = "image")]
            {
                if let Some(image) = &msg.image {
                    return serde_json::json!({
                        "role": "user",
                        "content": [
                            { "type": "text", "text": msg.text() },
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
            if let Some(parts) = chat_content_parts(msg) {
                return serde_json::json!({
                    "role": "user",
                    "content": parts
                });
            }
            serde_json::json!({
                "role": "user",
                "content": msg.text()
            })
        }
        "assistant" => {
            let mut json = serde_json::json!({
                "role": "assistant",
                "content": msg.text()
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
        // The Chat Completions tool role only accepts a string, so block
        // results degrade to text with image placeholders.
        "tool" => serde_json::json!({
            "role": "tool",
            "content": crate::content::tool_result_fallback_text(&msg.content),
            "tool_call_id": msg.id.clone().unwrap_or_default()
        }),
        _ => serde_json::json!({
            "role": "user",
            "content": msg.text()
        }),
    }
}

/// Chat Completions content parts for a message carrying image blocks, in
/// block order. `None` when the content has no image blocks, so plain text
/// keeps the legacy string form. Without this, image blocks accepted by
/// Message deserialization would be silently dropped from the request.
fn chat_content_parts(msg: &Message) -> Option<Vec<serde_json::Value>> {
    let MessageContent::Blocks(blocks) = &msg.content else {
        return None;
    };
    if !blocks
        .iter()
        .any(|b| matches!(b, ContentBlock::Image { .. }))
    {
        return None;
    }
    Some(
        blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } if !text.is_empty() => {
                    Some(serde_json::json!({ "type": "text", "text": text }))
                }
                ContentBlock::Image { data, mime_type } => Some(serde_json::json!({
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{mime_type};base64,{data}"),
                        "detail": "auto"
                    }
                })),
                _ => None,
            })
            .collect(),
    )
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
        message.content = crate::content::content_with_thinking(&thinking, &message.text());
    }

    if let Some(tool_calls) = &msg.tool_calls {
        let calls: Vec<ToolCall> = tool_calls
            .iter()
            .map(|call| {
                let (parameters, parse_error) =
                    crate::json_repair::parse_tool_arguments(&call.function.arguments);
                ToolCall {
                    function: ToolCallFunction {
                        id: Some(call.id.clone()),
                        name: call.function.name.clone(),
                        parameters,
                        parse_error,
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

/// Normalize OpenAI usage to the framework `Usage`. OpenAI's `prompt_tokens`
/// INCLUDES cached tokens, so cached is subtracted out to match the
/// Anthropic-style accounting used by `Usage.input_tokens`.
pub(crate) fn usage_from_openai(usage: &OpenAIUsage) -> Usage {
    let cached = usage
        .prompt_tokens_details
        .as_ref()
        .and_then(|d| d.cached_tokens)
        .unwrap_or(0);
    Usage {
        input_tokens: usage.prompt_tokens.saturating_sub(cached),
        output_tokens: usage.completion_tokens,
        cache_read_tokens: cached,
        cache_write_tokens: 0,
    }
}

/// Normalize a Chat Completions `finish_reason` to the framework
/// stop_reason vocabulary. Unknown provider values pass through unchanged.
pub(crate) fn normalize_finish_reason(raw: &str) -> String {
    match raw {
        "stop" => "stop",
        "tool_calls" | "function_call" => "tool_use",
        "length" => "length",
        "content_filter" => "error",
        other => other,
    }
    .to_string()
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
            "parameters": info.parameters
        }
    })
}

/// Tool call being assembled from streaming fragments.
///
/// The Chat Completions API splits a tool call's id/name/arguments across
/// multiple chunks correlated by `ChatToolCallChunk.index`.
#[derive(Default)]
pub(crate) struct PendingToolCall {
    pub id: Option<String>,
    pub name: String,
    pub arguments: String,
}

/// Merge one delta's tool call fragments into the per-index buffers.
pub(crate) fn accumulate_tool_call_chunks(
    pending: &mut std::collections::BTreeMap<u32, PendingToolCall>,
    chunks: &[ChatToolCallChunk],
) {
    for call in chunks {
        let e = pending.entry(call.index).or_default();
        if let Some(id) = &call.id {
            e.id = Some(id.clone());
        }
        if let Some(f) = &call.function {
            if let Some(n) = &f.name {
                e.name.push_str(n);
            }
            if let Some(a) = &f.arguments {
                e.arguments.push_str(a);
            }
        }
    }
}

/// Finalize accumulated tool calls in index order once the stream completes.
pub(crate) fn finalize_pending_tool_calls(
    pending: std::collections::BTreeMap<u32, PendingToolCall>,
) -> Vec<ToolCall> {
    pending
        .into_values()
        .map(|p| {
            let (parameters, parse_error) = crate::json_repair::parse_tool_arguments(&p.arguments);
            ToolCall {
                function: ToolCallFunction {
                    id: p.id,
                    name: p.name,
                    parameters,
                    parse_error,
                },
            }
        })
        .collect()
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
                // function_call_output's `output` only accepts a string, so
                // block results degrade to text with image placeholders.
                input_items.push(serde_json::json!({
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": crate::content::tool_result_fallback_text(&msg.content),
                }));
            }
            "assistant" => {
                if let Some(tool_calls) = &msg.tool_calls {
                    if !msg.text().is_empty() {
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
    if let Some(image) = &msg.image {
        input_items.push(serde_json::json!({
            "type": "message",
            "role": role_str,
            "content": [
                { "type": "input_text", "text": msg.text() },
                { "type": "input_image", "detail": "auto", "image_url": image.get_base64() }
            ]
        }));
        return Ok(());
    }

    // Image blocks map to input_image parts in block order; plain text keeps
    // the legacy string form.
    if let MessageContent::Blocks(blocks) = &msg.content
        && blocks
            .iter()
            .any(|b| matches!(b, ContentBlock::Image { .. }))
    {
        let parts: Vec<serde_json::Value> = blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } if !text.is_empty() => {
                    Some(serde_json::json!({ "type": "input_text", "text": text }))
                }
                ContentBlock::Image { data, mime_type } => Some(serde_json::json!({
                    "type": "input_image",
                    "detail": "auto",
                    "image_url": format!("data:{mime_type};base64,{data}")
                })),
                _ => None,
            })
            .collect();
        input_items.push(serde_json::json!({
            "type": "message",
            "role": role_str,
            "content": parts
        }));
        return Ok(());
    }

    input_items.push(serde_json::json!({
        "type": "message",
        "role": role_str,
        "content": msg.text()
    }));
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
                let (parameters, parse_error) = crate::json_repair::parse_tool_arguments(arguments);
                tool_calls.push(ToolCall {
                    function: ToolCallFunction {
                        id: Some(call_id),
                        name,
                        parameters,
                        parse_error,
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

/// Derive a stable prompt cache key from the agent's preset and instance ids.
///
/// OpenAI caps `prompt_cache_key` at 64 chars; longer keys are rejected. The
/// key must be deterministic across calls of the same agent instance so that
/// repeated requests route to the same cache, so it is derived purely from
/// stable identifiers (no timestamps or random data).
pub(crate) fn prompt_cache_key(preset_id: &str, agent_id: &str) -> String {
    let key = if preset_id.is_empty() {
        agent_id.to_string()
    } else {
        format!("{}:{}", preset_id, agent_id)
    };
    // Clamp on a char boundary so a multi-byte id near the limit stays valid.
    if key.len() <= 64 {
        key
    } else {
        let mut end = 64;
        while !key.is_char_boundary(end) {
            end -= 1;
        }
        key[..end].to_string()
    }
}

/// Merge user options JSON into a request JSON object.
///
/// A `null` option removes the key from the request instead of serializing
/// `"key": null` — the escape hatch for defaults this client sets
/// unconditionally (e.g. `stream_options`) on OpenAI-compatible servers
/// that reject the parameter regardless of its value.
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
            if value.is_null() {
                req_obj.remove(key);
            } else {
                req_obj.insert(key.clone(), value.clone());
            }
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
                parse_error: None,
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
        assert_eq!(result.text(), "Hello!");
        assert!(result.tool_calls.is_none());
        assert!(result.thinking().is_none());
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
        assert_eq!(result.text(), "I'll check.");
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
            result.thinking(),
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
        // usage is consumed by the typed field, not the extra catch-all
        assert!(!resp.extra.contains_key("usage"));
        let usage = resp.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        // Unmodeled keys survive for the raw response port
        assert_eq!(usage.extra.get("total_tokens").unwrap(), 15);
    }

    #[test]
    fn test_serde_chat_completion_response_without_usage() {
        let json = r#"{"choices": []}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert!(resp.usage.is_none());
    }

    #[test]
    fn test_usage_from_openai_normalizes_cached_tokens() {
        let json = r#"{
            "prompt_tokens": 100,
            "completion_tokens": 20,
            "total_tokens": 120,
            "prompt_tokens_details": {"cached_tokens": 60, "audio_tokens": 0}
        }"#;
        let usage: OpenAIUsage = serde_json::from_str(json).unwrap();
        assert_eq!(
            usage_from_openai(&usage),
            Usage {
                input_tokens: 40,
                output_tokens: 20,
                cache_read_tokens: 60,
                cache_write_tokens: 0,
            }
        );
    }

    #[test]
    fn test_usage_from_openai_without_details() {
        let json = r#"{"prompt_tokens": 10, "completion_tokens": 5}"#;
        let usage: OpenAIUsage = serde_json::from_str(json).unwrap();
        assert_eq!(
            usage_from_openai(&usage),
            Usage {
                input_tokens: 10,
                output_tokens: 5,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            }
        );
    }

    #[test]
    fn test_usage_from_openai_cached_exceeding_prompt_saturates() {
        // Defensive: a server reporting cached > prompt must not underflow.
        let json = r#"{
            "prompt_tokens": 5,
            "completion_tokens": 1,
            "prompt_tokens_details": {"cached_tokens": 10}
        }"#;
        let usage: OpenAIUsage = serde_json::from_str(json).unwrap();
        assert_eq!(usage_from_openai(&usage).input_tokens, 0);
    }

    #[test]
    fn test_serde_chat_stream_chunk_usage_only_final_chunk() {
        // With stream_options.include_usage, the final chunk has EMPTY
        // choices and carries the usage object.
        let json = r#"{
            "id": "chatcmpl-123",
            "choices": [],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        }"#;
        let chunk: ChatStreamChunk = serde_json::from_str(json).unwrap();
        assert!(chunk.choices.is_empty());
        assert_eq!(chunk.usage.unwrap().prompt_tokens, 10);
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
    fn test_serde_response_stream_event_incomplete() {
        let json = r#"{"type": "response.incomplete", "response": {"status": "incomplete", "incomplete_details": {"reason": "max_output_tokens"}}}"#;
        let event: ResponseStreamEvent = serde_json::from_str(json).unwrap();
        if let ResponseStreamEvent::Incomplete { response } = event {
            assert_eq!(
                response["incomplete_details"]["reason"],
                "max_output_tokens"
            );
        } else {
            panic!("Expected Incomplete event");
        }
    }

    #[test]
    fn test_serde_response_stream_event_failed() {
        let json = r#"{"type": "response.failed", "response": {"status": "failed"}}"#;
        let event: ResponseStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, ResponseStreamEvent::Failed { .. }));
    }

    #[test]
    fn test_normalize_finish_reason() {
        assert_eq!(normalize_finish_reason("stop"), "stop");
        assert_eq!(normalize_finish_reason("tool_calls"), "tool_use");
        assert_eq!(normalize_finish_reason("function_call"), "tool_use");
        assert_eq!(normalize_finish_reason("length"), "length");
        assert_eq!(normalize_finish_reason("content_filter"), "error");
        // Unknown provider values pass through unchanged
        assert_eq!(normalize_finish_reason("weird_reason"), "weird_reason");
    }

    #[test]
    fn test_serde_response_stream_event_other() {
        let json = r#"{"type": "response.created", "response": {}}"#;
        let event: ResponseStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, ResponseStreamEvent::Other));
    }

    // =========================================================================
    // Streaming tool call accumulation
    // =========================================================================

    fn chunk(
        index: u32,
        id: Option<&str>,
        name: Option<&str>,
        args: Option<&str>,
    ) -> ChatToolCallChunk {
        ChatToolCallChunk {
            index,
            id: id.map(String::from),
            function: (name.is_some() || args.is_some()).then(|| ChatFunctionCallChunk {
                name: name.map(String::from),
                arguments: args.map(String::from),
            }),
        }
    }

    #[test]
    fn test_accumulate_interleaved_tool_calls() {
        let mut pending = std::collections::BTreeMap::new();

        // id/name arrive only in the first fragment of each call; arguments
        // are split across three deltas with the two calls interleaved.
        accumulate_tool_call_chunks(
            &mut pending,
            &[
                chunk(0, Some("call_a"), Some("get_weather"), Some("")),
                chunk(1, Some("call_b"), Some("search"), None),
            ],
        );
        accumulate_tool_call_chunks(
            &mut pending,
            &[
                chunk(1, None, None, Some(r#"{"q":"ru"#)),
                chunk(0, None, None, Some(r#"{"city":"#)),
            ],
        );
        accumulate_tool_call_chunks(
            &mut pending,
            &[
                chunk(0, None, None, Some(r#""Tokyo"}"#)),
                chunk(1, None, None, Some(r#"st"}"#)),
            ],
        );

        let calls = finalize_pending_tool_calls(pending);
        assert_eq!(calls.len(), 2);

        assert_eq!(calls[0].function.id, Some("call_a".to_string()));
        assert_eq!(calls[0].function.name, "get_weather");
        assert_eq!(
            calls[0].function.parameters,
            serde_json::json!({"city": "Tokyo"})
        );
        assert_eq!(calls[0].function.parse_error, None);

        assert_eq!(calls[1].function.id, Some("call_b".to_string()));
        assert_eq!(calls[1].function.name, "search");
        assert_eq!(
            calls[1].function.parameters,
            serde_json::json!({"q": "rust"})
        );
        assert_eq!(calls[1].function.parse_error, None);
    }

    #[test]
    fn test_finalize_empty_args_is_no_arg_call() {
        let mut pending = std::collections::BTreeMap::new();
        accumulate_tool_call_chunks(
            &mut pending,
            &[chunk(0, Some("call_a"), Some("ping"), None)],
        );

        let calls = finalize_pending_tool_calls(pending);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.parameters, serde_json::json!({}));
        assert_eq!(calls[0].function.parse_error, None);
    }

    #[test]
    fn test_finalize_unparseable_args_sets_parse_error() {
        let mut pending = std::collections::BTreeMap::new();
        accumulate_tool_call_chunks(
            &mut pending,
            &[chunk(
                0,
                Some("call_a"),
                Some("search"),
                Some(r#"{"q": trunc"#),
            )],
        );

        let calls = finalize_pending_tool_calls(pending);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.id, Some("call_a".to_string()));
        assert_eq!(calls[0].function.parameters, serde_json::json!({}));
        let err = calls[0].function.parse_error.as_deref().unwrap();
        assert!(err.contains(r#"{"q": trunc"#), "err was: {err}");
    }

    // =========================================================================
    // prompt_cache_key
    // =========================================================================

    #[test]
    fn test_prompt_cache_key_combines_ids() {
        assert_eq!(prompt_cache_key("preset1", "agent1"), "preset1:agent1");
    }

    #[test]
    fn test_prompt_cache_key_empty_preset_uses_agent_id() {
        assert_eq!(prompt_cache_key("", "agent1"), "agent1");
    }

    #[test]
    fn test_prompt_cache_key_is_deterministic() {
        assert_eq!(
            prompt_cache_key("p", "a"),
            prompt_cache_key("p", "a"),
            "same ids must yield the same key"
        );
    }

    #[test]
    fn test_prompt_cache_key_clamped_to_64_chars() {
        let preset = "p".repeat(50);
        let agent = "a".repeat(50);
        let key = prompt_cache_key(&preset, &agent);
        assert!(key.len() <= 64, "key len was {}", key.len());
    }

    #[test]
    fn test_prompt_cache_key_clamps_on_char_boundary() {
        // A multi-byte char straddling the 64-byte limit must not be split.
        let preset = "あ".repeat(30); // 90 bytes
        let key = prompt_cache_key(&preset, "agent");
        assert!(key.len() <= 64, "key len was {}", key.len());
        // Round-trips as valid UTF-8 (would panic on a bad boundary slice).
        assert!(!key.is_empty());
    }

    #[test]
    fn test_map_http_error() {
        assert!(matches!(
            map_http_error(401, "Unauthorized", None),
            AgentError::InvalidConfig(_)
        ));
        assert!(matches!(
            map_http_error(400, "Bad request", None),
            AgentError::InvalidValue(_)
        ));
        assert!(matches!(
            map_http_error(418, "I'm a teapot", None),
            AgentError::IoError(_)
        ));
    }

    #[test]
    fn test_map_http_error_rate_limited() {
        let err = map_http_error(429, "Rate limited", None);
        assert!(matches!(
            err,
            AgentError::RateLimited {
                retry_after: None,
                ..
            }
        ));

        let retry_after = Some(std::time::Duration::from_secs(30));
        let err = map_http_error(429, "Rate limited", retry_after);
        assert!(
            matches!(err, AgentError::RateLimited { retry_after: Some(d), .. } if d.as_secs() == 30)
        );
    }

    #[test]
    fn test_map_http_error_quota_exhausted_not_retryable() {
        let err = map_http_error(
            429,
            "You exceeded your current quota, please check your plan and billing details.",
            None,
        );
        assert!(matches!(err, AgentError::InvalidConfig(_)));
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_map_http_error_overloaded() {
        assert!(matches!(
            map_http_error(500, "Server error", None),
            AgentError::Overloaded(_)
        ));
        assert!(matches!(
            map_http_error(503, "Service unavailable", None),
            AgentError::Overloaded(_)
        ));
        let err = map_http_error(500, "Server error", None);
        if let AgentError::Overloaded(msg) = err {
            assert!(msg.contains("500"), "msg was: {msg}");
            assert!(msg.contains("OpenAI"), "msg was: {msg}");
        } else {
            panic!("Expected Overloaded");
        }
    }

    #[test]
    fn test_map_http_error_context_overflow() {
        assert!(matches!(
            map_http_error(
                400,
                "This model's maximum context length is 128000 tokens",
                None
            ),
            AgentError::ContextOverflow(_)
        ));
        assert!(matches!(
            map_http_error(
                400,
                "Your input exceeds the context window of this model",
                None
            ),
            AgentError::ContextOverflow(_)
        ));
    }

    #[test]
    fn test_map_http_error_rate_limit_wording_excluded_from_overflow() {
        // A 429 whose body mentions context size must stay RateLimited
        assert!(matches!(
            map_http_error(429, "maximum context length rate limit reached", None),
            AgentError::RateLimited { .. }
        ));
        // A 400 mentioning both overflow and rate limit wording is not overflow
        assert!(matches!(
            map_http_error(400, "maximum context length; rate limit applies", None),
            AgentError::InvalidValue(_)
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
    fn test_response_input_user_image_blocks() {
        let mut msg = Message::user(String::new());
        msg.content = MessageContent::Blocks(vec![
            ContentBlock::Image {
                data: "iVBORw0KGgo=".to_string(),
                mime_type: "image/png".to_string(),
            },
            ContentBlock::Text {
                text: "what is this?".to_string(),
            },
        ]);
        let messages = vector![AgentValue::from(msg)];
        let items = messages_to_response_input(&messages).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0]["content"],
            serde_json::json!([
                {"type": "input_image", "detail": "auto",
                 "image_url": "data:image/png;base64,iVBORw0KGgo="},
                {"type": "input_text", "text": "what is this?"},
            ])
        );
    }

    #[test]
    fn test_message_to_chat_json_user_image_blocks() {
        let mut msg = Message::user(String::new());
        msg.content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "what is this?".to_string(),
            },
            ContentBlock::Image {
                data: "iVBORw0KGgo=".to_string(),
                mime_type: "image/png".to_string(),
            },
        ]);

        let json = message_to_chat_json(&msg);
        assert_eq!(
            json["content"],
            serde_json::json!([
                {"type": "text", "text": "what is this?"},
                {"type": "image_url", "image_url": {
                    "url": "data:image/png;base64,iVBORw0KGgo=", "detail": "auto"}},
            ])
        );
    }

    #[test]
    fn test_message_to_chat_json_tool_result_blocks_fallback() {
        let mut msg = Message::tool_with_content(
            "my_tool".to_string(),
            MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: "caption".to_string(),
                },
                ContentBlock::Image {
                    data: "iVBORw0KGgo=".to_string(),
                    mime_type: "image/png".to_string(),
                },
            ]),
        );
        msg.id = Some("call_img".to_string());

        let json = message_to_chat_json(&msg);
        assert_eq!(json["content"], "caption\n[image: image/png]");
        assert_eq!(json["tool_call_id"], "call_img");
    }

    #[test]
    fn test_message_to_chat_json_tool_result_text_unchanged() {
        let mut msg = Message::tool("my_tool".to_string(), "plain result".to_string());
        msg.id = Some("call_txt".to_string());

        let json = message_to_chat_json(&msg);
        assert_eq!(json["content"], "plain result");
    }

    #[test]
    fn test_response_input_tool_result_blocks_fallback() {
        let mut msg = Message::tool_with_content(
            "my_tool".to_string(),
            MessageContent::Blocks(vec![ContentBlock::Image {
                data: "iVBORw0KGgo=".to_string(),
                mime_type: "image/png".to_string(),
            }]),
        );
        msg.id = Some("call_img".to_string());

        let items = messages_to_response_input(&vector![AgentValue::from(msg)]).unwrap();
        assert_eq!(items[0]["type"], "function_call_output");
        assert_eq!(items[0]["output"], "[image: image/png]");
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
        assert_eq!(msg.text(), "Hello!");
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
        assert_eq!(msg.text(), "I'll check.");
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
        assert_eq!(msg.text(), "[Refusal: I cannot do that.]");
    }

    // =========================================================================
    // merge_options
    // =========================================================================

    #[test]
    fn test_merge_options_inserts_and_null_removes_key() {
        let mut request = serde_json::json!({
            "model": "gpt-5-nano",
            "stream_options": { "include_usage": true },
        });
        let mut options: AgentValueMap<String, AgentValue> = AgentValueMap::new();
        options.insert("stream_options".into(), AgentValue::unit());
        options.insert("seed".into(), AgentValue::integer(42));
        merge_options(&mut request, &options).unwrap();
        assert!(request.get("stream_options").is_none());
        assert_eq!(request["seed"], 42);
    }
}
