#![cfg(feature = "openai")]

use std::sync::{Arc, Mutex};

use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCallChunk,
    ChatCompletionMessageToolCalls, ChatCompletionRequestAssistantMessageArgs,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs,
    ChatCompletionResponseMessage, ChatCompletionTool, FunctionObject, Role,
};
use async_openai::types::embeddings::{CreateEmbeddingRequest, CreateEmbeddingRequestArgs};
use async_openai::{Client, config::OpenAIConfig};
use modular_agent_core::tool;
use modular_agent_core::{
    AgentError, AgentValue, AgentValueMap, Message, ModularAgent, ToolCall, ToolCallFunction,
};

use crate::chat::ChatAgent;

const CONFIG_OPENAI_API_KEY: &str = "openai_api_key";
const CONFIG_OPENAI_API_BASE: &str = "openai_api_base";

/// Shared client management for OpenAI
pub struct OpenAIManager {
    client: Arc<Mutex<Option<Client<OpenAIConfig>>>>,
}

impl OpenAIManager {
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_client(&self, ma: &ModularAgent) -> Result<Client<OpenAIConfig>, AgentError> {
        let mut client_guard = self.client.lock().unwrap();

        if let Some(client) = client_guard.as_ref() {
            return Ok(client.clone());
        }

        let mut config = OpenAIConfig::new();

        if let Some(api_key) = ma
            .get_global_configs(ChatAgent::DEF_NAME)
            .and_then(|cfg| cfg.get_string(CONFIG_OPENAI_API_KEY).ok())
            .filter(|key| !key.is_empty())
        {
            config = config.with_api_key(&api_key);
        }

        if let Some(api_base) = ma
            .get_global_configs(ChatAgent::DEF_NAME)
            .and_then(|cfg| cfg.get_string(CONFIG_OPENAI_API_BASE).ok())
            .filter(|key| !key.is_empty())
        {
            config = config.with_api_base(&api_base);
        }

        let new_client = Client::with_config(config);
        *client_guard = Some(new_client.clone());

        Ok(new_client)
    }
}

impl Default for OpenAIManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate embeddings
pub async fn generate_embeddings(
    client: &Client<OpenAIConfig>,
    texts: Vec<String>,
    model_name: &str,
    config_options: &AgentValueMap<String, AgentValue>,
) -> Result<Vec<Vec<f32>>, AgentError> {
    let mut request = CreateEmbeddingRequestArgs::default()
        .model(model_name.to_string())
        .input(texts)
        .build()
        .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

    if !config_options.is_empty() {
        let options_json = serde_json::to_value(config_options)
            .map_err(|e| AgentError::InvalidValue(format!("Invalid JSON in options: {}", e)))?;

        let mut request_json = serde_json::to_value(&request)
            .map_err(|e| AgentError::InvalidValue(format!("Serialization error: {}", e)))?;

        if let (Some(request_obj), Some(options_obj)) =
            (request_json.as_object_mut(), options_json.as_object())
        {
            for (key, value) in options_obj {
                request_obj.insert(key.clone(), value.clone());
            }
        }
        request = serde_json::from_value::<CreateEmbeddingRequest>(request_json)
            .map_err(|e| AgentError::InvalidValue(format!("Deserialization error: {}", e)))?;
    }

    let res = client
        .embeddings()
        .create(request)
        .await
        .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

    Ok(res.data.into_iter().map(|d| d.embedding).collect())
}

// Message conversion functions

pub fn message_from_openai_msg(msg: ChatCompletionResponseMessage) -> Message {
    let role = match msg.role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
        Role::Function => "function",
    };
    let content = msg.content.unwrap_or_default();
    let mut message = Message::new(role.to_string(), content);

    let thinking = msg
        .refusal
        .map(|r| format!("Refusal: {}", r))
        .unwrap_or_default();
    if !thinking.is_empty() {
        message.thinking = Some(thinking);
    }

    if let Some(tool_calls) = msg.tool_calls {
        let mut calls: Vec<ToolCall> = Vec::new();
        for call in tool_calls {
            if let ChatCompletionMessageToolCalls::Function(ref func_call) = call {
                if let Ok(c) = try_from_chat_completion_message_tool_call_to_tool_call(func_call) {
                    calls.push(c);
                }
            }
        }
        if !calls.is_empty() {
            message.tool_calls = Some(calls.into());
        }
    }

    message
}

pub fn message_to_chat_completion_msg(msg: &Message) -> ChatCompletionRequestMessage {
    match msg.role.as_str() {
        "system" => ChatCompletionRequestSystemMessageArgs::default()
            .content(msg.content.clone())
            .build()
            .unwrap()
            .into(),
        "user" => {
            #[cfg(feature = "image")]
            {
                if let Some(image) = &msg.image {
                    use async_openai::types::chat::{
                        ChatCompletionRequestMessageContentPartImage,
                        ChatCompletionRequestMessageContentPartText, ImageDetail, ImageUrl,
                    };

                    let image_url = ImageUrl {
                        url: image.get_base64(),
                        detail: Some(ImageDetail::Auto),
                    };
                    let img = ChatCompletionRequestMessageContentPartImage { image_url };
                    let text = ChatCompletionRequestMessageContentPartText {
                        text: msg.content.clone(),
                    };

                    return ChatCompletionRequestUserMessageArgs::default()
                        .content(vec![text.into(), img.into()])
                        .build()
                        .unwrap()
                        .into();
                }
            }
            ChatCompletionRequestUserMessageArgs::default()
                .content(msg.content.clone())
                .build()
                .unwrap()
                .into()
        }
        "assistant" => ChatCompletionRequestAssistantMessageArgs::default()
            .content(msg.content.clone())
            .build()
            .unwrap()
            .into(),
        "tool" => ChatCompletionRequestToolMessageArgs::default()
            .content(msg.content.clone())
            .tool_call_id(msg.id.clone().unwrap_or_default())
            .build()
            .unwrap()
            .into(),
        _ => ChatCompletionRequestUserMessageArgs::default()
            .content(msg.content.clone())
            .build()
            .unwrap()
            .into(),
    }
}

