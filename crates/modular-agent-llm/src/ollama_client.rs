#![cfg(feature = "ollama")]

use std::sync::{Arc, Mutex};

use modular_agent_core::tool;
use modular_agent_core::{
    AgentError, AgentValue, AgentValueMap, Message, ModularAgent, ToolCall, ToolCallFunction,
};

use crate::chat::ChatAgent;

use im::vector;

const CONFIG_OLLAMA_API_KEY: &str = "ollama_api_key";
const CONFIG_OLLAMA_URL: &str = "ollama_url";
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

// ============================================================================
// Client management
// ============================================================================

#[derive(Clone)]
pub(crate) struct OllamaClient {
    http: reqwest::Client,
    api_base: String,
    api_key: String,
}

pub struct OllamaManager {
    client: Arc<Mutex<Option<OllamaClient>>>,
}

impl OllamaManager {
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_ollama_url(ma: &ModularAgent) -> String {
        if let Some(ollama_url) = ma
            .get_global_configs(ChatAgent::DEF_NAME)
            .and_then(|cfg| cfg.get_string(CONFIG_OLLAMA_URL).ok())
            .filter(|url| !url.is_empty())
        {
            return ollama_url;
        }
        if let Ok(ollama_api_base_url) = std::env::var("OLLAMA_API_BASE_URL") {
            return ollama_api_base_url;
        } else if let Ok(ollama_host) = std::env::var("OLLAMA_HOST") {
            return format!("http://{}:11434", ollama_host);
        }
        DEFAULT_OLLAMA_URL.to_string()
    }

    pub fn get_ollama_api_key(ma: &ModularAgent) -> String {
        ma.get_global_configs(ChatAgent::DEF_NAME)
            .and_then(|cfg| cfg.get_string(CONFIG_OLLAMA_API_KEY).ok())
            .filter(|key| !key.is_empty())
            .or_else(|| {
                std::env::var("OLLAMA_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
            })
            .unwrap_or_default()
    }

    pub fn get_client(&self, ma: &ModularAgent) -> Result<OllamaClient, AgentError> {
        let mut client_guard = self.client.lock().unwrap();

        if let Some(client) = client_guard.as_ref() {
            return Ok(client.clone());
        }

        let api_base = Self::get_ollama_url(ma);
        let api_key = Self::get_ollama_api_key(ma);
        let new_client = OllamaClient {
            http: reqwest::Client::new(),
            api_base,
            api_key,
        };
        *client_guard = Some(new_client.clone());

        Ok(new_client)
    }
}

impl Default for OllamaManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// HTTP request methods
// ============================================================================

impl OllamaClient {
    pub(crate) fn chat_url(&self) -> String {
        format!("{}/api/chat", self.api_base.trim_end_matches('/'))
    }

    pub(crate) fn generate_url(&self) -> String {
        format!("{}/api/generate", self.api_base.trim_end_matches('/'))
    }

    fn embed_url(&self) -> String {
        format!("{}/api/embed", self.api_base.trim_end_matches('/'))
    }

    fn tags_url(&self) -> String {
        format!("{}/api/tags", self.api_base.trim_end_matches('/'))
    }

    fn show_url(&self) -> String {
        format!("{}/api/show", self.api_base.trim_end_matches('/'))
    }

    fn apply_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if !self.api_key.is_empty() {
            builder.header("Authorization", format!("Bearer {}", self.api_key))
        } else {
            builder
        }
    }

