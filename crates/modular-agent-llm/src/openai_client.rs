#![cfg(feature = "openai")]

use std::sync::{Arc, Mutex};

use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCallChunk,
    ChatCompletionMessageToolCalls, ChatCompletionRequestAssistantMessageArgs,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs,
    ChatCompletionResponseMessage, ChatCompletionTool, FunctionCall, FunctionObject, Role,
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
        "assistant" => {
            let mut builder = ChatCompletionRequestAssistantMessageArgs::default();
            builder.content(msg.content.clone());
            if let Some(tool_calls) = &msg.tool_calls {
                let tc: Vec<ChatCompletionMessageToolCalls> = tool_calls
                    .iter()
                    .map(|tc| {
                        ChatCompletionMessageToolCalls::Function(ChatCompletionMessageToolCall {
                            id: tc
                                .function
                                .id
                                .clone()
                                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                            function: FunctionCall {
                                name: tc.function.name.clone(),
                                arguments: tc.function.parameters.to_string(),
                            },
                        })
                    })
                    .collect();
                builder.tool_calls(tc);
            }
            builder.build().unwrap().into()
        }
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

use async_openai::types::responses::{
    FunctionCallOutput, FunctionCallOutputItemParam, FunctionToolCall, InputItem, Item, OutputItem,
    OutputMessageContent,
};