pub fn try_from_tool_info_to_chat_completion_tool(
    info: tool::ToolInfo,
) -> Result<ChatCompletionTool, AgentError> {
    let function = FunctionObject {
        name: info.name,
        description: if info.description.is_empty() {
            None
        } else {
            Some(info.description)
        },
        parameters: info.parameters,
        strict: None,
    };
    Ok(ChatCompletionTool { function })
}

pub fn try_from_chat_completion_message_tool_call_chunk_to_tool_call(
    call: &ChatCompletionMessageToolCallChunk,
) -> Result<ToolCall, AgentError> {
    let Some(function) = &call.function else {
        return Err(AgentError::InvalidValue(
            "ToolCallChunk missing function".to_string(),
        ));
    };
    let Some(name) = &function.name else {
        return Err(AgentError::InvalidValue(
            "ToolCallChunk function missing name".to_string(),
        ));
    };
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

fn try_from_chat_completion_message_tool_call_to_tool_call(
    call: &ChatCompletionMessageToolCall,
) -> Result<ToolCall, AgentError> {
    let parameters = serde_json::from_str(&call.function.arguments).map_err(|e| {
        AgentError::InvalidValue(format!("Failed to parse tool call arguments JSON: {}", e))
    })?;

    let function = ToolCallFunction {
        id: Some(call.id.clone()),
        name: call.function.name.clone(),
        parameters,
    };
    Ok(ToolCall { function })
}

// ============================================================================
// Responses API conversion functions
// ============================================================================

use async_openai::types::responses::{InputItem, OutputItem, OutputMessageContent};

/// Convert messages to Responses API input format.
///
/// The Responses API accepts input as a vector of InputItem, which can be:
/// - InputItem::Text(String) - simple text input
/// - InputItem::Message { role, content } - structured message
pub fn messages_to_response_input(
    messages: &im::Vector<AgentValue>,
) -> Result<Vec<InputItem>, AgentError> {
    let mut input_items = Vec::new();

    for msg_value in messages.iter() {
        let Some(msg) = msg_value.as_message() else {
            continue;
        };

        // Use serde_json to build the InputItem correctly
        // The InputItem structure varies by async-openai version
        let role_str = match msg.role.as_str() {
            "user" => "user",
            "assistant" => "assistant",
            "system" | "developer" => "developer",
            "tool" => "tool",
            _ => "user",
        };

        #[cfg(feature = "image")]
        let item = if let Some(image) = &msg.image {
            // Build message with image content
            let content = serde_json::json!([
                { "type": "input_text", "text": msg.content },
                { "type": "input_image", "detail": "auto", "image_url": image.get_base64() }
            ]);
            let item_json = serde_json::json!({
                "type": "message",
                "role": role_str,
                "content": content
            });
            serde_json::from_value::<InputItem>(item_json)
                .map_err(|e| AgentError::InvalidValue(format!("Failed to build input item: {}", e)))?
        } else {
            // Build text-only message
            let item_json = serde_json::json!({
                "type": "message",
                "role": role_str,
                "content": msg.content
            });
            serde_json::from_value::<InputItem>(item_json)
                .map_err(|e| AgentError::InvalidValue(format!("Failed to build input item: {}", e)))?
        };

        #[cfg(not(feature = "image"))]
        let item = {
            let item_json = serde_json::json!({
                "type": "message",
                "role": role_str,
                "content": msg.content
            });
            serde_json::from_value::<InputItem>(item_json)
                .map_err(|e| AgentError::InvalidValue(format!("Failed to build input item: {}", e)))?
        };

        input_items.push(item);
    }

    Ok(input_items)
}

/// Convert Responses API output to internal Message.
pub fn response_output_to_message(output: &[OutputItem]) -> Result<Message, AgentError> {
    let mut content = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    for item in output {
        match item {
            OutputItem::Message(msg) => {
                for part in &msg.content {
                    match part {
                        OutputMessageContent::OutputText(text_content) => {
                            content.push_str(&text_content.text);
                        }
                        OutputMessageContent::Refusal(refusal) => {
                            content.push_str(&format!("[Refusal: {}]", refusal.refusal));
                        }
                    }
                }
            }
            OutputItem::FunctionCall(fc) => {
                let parameters: serde_json::Value =
                    serde_json::from_str(&fc.arguments).unwrap_or_default();
                let tool_call = ToolCall {
                    function: ToolCallFunction {
                        id: Some(fc.call_id.clone()),
                        name: fc.name.clone(),
                        parameters,
                    },
                };
                tool_calls.push(tool_call);
            }
            // Handle other output types as needed (file_search, web_search, etc.)
            _ => {}
        }
    }

    let mut message = Message::assistant(content);
    if !tool_calls.is_empty() {
        message.tool_calls = Some(tool_calls.into());
    }

    Ok(message)
}