    /// POST JSON and parse typed response (non-streaming).
    pub(crate) async fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<T, AgentError> {
        let resp = self
            .apply_auth(self.http.post(url))
            .header("content-type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama request error: {}", e)))?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }

        resp.json()
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama response parse error: {}", e)))
    }

    /// GET and parse typed response.
    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, AgentError> {
        let resp = self
            .apply_auth(self.http.get(url))
            .send()
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama request error: {}", e)))?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }

        resp.json()
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama response parse error: {}", e)))
    }

    /// POST and return an NDJSON stream.
    ///
    /// Ollama uses Newline-Delimited JSON (NOT SSE), so we read the
    /// byte stream, buffer by newlines, and parse each line as JSON.
    pub(crate) async fn post_ndjson_stream<T: serde::de::DeserializeOwned + 'static>(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<T, AgentError>> + Send>>,
        AgentError,
    > {
        use futures::StreamExt;

        let resp = self
            .apply_auth(self.http.post(url))
            .header("content-type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama stream request error: {}", e)))?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }

        // Map bytes stream to String chunks to avoid direct bytes crate dependency
        let string_stream = Box::pin(resp.bytes_stream().map(|result| {
            result
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .map_err(|e| AgentError::IoError(format!("Ollama stream error: {}", e)))
        }));
        Ok(Box::pin(ndjson_stream::<T, _>(string_stream)))
    }

    // ========================================================================
    // High-level API methods
    // ========================================================================

    /// Generate embeddings via POST /api/embed.
    pub(crate) async fn generate_embeddings(
        &self,
        input: Vec<String>,
        model_name: &str,
        options: &serde_json::Value,
    ) -> Result<Vec<Vec<f32>>, AgentError> {
        let mut request = serde_json::json!({
            "model": model_name,
            "input": input,
        });
        if options.is_object() && !options.as_object().unwrap().is_empty() {
            request["options"] = options.clone();
        }

        let res: EmbedResponse = self.post_json(&self.embed_url(), &request).await?;
        Ok(res.embeddings)
    }

    /// List local models via GET /api/tags.
    pub(crate) async fn list_local_models(&self) -> Result<serde_json::Value, AgentError> {
        let res: ListModelsResponse = self.get_json(&self.tags_url()).await?;
        serde_json::to_value(&res.models)
            .map_err(|e| AgentError::IoError(format!("Serialization error: {}", e)))
    }

    /// Show model info via POST /api/show.
    pub(crate) async fn show_model_info(
        &self,
        model_name: &str,
    ) -> Result<serde_json::Value, AgentError> {
        let request = serde_json::json!({ "name": model_name });
        let res: serde_json::Value = self.post_json(&self.show_url(), &request).await?;
        Ok(res)
    }
}

/// Parse an NDJSON stream from a string-chunk stream.
///
/// Buffers incoming string chunks, splits on newline boundaries, and
/// deserializes each complete line as JSON of type T.
fn ndjson_stream<T, S>(chunk_stream: S) -> impl futures::Stream<Item = Result<T, AgentError>>
where
    T: serde::de::DeserializeOwned,
    S: futures::Stream<Item = Result<String, AgentError>> + Unpin,
{
    use futures::StreamExt;

    futures::stream::unfold(
        (chunk_stream, String::new()),
        |(mut stream, mut buffer)| async move {
            loop {
                // Check if buffer contains a complete line
                if let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].trim().to_string();
                    buffer = buffer[pos + 1..].to_string();
                    if line.is_empty() {
                        continue;
                    }
                    let result = serde_json::from_str::<T>(&line).map_err(|e| {
                        AgentError::IoError(format!("Ollama stream parse error: {}", e))
                    });
                    return Some((result, (stream, buffer)));
                }

                // Need more data from the stream
                match stream.next().await {
                    Some(Ok(chunk)) => {
                        buffer.push_str(&chunk);
                    }
                    Some(Err(e)) => {
                        return Some((Err(e), (stream, buffer)));
                    }
                    None => {
                        // Stream ended; parse any remaining buffer
                        let remaining = buffer.trim().to_string();
                        if !remaining.is_empty() {
                            buffer.clear();
                            let result = serde_json::from_str::<T>(&remaining).map_err(|e| {
                                AgentError::IoError(format!("Ollama stream parse error: {}", e))
                            });
                            return Some((result, (stream, buffer)));
                        }
                        return None;
                    }
                }
            }
        },
    )
}

fn map_http_error(status: u16, body: &str) -> AgentError {
    match status {
        400 => AgentError::InvalidValue(format!("Ollama Bad Request: {}", body)),
        401 => AgentError::InvalidConfig(format!("Ollama authentication failed: {}", body)),
        404 => AgentError::InvalidConfig(format!("Ollama model not found: {}", body)),
        _ => AgentError::IoError(format!("Ollama API Error ({}): {}", status, body)),
    }
}