/// Convert messages to Responses API input format.
///
/// Maps internal Message types to the correct Responses API InputItem variants:
/// - Assistant messages with tool_calls → FunctionCall items
/// - Tool result messages → FunctionCallOutput items
/// - Other messages → EasyMessage items (via serde_json)
pub fn messages_to_response_input(
    messages: &im::Vector<AgentValue>,
) -> Result<Vec<InputItem>, AgentError> {
    let mut input_items = Vec::new();

    for msg_value in messages.iter() {
        let Some(msg) = msg_value.as_message() else {
            continue;
        };

        match msg.role.as_str() {
            "tool" => {
                // Tool result → function_call_output item
                let call_id = msg
                    .id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                input_items.push(InputItem::Item(Item::FunctionCallOutput(
                    FunctionCallOutputItemParam {
                        call_id,
                        output: FunctionCallOutput::Text(msg.content.clone()),
                        id: None,
                        status: None,
                    },
                )));
            }
            "assistant" => {
                if let Some(tool_calls) = &msg.tool_calls {
                    // Assistant text content as message item (if non-empty)
                    if !msg.content.is_empty() {
                        build_response_message_item(&mut input_items, "assistant", msg)?;
                    }
                    // Each tool_call → function_call item
                    for tc in tool_calls.iter() {
                        let call_id = tc
                            .function
                            .id
                            .clone()
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                        input_items.push(InputItem::Item(Item::FunctionCall(FunctionToolCall {
                            arguments: tc.function.parameters.to_string(),
                            call_id,
                            name: tc.function.name.clone(),
                            id: None,
                            status: None,
                        })));
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

/// Build a Responses API message InputItem (for user/assistant/developer roles).
fn build_response_message_item(
    input_items: &mut Vec<InputItem>,
    role_str: &str,
    msg: &Message,
) -> Result<(), AgentError> {
    #[cfg(feature = "image")]
    let item = if let Some(image) = &msg.image {
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
    Ok(())
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
    // Chat Completions: message_to_chat_completion_msg
    // =========================================================================

    #[test]
    fn test_chat_completion_assistant_without_tool_calls() {
        let msg = Message::assistant("Hello".to_string());
        let result = message_to_chat_completion_msg(&msg);
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["role"], "assistant");
        assert_eq!(json["content"], "Hello");
        assert!(json.get("tool_calls").is_none());
    }

    #[test]
    fn test_chat_completion_assistant_with_tool_calls() {
        let mut msg = Message::assistant("".to_string());
        msg.tool_calls = Some(
            vector![make_tool_call(
                "call_123",
                "get_weather",
                serde_json::json!({"city": "Tokyo"})
            )],
        );

        let result = message_to_chat_completion_msg(&msg);
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["role"], "assistant");

        let tool_calls = json["tool_calls"].as_array().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0]["type"], "function");
        assert_eq!(tool_calls[0]["id"], "call_123");
        assert_eq!(tool_calls[0]["function"]["name"], "get_weather");

        let args: serde_json::Value =
            serde_json::from_str(tool_calls[0]["function"]["arguments"].as_str().unwrap())
                .unwrap();
        assert_eq!(args["city"], "Tokyo");
    }

    #[test]
    fn test_chat_completion_tool_result() {
        let mut msg = Message::tool("get_weather".to_string(), "22°C".to_string());
        msg.id = Some("call_123".to_string());

        let result = message_to_chat_completion_msg(&msg);
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["role"], "tool");
        assert_eq!(json["content"], "22°C");
        assert_eq!(json["tool_call_id"], "call_123");
    }

    // =========================================================================
    // Responses API: messages_to_response_input
    // =========================================================================

    #[test]
    fn test_response_input_user_message() {
        let messages = vector![AgentValue::from(Message::user("Hello".to_string()))];
        let items = messages_to_response_input(&messages).unwrap();

        assert_eq!(items.len(), 1);
        let json = serde_json::to_value(&items[0]).unwrap();
        assert_eq!(json["role"], "user");
        assert_eq!(json["content"], "Hello");
    }

    #[test]
    fn test_response_input_assistant_without_tool_calls() {
        let messages = vector![AgentValue::from(Message::assistant("Hi there".to_string()))];
        let items = messages_to_response_input(&messages).unwrap();

        assert_eq!(items.len(), 1);
        let json = serde_json::to_value(&items[0]).unwrap();
        assert_eq!(json["role"], "assistant");
        assert_eq!(json["content"], "Hi there");
    }

    #[test]
    fn test_response_input_assistant_with_tool_calls() {
        let mut msg = Message::assistant("I'll check.".to_string());
        msg.tool_calls = Some(
            vector![make_tool_call(
                "call_456",
                "get_weather",
                serde_json::json!({"city": "NY"})
            )],
        );
        let messages = vector![AgentValue::from(msg)];
        let items = messages_to_response_input(&messages).unwrap();

        // Should have: 1 message item (text) + 1 function_call item
        assert_eq!(items.len(), 2);

        let msg_json = serde_json::to_value(&items[0]).unwrap();
        assert_eq!(msg_json["role"], "assistant");
        assert_eq!(msg_json["content"], "I'll check.");

        let fc_json = serde_json::to_value(&items[1]).unwrap();
        assert_eq!(fc_json["type"], "function_call");
        assert_eq!(fc_json["call_id"], "call_456");
        assert_eq!(fc_json["name"], "get_weather");
    }

    #[test]
    fn test_response_input_assistant_with_tool_calls_no_content() {
        let mut msg = Message::assistant("".to_string());
        msg.tool_calls = Some(
            vector![make_tool_call(
                "call_789",
                "search",
                serde_json::json!({"q": "test"})
            )],
        );
        let messages = vector![AgentValue::from(msg)];
        let items = messages_to_response_input(&messages).unwrap();

        // No text content → only function_call item, no message item
        assert_eq!(items.len(), 1);
        let fc_json = serde_json::to_value(&items[0]).unwrap();
        assert_eq!(fc_json["type"], "function_call");
        assert_eq!(fc_json["name"], "search");
    }

    #[test]
    fn test_response_input_tool_result() {
        let mut msg = Message::tool("get_weather".to_string(), "22°C".to_string());
        msg.id = Some("call_456".to_string());
        let messages = vector![AgentValue::from(msg)];
        let items = messages_to_response_input(&messages).unwrap();

        assert_eq!(items.len(), 1);
        let json = serde_json::to_value(&items[0]).unwrap();
        assert_eq!(json["type"], "function_call_output");
        assert_eq!(json["call_id"], "call_456");
        assert_eq!(json["output"], "22°C");
    }

    #[test]
    fn test_response_input_tool_result_no_id() {
        let msg = Message::tool("my_tool".to_string(), "result".to_string());
        let messages = vector![AgentValue::from(msg)];
        let items = messages_to_response_input(&messages).unwrap();

        let json = serde_json::to_value(&items[0]).unwrap();
        assert_eq!(json["type"], "function_call_output");
        // Should have a generated UUID, not empty
        let call_id = json["call_id"].as_str().unwrap();
        assert!(!call_id.is_empty());
    }

    #[test]
    fn test_response_input_full_round_trip() {
        // Simulate: user → assistant(tool_call) → tool_result
        let mut assistant_msg = Message::assistant("".to_string());
        assistant_msg.tool_calls = Some(
            vector![make_tool_call(
                "call_abc",
                "get_horoscope",
                serde_json::json!({"sign": "Virgo"})
            )],
        );

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

        let user_json = serde_json::to_value(&items[0]).unwrap();
        assert_eq!(user_json["role"], "user");

        let fc_json = serde_json::to_value(&items[1]).unwrap();
        assert_eq!(fc_json["type"], "function_call");
        assert_eq!(fc_json["name"], "get_horoscope");

        let fco_json = serde_json::to_value(&items[2]).unwrap();
        assert_eq!(fco_json["type"], "function_call_output");
        assert_eq!(fco_json["call_id"], "call_abc");
        assert_eq!(fco_json["output"], "Virgo: Good day!");
    }

    #[test]
    fn test_response_input_system_message() {
        let messages = vector![AgentValue::from(Message::system(
            "You are helpful.".to_string()
        ))];
        let items = messages_to_response_input(&messages).unwrap();

        let json = serde_json::to_value(&items[0]).unwrap();
        assert_eq!(json["role"], "developer");
    }
}
