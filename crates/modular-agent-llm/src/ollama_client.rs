#![cfg(feature = "ollama")]

use std::sync::{Arc, Mutex};

use modular_agent_core::tool;
use modular_agent_core::{AgentError, Message, ModularAgent, ToolCall, ToolCallFunction};

use crate::chat::ChatAgent;

use im::vector;
pub use ollama_rs::generation::completion::GenerationContext;
pub use ollama_rs::generation::embeddings::request::EmbeddingsInput;
pub use ollama_rs::models::ModelOptions;
use ollama_rs::{
    Ollama,
    generation::{
        chat::{ChatMessage, MessageRole},
        embeddings::request::GenerateEmbeddingsRequest,
        tools::ToolCall as OllamaToolCall,
        tools::ToolCallFunction as OllamaToolCallFunction,
    },
};
use schemars::{Schema, json_schema};

const CONFIG_OLLAMA_URL: &str = "ollama_url";

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Shared client management for Ollama
pub struct OllamaManager {
    client: Arc<Mutex<Option<Ollama>>>,
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

    pub fn get_client(&self, ma: &ModularAgent) -> Result<Ollama, AgentError> {
        let mut client_guard = self.client.lock().unwrap();

        if let Some(client) = client_guard.as_ref() {
            return Ok(client.clone());
        }

        let api_base_url = Self::get_ollama_url(ma);
        let new_client = Ollama::try_new(api_base_url)
            .map_err(|e| AgentError::IoError(format!("Ollama Client Error: {}", e)))?;
        *client_guard = Some(new_client.clone());

        Ok(new_client)
    }
}

impl Default for OllamaManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate embeddings
pub async fn generate_embeddings(
    client: &Ollama,
    input: EmbeddingsInput,
    model_name: &str,
    model_options: Option<ModelOptions>,
) -> Result<Vec<Vec<f32>>, AgentError> {
    let mut request = GenerateEmbeddingsRequest::new(model_name.to_string(), input);
    if let Some(options) = model_options {
        request = request.options(options);
    }
    let res = client
        .generate_embeddings(request)
        .await
        .map_err(|e| AgentError::IoError(format!("generate_embeddings: {}", e)))?;
    Ok(res.embeddings)
}

// Message conversion functions

pub fn message_from_ollama(msg: ChatMessage) -> Message {
    let role = match msg.role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::System => "system",
        MessageRole::Tool => "tool",
    };
    let mut message = Message::new(role.to_string(), msg.content);
    if !msg.tool_calls.is_empty() {
        let mut calls = vector![];
        for call in msg.tool_calls {
            let tool_call = ToolCall {
                function: ToolCallFunction {
                    id: None,
                    name: call.function.name,
                    parameters: call.function.arguments,
                },
            };
            calls.push_back(tool_call);
        }
        message.tool_calls = Some(calls);
    }
    message
}

pub fn message_to_chat(msg: Message) -> ChatMessage {
    let mut cmsg = match msg.role.as_str() {
        "user" => ChatMessage::user(msg.content),
        "assistant" => ChatMessage::assistant(msg.content),
        "system" => ChatMessage::system(msg.content),
        "tool" => ChatMessage::tool(msg.content),
        _ => ChatMessage::user(msg.content),
    };
    // Set tool_calls on assistant messages for tool call round-trip
    if let Some(tool_calls) = &msg.tool_calls {
        cmsg.tool_calls = tool_calls
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
        if let Some(img) = msg.image {
            let img_str = img
                .get_base64()
                .trim_start_matches("data:image/png;base64,")
                .to_string();
            cmsg = cmsg.add_image(ollama_rs::generation::images::Image::from_base64(img_str));
        }
    }
    cmsg
}

pub fn from_tool_info_to_ollama_tool_info(
    info: tool::ToolInfo,
) -> ollama_rs::generation::tools::ToolInfo {
    let schema: Schema = if let Some(params) = info.parameters {
        Schema::try_from(params).unwrap_or_else(|_| json_schema!({}))
    } else {
        json_schema!({})
    };
    ollama_rs::generation::tools::ToolInfo {
        tool_type: ollama_rs::generation::tools::ToolType::Function,
        function: ollama_rs::generation::tools::ToolFunctionInfo {
            name: info.name,
            description: info.description,
            parameters: schema,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use modular_agent_core::ToolCallFunction;

    #[test]
    fn test_message_to_chat_assistant_without_tool_calls() {
        let msg = Message::assistant("Hello".to_string());
        let cmsg = message_to_chat(msg);
        assert!(cmsg.tool_calls.is_empty());
        assert_eq!(cmsg.content, "Hello");
    }

    #[test]
    fn test_message_to_chat_assistant_with_tool_calls() {
        let mut msg = Message::assistant("".to_string());
        msg.tool_calls = Some(im::vector![ToolCall {
            function: ToolCallFunction {
                id: Some("call_1".to_string()),
                name: "get_weather".to_string(),
                parameters: serde_json::json!({"city": "Tokyo"}),
            },
        }]);

        let cmsg = message_to_chat(msg);
        assert_eq!(cmsg.tool_calls.len(), 1);
        assert_eq!(cmsg.tool_calls[0].function.name, "get_weather");
        assert_eq!(
            cmsg.tool_calls[0].function.arguments,
            serde_json::json!({"city": "Tokyo"})
        );
    }

    #[test]
    fn test_message_to_chat_user_no_tool_calls() {
        let msg = Message::user("Hello".to_string());
        let cmsg = message_to_chat(msg);
        assert!(cmsg.tool_calls.is_empty());
        assert_eq!(cmsg.content, "Hello");
    }

    #[test]
    fn test_message_to_chat_tool_result() {
        let msg = Message::tool("my_tool".to_string(), "result".to_string());
        let cmsg = message_to_chat(msg);
        assert_eq!(cmsg.content, "result");
        assert!(cmsg.tool_calls.is_empty());
    }
}