// ============================================================================
// Serde type definitions — Chat
// ============================================================================

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub(crate) struct OllamaChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<OllamaToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub(crate) struct OllamaToolCall {
    pub function: OllamaToolCallFunction,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub(crate) struct OllamaToolCallFunction {
    pub name: String,
    #[serde(alias = "parameters")]
    pub arguments: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub(crate) struct OllamaToolInfo {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OllamaToolFunctionInfo,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub(crate) struct OllamaToolFunctionInfo {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub(crate) struct ChatResponse {
    pub model: String,
    pub created_at: String,
    pub message: OllamaChatMessage,
    pub done: bool,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// ============================================================================
// Serde type definitions — Generate (Completion)
// ============================================================================

/// Context for multi-turn completion (returned by /api/generate).
pub type GenerationContext = Vec<i64>;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub(crate) struct GenerateResponse {
    pub model: String,
    pub created_at: String,
    pub response: String,
    pub done: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<GenerationContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// ============================================================================
// Serde type definitions — Embeddings
// ============================================================================

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub(crate) struct EmbedResponse {
    pub embeddings: Vec<Vec<f32>>,
}

// ============================================================================
// Serde type definitions — Models
// ============================================================================

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
struct ListModelsResponse {
    models: Vec<serde_json::Value>,
}

// ============================================================================
// Message conversion functions
// ============================================================================

/// Convert an Ollama API chat message to an internal Message.
pub(crate) fn message_from_ollama(msg: &OllamaChatMessage) -> Message {
    let mut message = Message::new(msg.role.clone(), msg.content.clone());
    if let Some(thinking) = &msg.thinking
        && !thinking.is_empty()
    {
        message.thinking = Some(thinking.clone());
    }
    if !msg.tool_calls.is_empty() {
        let mut calls = vector![];
        for call in &msg.tool_calls {
            let tool_call = ToolCall {
                function: ToolCallFunction {
                    id: None,
                    name: call.function.name.clone(),
                    parameters: call.function.arguments.clone(),
                },
            };
            calls.push_back(tool_call);
        }
        message.tool_calls = Some(calls);
    }
    message
}

/// Convert an internal Message to an Ollama API chat message.
pub(crate) fn message_to_ollama(msg: &Message) -> OllamaChatMessage {
    let mut omsg = OllamaChatMessage {
        role: msg.role.clone(),
        content: msg.content.clone(),
        tool_calls: vec![],
        images: None,
        thinking: None,
    };

    if let Some(tool_calls) = &msg.tool_calls {
        omsg.tool_calls = tool_calls
            .iter()
            .map(|tc| OllamaToolCall {
                function: OllamaToolCallFunction {
                    name: tc.function.name.clone(),
                    arguments: tc.function.parameters.clone(),
                },
            })
            .collect();
    }

    #[cfg(feature = "image")]
    {
        if let Some(img) = &msg.image {
            let img_str = img
                .get_base64()
                .trim_start_matches("data:image/png;base64,")
                .to_string();
            omsg.images = Some(vec![img_str]);
        }
    }

    omsg
}

/// Convert a framework ToolInfo to an Ollama tool definition.
pub(crate) fn tool_info_to_ollama(info: tool::ToolInfo) -> OllamaToolInfo {
    let parameters = info.parameters.unwrap_or_else(|| serde_json::json!({}));
    OllamaToolInfo {
        tool_type: "function".to_string(),
        function: OllamaToolFunctionInfo {
            name: info.name,
            description: info.description,
            parameters,
        },
    }
}

/// Merge user options into the Ollama `"options"` field of a request.
///
/// Unlike OpenAI (which flat-merges into the top-level), Ollama requires
/// model parameters like `temperature` to be nested under `"options"`.
pub(crate) fn merge_options(
    request: &mut serde_json::Value,
    config_options: &AgentValueMap<String, AgentValue>,
) -> Result<(), AgentError> {
    if config_options.is_empty() {
        return Ok(());
    }
    let options_json = serde_json::to_value(config_options)
        .map_err(|e| AgentError::InvalidValue(format!("Invalid JSON in options: {}", e)))?;
    request["options"] = options_json;
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use modular_agent_core::ToolCallFunction;

    // =========================================================================
    // Message conversion
    // =========================================================================

    #[test]
    fn test_message_to_ollama_assistant_without_tool_calls() {
        let msg = Message::assistant("Hello".to_string());
        let omsg = message_to_ollama(&msg);
        assert!(omsg.tool_calls.is_empty());
        assert_eq!(omsg.content, "Hello");
        assert_eq!(omsg.role, "assistant");
    }

    #[test]
    fn test_message_to_ollama_assistant_with_tool_calls() {
        let mut msg = Message::assistant("".to_string());
        msg.tool_calls = Some(im::vector![ToolCall {
            function: ToolCallFunction {
                id: Some("call_1".to_string()),
                name: "get_weather".to_string(),
                parameters: serde_json::json!({"city": "Tokyo"}),
            },
        }]);

        let omsg = message_to_ollama(&msg);
        assert_eq!(omsg.tool_calls.len(), 1);
        assert_eq!(omsg.tool_calls[0].function.name, "get_weather");
        assert_eq!(
            omsg.tool_calls[0].function.arguments,
            serde_json::json!({"city": "Tokyo"})
        );
    }

    #[test]
    fn test_message_to_ollama_user_no_tool_calls() {
        let msg = Message::user("Hello".to_string());
        let omsg = message_to_ollama(&msg);
        assert!(omsg.tool_calls.is_empty());
        assert_eq!(omsg.content, "Hello");
    }

    #[test]
    fn test_message_to_ollama_tool_result() {
        let msg = Message::tool("my_tool".to_string(), "result".to_string());
        let omsg = message_to_ollama(&msg);
        assert_eq!(omsg.content, "result");
        assert!(omsg.tool_calls.is_empty());
    }

    #[test]
    fn test_message_from_ollama_text() {
        let omsg = OllamaChatMessage {
            role: "assistant".to_string(),
            content: "Hello!".to_string(),
            tool_calls: vec![],
            images: None,
            thinking: None,
        };
        let msg = message_from_ollama(&omsg);
        assert_eq!(msg.content, "Hello!");
        assert_eq!(msg.role, "assistant");
        assert!(msg.tool_calls.is_none());
        assert!(msg.thinking.is_none());
    }

    #[test]
    fn test_message_from_ollama_with_tool_calls() {
        let omsg = OllamaChatMessage {
            role: "assistant".to_string(),
            content: "".to_string(),
            tool_calls: vec![OllamaToolCall {
                function: OllamaToolCallFunction {
                    name: "search".to_string(),
                    arguments: serde_json::json!({"q": "test"}),
                },
            }],
            images: None,
            thinking: None,
        };
        let msg = message_from_ollama(&omsg);
        let tc = msg.tool_calls.unwrap();
        assert_eq!(tc.len(), 1);
        assert_eq!(tc[0].function.name, "search");
        assert_eq!(tc[0].function.parameters, serde_json::json!({"q": "test"}));
    }

    #[test]
    fn test_message_from_ollama_with_thinking() {
        let omsg = OllamaChatMessage {
            role: "assistant".to_string(),
            content: "Answer".to_string(),
            tool_calls: vec![],
            images: None,
            thinking: Some("Let me think...".to_string()),
        };
        let msg = message_from_ollama(&omsg);
        assert_eq!(msg.content, "Answer");
        assert_eq!(msg.thinking, Some("Let me think...".to_string()));
    }

    // =========================================================================
    // Serde: response types
    // =========================================================================

    #[test]
    fn test_serde_chat_response() {
        let json = r#"{
            "model": "llama3.2",
            "created_at": "2024-01-01T00:00:00Z",
            "message": {
                "role": "assistant",
                "content": "Hello!"
            },
            "done": true,
            "total_duration": 1234567890,
            "eval_count": 42
        }"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.model, "llama3.2");
        assert!(resp.done);
        assert_eq!(resp.message.content, "Hello!");
        assert!(resp.extra.contains_key("total_duration"));
        assert!(resp.extra.contains_key("eval_count"));
    }

    #[test]
    fn test_serde_chat_response_with_tool_calls() {
        let json = r#"{
            "model": "llama3.2",
            "created_at": "2024-01-01T00:00:00Z",
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    {
                        "function": {
                            "name": "get_weather",
                            "arguments": {"city": "Tokyo"}
                        }
                    }
                ]
            },
            "done": true
        }"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.message.tool_calls.len(), 1);
        assert_eq!(resp.message.tool_calls[0].function.name, "get_weather");
    }

    #[test]
    fn test_serde_chat_response_stream_chunk() {
        let json = r#"{
            "model": "llama3.2",
            "created_at": "2024-01-01T00:00:00Z",
            "message": {
                "role": "assistant",
                "content": "Hello"
            },
            "done": false
        }"#;
        let chunk: ChatResponse = serde_json::from_str(json).unwrap();
        assert!(!chunk.done);
        assert_eq!(chunk.message.content, "Hello");
    }

    #[test]
    fn test_serde_generate_response() {
        let json = r#"{
            "model": "codellama",
            "created_at": "2024-01-01T00:00:00Z",
            "response": "def hello():",
            "done": true,
            "context": [1, 2, 3]
        }"#;
        let resp: GenerateResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.response, "def hello():");
        assert_eq!(resp.context, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_serde_generate_response_no_context() {
        let json = r#"{
            "model": "codellama",
            "created_at": "2024-01-01T00:00:00Z",
            "response": "hello",
            "done": true
        }"#;
        let resp: GenerateResponse = serde_json::from_str(json).unwrap();
        assert!(resp.context.is_none());
    }

    #[test]
    fn test_serde_embed_response() {
        let json = r#"{"embeddings": [[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]]}"#;
        let resp: EmbedResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.embeddings.len(), 2);
        assert_eq!(resp.embeddings[0], vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_serde_list_models_response() {
        let json = r#"{"models": [{"name": "llama3.2:latest", "size": 12345}]}"#;
        let resp: ListModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.models.len(), 1);
        assert_eq!(resp.models[0]["name"], "llama3.2:latest");
    }

    #[test]
    fn test_serde_tool_call_with_parameters_alias() {
        // Ollama may return "parameters" instead of "arguments"
        let json = r#"{"function": {"name": "search", "parameters": {"q": "test"}}}"#;
        let tc: OllamaToolCall = serde_json::from_str(json).unwrap();
        assert_eq!(tc.function.name, "search");
        assert_eq!(tc.function.arguments, serde_json::json!({"q": "test"}));
    }

    // =========================================================================
    // Tool info conversion
    // =========================================================================

    #[test]
    fn test_tool_info_to_ollama() {
        let info = tool::ToolInfo {
            name: "get_weather".to_string(),
            description: "Get weather".to_string(),
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "location": { "type": "string" }
                }
            })),
        };
        let tool = tool_info_to_ollama(info);
        assert_eq!(tool.tool_type, "function");
        assert_eq!(tool.function.name, "get_weather");
        assert_eq!(tool.function.description, "Get weather");
        assert_eq!(tool.function.parameters["type"], "object");
    }

    #[test]
    fn test_tool_info_to_ollama_no_params() {
        let info = tool::ToolInfo {
            name: "list_items".to_string(),
            description: "List items".to_string(),
            parameters: None,
        };
        let tool = tool_info_to_ollama(info);
        assert_eq!(tool.function.parameters, serde_json::json!({}));
    }

    // =========================================================================
    // merge_options
    // =========================================================================

    #[test]
    fn test_merge_options_empty() {
        let mut request = serde_json::json!({"model": "test"});
        let options = AgentValueMap::new();
        merge_options(&mut request, &options).unwrap();
        assert!(request.get("options").is_none());
    }

    #[test]
    fn test_merge_options_nests_under_options_key() {
        let mut request = serde_json::json!({"model": "test"});
        let mut options = AgentValueMap::new();
        options.insert("temperature".to_string(), AgentValue::from(0.7f64));
        merge_options(&mut request, &options).unwrap();
        assert!(request["options"]["temperature"].is_number());
    }

    // =========================================================================
    // map_http_error
    // =========================================================================

    #[test]
    fn test_map_http_error() {
        assert!(matches!(
            map_http_error(400, "bad"),
            AgentError::InvalidValue(_)
        ));
        assert!(matches!(
            map_http_error(401, "unauthorized"),
            AgentError::InvalidConfig(_)
        ));
        assert!(matches!(
            map_http_error(404, "not found"),
            AgentError::InvalidConfig(_)
        ));
        assert!(matches!(
            map_http_error(500, "internal"),
            AgentError::IoError(_)
        ));
    }
}
