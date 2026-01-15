#![cfg(feature = "openai")]

use std::sync::{Arc, Mutex};
use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    Message, ToolCall, ToolCallFunction, askit_agent, async_trait,
};
use async_openai::types::responses::{FunctionArgs, ToolDefinition};
use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCallChunk, ChatCompletionTool,
    ChatCompletionToolArgs, FunctionObjectArgs,
};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs,
        ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestToolMessageArgs,
        ChatCompletionRequestUserMessageArgs,
        ChatCompletionResponseMessage,
        CreateChatCompletionRequest,
        CreateChatCompletionRequestArgs,
        CreateCompletionRequest,
        CreateCompletionRequestArgs,
        CreateEmbeddingRequest,
        CreateEmbeddingRequestArgs,
        Role,
        // responses::{self, CreateResponse, CreateResponseArgs, OutputContent, OutputMessage},
    },
};
use futures::StreamExt;
use im::vector;

use crate::tool::{self, list_tool_infos_patterns};

const CATEGORY: &str = "LLM/OpenAI";

const PIN_EMBEDDINGS: &str = "embeddings";
const PIN_INPUT: &str = "input";
const PIN_MESSAGE: &str = "message";
const PIN_PROMPT: &str = "prompt";
const PIN_RESPONSE: &str = "response";

const CONFIG_MODEL: &str = "model";
const CONFIG_OPENAI_API_KEY: &str = "openai_api_key";
const CONFIG_OPENAI_API_BASE: &str = "openai_api_base";
const CONFIG_OPTIONS: &str = "options";
const CONFIG_STREAM: &str = "stream";
const CONFIG_SYSTEM: &str = "system";
const CONFIG_TOOLS: &str = "tools";

const DEFAULT_CONFIG_MODEL: &str = "gpt-5-nano";

// Shared client management for OpenAI agents
struct OpenAIManager {
    client: Arc<Mutex<Option<Client<OpenAIConfig>>>>,
}

impl OpenAIManager {
    fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }

    fn get_client(&self, askit: &ASKit) -> Result<Client<OpenAIConfig>, AgentError> {
        let mut client_guard = self.client.lock().unwrap();

        if let Some(client) = client_guard.as_ref() {
            return Ok(client.clone());
        }

        let mut config = OpenAIConfig::new();

        if let Some(api_key) = askit
            .get_global_configs(crate::openai::OpenAICompletionAgent::DEF_NAME)
            .and_then(|cfg| cfg.get_string(CONFIG_OPENAI_API_KEY).ok())
            .filter(|key| !key.is_empty())
        {
            config = config.with_api_key(&api_key);
        }

        if let Some(api_base) = askit
            .get_global_configs(crate::openai::OpenAICompletionAgent::DEF_NAME)
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

// OpenAI Completion Agent
#[askit_agent(
    title="Completion",
    category=CATEGORY,
    inputs=[PIN_PROMPT],
    outputs=[PIN_MESSAGE, PIN_RESPONSE],
    string_config(name=CONFIG_MODEL, default="gpt-3.5-turbo-instruct"),
    text_config(name=CONFIG_SYSTEM),
    object_config(name=CONFIG_OPTIONS),
    string_global_config(name=CONFIG_OPENAI_API_KEY, title="OpenAI API Key"),
    string_global_config(name=CONFIG_OPENAI_API_BASE, title="OpenAI API Base URL", default="https://api.openai.com/v1"),
)]
pub struct OpenAICompletionAgent {
    data: AgentData,
    manager: OpenAIManager,
}

#[async_trait]
impl AsAgent for OpenAICompletionAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            manager: OpenAIManager::new(),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config_model = &self.configs()?.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        let mut messages;
        {
            if value.is_array() {
                let arr = value.as_array().unwrap();
                messages = Vec::new();
                for item in arr {
                    let msg: Message = item.clone().try_into()?;
                    messages.push(msg);
                }
                // Just return if the last message is user
                if let Some(last_msg) = messages.last() {
                    if last_msg.role != "user" {
                        return Ok(());
                    }
                }
            } else {
                let message: Message = value.try_into()?;
                messages = vec![message];
            }
        }

        let mut request = CreateCompletionRequestArgs::default()
            .model(config_model)
            .prompt(
                messages
                    .iter()
                    .map(|m| m.content.clone())
                    .collect::<Vec<String>>(),
            )
            .build()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

        let config_options = self.configs()?.get_object_or_default(CONFIG_OPTIONS);
        if !config_options.is_empty() {
            // Merge options into request
            let options_json = serde_json::to_value(&config_options)
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
            request = serde_json::from_value::<CreateCompletionRequest>(request_json)
                .map_err(|e| AgentError::InvalidValue(format!("Deserialization error: {}", e)))?;
        }

        let client = self.manager.get_client(self.askit())?;
        let res = client
            .completions()
            .create(request)
            .await
            .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

        let message = Message::assistant(res.choices[0].text.clone());
        self.output(ctx.clone(), PIN_MESSAGE, message.into())
            .await?;

        let out_response = AgentValue::from_serialize(&res)?;
        self.output(ctx, PIN_RESPONSE, out_response).await?;

        Ok(())
    }
}

// OpenAI Chat Agent
#[askit_agent(
    title="Chat",
    category=CATEGORY,
    inputs=[PIN_MESSAGE],
    outputs=[PIN_MESSAGE, PIN_RESPONSE],
    boolean_config(name=CONFIG_STREAM, title="Stream"),
    string_config(name=CONFIG_MODEL, default=DEFAULT_CONFIG_MODEL),
    text_config(name=CONFIG_TOOLS),
    object_config(name=CONFIG_OPTIONS),
)]
pub struct OpenAIChatAgent {
    data: AgentData,
    manager: OpenAIManager,
}

#[async_trait]
impl AsAgent for OpenAIChatAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            manager: OpenAIManager::new(),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config_model = &self.configs()?.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        // Convert value to messages
        let Some(value) = value.to_message_value() else {
            return Err(AgentError::InvalidValue(
                "Input value is not a valid message".to_string(),
            ));
        };
        let messages = if value.is_array() {
            value.into_array().unwrap()
        } else {
            vector![value]
        };
        if messages.is_empty() {
            return Ok(());
        }

        // If the last message isn’t a user message, just return
        let role = &messages.last().unwrap().as_message().unwrap().role;
        if role != "user" && role != "tool" {
            return Ok(());
        }

        let config_options = self.configs()?.get_object_or_default(CONFIG_OPTIONS);
        let options_json =
            if !config_options.is_empty() {
                Some(serde_json::to_value(&config_options).map_err(|e| {
                    AgentError::InvalidValue(format!("Invalid JSON in options: {}", e))
                })?)
            } else {
                None
            };

        let config_tools = self.configs()?.get_string_or_default(CONFIG_TOOLS);
        let tool_infos = if config_tools.is_empty() {
            vec![]
        } else {
            list_tool_infos_patterns(&config_tools)
                .map_err(|e| {
                    AgentError::InvalidConfig(format!(
                        "Invalid regex patterns in tools config: {}",
                        e
                    ))
                })?
                .into_iter()
                .map(|tool| tool.try_into())
                .collect::<Result<Vec<ChatCompletionTool>, AgentError>>()?
        };

        let use_stream = self.configs()?.get_bool_or_default(CONFIG_STREAM);

        let client = self.manager.get_client(self.askit())?;

        let mut request = CreateChatCompletionRequestArgs::default()
            .model(config_model)
            .messages(
                messages
                    .iter()
                    .filter_map(|m| m.as_message())
                    .map(message_to_chat_completion_msg)
                    .collect::<Vec<ChatCompletionRequestMessage>>(),
            )
            .tools(tool_infos.clone())
            .stream(use_stream)
            // .stream_options(async_openai::types::ChatCompletionStreamOptions {
            //     include_usage: true,
            // })
            .build()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

        if let Some(options_json) = &options_json {
            // Merge options into request
            let mut request_json = serde_json::to_value(&request)
                .map_err(|e| AgentError::InvalidValue(format!("Serialization error: {}", e)))?;

            if let (Some(request_obj), Some(options_obj)) =
                (request_json.as_object_mut(), options_json.as_object())
            {
                for (key, value) in options_obj {
                    request_obj.insert(key.clone(), value.clone());
                }
            }
            request = serde_json::from_value::<CreateChatCompletionRequest>(request_json)
                .map_err(|e| AgentError::InvalidValue(format!("Deserialization error: {}", e)))?;
        }

        let id = uuid::Uuid::new_v4().to_string();
        if use_stream {
            let mut stream = client
                .chat()
                .create_stream(request)
                .await
                .map_err(|e| AgentError::IoError(format!("OpenAI Stream Error: {}", e)))?;

            let mut message = Message::assistant("".to_string());
            message.id = Some(id.clone());
            let mut content = String::new();
            let mut thinking = String::new();
            let mut tool_calls: Vec<ToolCall> = Vec::new();
            while let Some(res) = stream.next().await {
                let res = res.map_err(|_| AgentError::IoError(format!("OpenAI Stream Error")))?;

                for c in &res.choices {
                    if let Some(ref delta_content) = c.delta.content {
                        content.push_str(delta_content);
                    }
                    // FIXME: correct tool call chunks handling in streaming
                    if let Some(tc) = &c.delta.tool_calls {
                        for call in tc {
                            if let Ok(c) =
                                try_from_chat_completion_message_tool_call_chunk_to_tool_call(call)
                            {
                                tool_calls.push(c);
                            }
                        }
                    }
                    if let Some(refusal) = &c.delta.refusal {
                        thinking.push_str(&format!("Refusal: {}", refusal));
                    }
                }

                message.content = content.clone();
                if !thinking.is_empty() {
                    message.thinking = Some(thinking.clone());
                }
                if !tool_calls.is_empty() {
                    message.tool_calls = Some(tool_calls.clone().into());
                }

                self.output(ctx.clone(), PIN_MESSAGE, message.clone().into())
                    .await?;

                let out_response = AgentValue::from_serialize(&res)?;
                self.output(ctx.clone(), PIN_RESPONSE, out_response).await?;
            }

            return Ok(());
        } else {
            let res = client
                .chat()
                .create(request)
                .await
                .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

            for c in &res.choices {
                let mut message: Message = message_from_openai_msg(c.message.clone());
                message.id = Some(id.clone());

                self.output(ctx.clone(), PIN_MESSAGE, message.clone().into())
                    .await?;

                let out_response = AgentValue::from_serialize(&res)?;
                self.output(ctx.clone(), PIN_RESPONSE, out_response).await?;
            }

            return Ok(());
        }
    }
}

// OpenAI Embeddings Agent
#[askit_agent(
    title="Embeddings",
    category=CATEGORY,
    inputs=[PIN_INPUT],
    outputs=[PIN_EMBEDDINGS],
    string_config(name=CONFIG_MODEL, default="text-embedding-3-small"),
    object_config(name=CONFIG_OPTIONS)
)]
pub struct OpenAIEmbeddingsAgent {
    data: AgentData,
    manager: OpenAIManager,
}

#[async_trait]
impl AsAgent for OpenAIEmbeddingsAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            manager: OpenAIManager::new(),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config_model = &self.configs()?.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        let input = value.as_str().unwrap_or(""); // TODO: other types
        if input.is_empty() {
            return Ok(());
        }

        let client = self.manager.get_client(self.askit())?;
        let mut request = CreateEmbeddingRequestArgs::default()
            .model(config_model.to_string())
            .input(vec![input])
            .build()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

        let config_options = self.configs()?.get_object_or_default(CONFIG_OPTIONS);
        if !config_options.is_empty() {
            // Merge options into request
            let options_json = serde_json::to_value(&config_options)
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

        let embedding = AgentValue::tensor(res.data[0].embedding.clone());
        self.output(ctx.clone(), PIN_EMBEDDINGS, embedding).await?;

        Ok(())
    }
}

// // OpenAI Responses Agent
// // https://platform.openai.com/docs/api-reference/responses
// #[askit_agent(
//     title="Responses",
//     category=CATEGORY,
//     inputs=[PIN_MESSAGE],
//     outputs=[PIN_MESSAGE, PIN_RESPONSE],
//     boolean_config(name=CONFIG_STREAM),
//     string_config(name=CONFIG_MODEL, default=DEFAULT_CONFIG_MODEL),
//     text_config(name=CONFIG_TOOLS),
//     object_config(name=CONFIG_OPTIONS),
// )]
// pub struct OpenAIResponsesAgent {
//     data: AgentData,
//     manager: OpenAIManager,
// }

// #[async_trait]
// impl AsAgent for OpenAIResponsesAgent {
//     fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
//         Ok(Self {
//             data: AgentData::new(askit, id, spec),
//             manager: OpenAIManager::new(),
//         })
//     }

//     async fn process(
//         &mut self,
//         ctx: AgentContext,
//         pin: String,
//         value: AgentValue,
//     ) -> Result<(), AgentError> {
//         let config_model = &self.configs()?.get_string_or_default(CONFIG_MODEL);
//         if config_model.is_empty() {
//             return Ok(());
//         }

//         // Convert value to messages
//         let Some(value) = value.to_message_value() else {
//             return Err(AgentError::InvalidValue(
//                 "Input value is not a valid message".to_string(),
//             ));
//         };
//         let messages = if value.is_array() {
//             value.into_array().unwrap()
//         } else {
//             vector![value]
//         };
//         if messages.is_empty() {
//             return Ok(());
//         }

//         // If the last message isn’t a user message, just return
//         let role = &messages.last().unwrap().as_message().unwrap().role;
//         if role != "user" && role != "tool" {
//             return Ok(());
//         }

//         let config_options = self.configs()?.get_object_or_default(CONFIG_OPTIONS);
//         let options_json =
//             if !config_options.is_empty() {
//                 Some(serde_json::to_value(&config_options).map_err(|e| {
//                     AgentError::InvalidValue(format!("Invalid JSON in options: {}", e))
//                 })?)
//             } else {
//                 None
//             };

//         let config_tools = self.configs()?.get_string_or_default(CONFIG_TOOLS);
//         let tool_infos = if config_tools.is_empty() {
//             vec![]
//         } else {
//             list_tool_infos_patterns(&config_tools)
//                 .map_err(|e| {
//                     AgentError::InvalidConfig(format!(
//                         "Invalid regex patterns in tools config: {}",
//                         e
//                     ))
//                 })?
//                 .into_iter()
//                 .map(|tool| tool.try_into())
//                 .collect::<Result<Vec<ToolDefinition>, AgentError>>()?
//         };

//         let use_stream = self.configs()?.get_bool_or_default(CONFIG_STREAM);

//         let client = self.manager.get_client(self.askit())?;

//         let mut request = CreateResponseArgs::default()
//             .model(config_model)
//             .input(responses::Input::Items(
//                 messages
//                     .iter()
//                     .filter_map(|m| m.as_message())
//                     .map(message_to_response_input_item)
//                     .collect::<Vec<responses::InputItem>>(),
//             ))
//             .tools(tool_infos.clone())
//             .stream(use_stream)
//             .build()
//             .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

//         if let Some(options_json) = &options_json {
//             // Merge options into request
//             let mut request_json = serde_json::to_value(&request)
//                 .map_err(|e| AgentError::InvalidValue(format!("Serialization error: {}", e)))?;

//             if let (Some(request_obj), Some(options_obj)) =
//                 (request_json.as_object_mut(), options_json.as_object())
//             {
//                 for (key, value) in options_obj {
//                     request_obj.insert(key.clone(), value.clone());
//                 }
//             }
//             request = serde_json::from_value::<CreateResponse>(request_json)
//                 .map_err(|e| AgentError::InvalidValue(format!("Deserialization error: {}", e)))?;
//         }

//         let id = uuid::Uuid::new_v4().to_string();
//         if use_stream {
//             let mut stream = client
//                 .responses()
//                 .create_stream(request)
//                 .await
//                 .map_err(|e| AgentError::IoError(format!("OpenAI Stream Error: {}", e)))?;

//             let mut message = Message::assistant("".to_string());
//             message.id = Some(id.clone());
//             let mut content = String::new();
//             let mut tool_calls: Vec<ToolCall> = Vec::new();
//             while let Some(res) = stream.next().await {
//                 let res_event =
//                     res.map_err(|e| AgentError::IoError(format!("OpenAI Stream Error: {}", e)))?;

//                 match &res_event {
//                     responses::ResponseEvent::ResponseOutputTextDelta(delta) => {
//                         content.push_str(&delta.delta);
//                     }
//                     responses::ResponseEvent::ResponseFunctionCallArgumentsDone(fc) => {
//                         if let Ok(parameters) =
//                             serde_json::from_str::<serde_json::Value>(&fc.arguments)
//                         {
//                             let call = ToolCall {
//                                 function: ToolCallFunction {
//                                     id: Some(fc.item_id.clone()),
//                                     name: fc.name.clone(),
//                                     parameters,
//                                 },
//                             };
//                             tool_calls.push(call);
//                         }
//                     }
//                     responses::ResponseEvent::ResponseCompleted(_) => {
//                         let out_response = AgentValue::from_serialize(&res_event)?;
//                         self.output(ctx.clone(), PIN_RESPONSE, out_response).await?;
//                         break;
//                     }
//                     _ => {}
//                 }

//                 message.content = content.clone();
//                 if !tool_calls.is_empty() {
//                     message.tool_calls = Some(tool_calls.clone().into());
//                 }

//                 self.output(ctx.clone(), PIN_MESSAGE, message.clone().into()).await?;

//                 let out_response = AgentValue::from_serialize(&res_event)?;
//                 self.output(ctx.clone(), PIN_RESPONSE, out_response).await?;
//             }

//             return Ok(());
//         } else {
//             let res = client
//                 .responses()
//                 .create(request)
//                 .await
//                 .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

//             // TODO: support tool calls
//             let mut res_message: Message = Message::assistant(get_output_text(&res)); // TODO: better conversion
//             res_message.id = Some(res.id.clone());

//             self.output(ctx.clone(), PIN_MESSAGE, res_message.clone().into()).await?;

//             let out_response = AgentValue::from_serialize(&res)?;
//             self.output(ctx.clone(), PIN_RESPONSE, out_response).await?;

//             return Ok(());
//         }
//     }
// }

// fn get_output_text(response: &responses::Response) -> String {
//     let mut output_text = String::new();
//     response.output.iter().for_each(|msg| {
//         if let responses::OutputContent::Message(m) = msg {
//             m.content.iter().for_each(|c| {
//                 if let responses::Content::OutputText(t) = c {
//                     output_text.push_str(&t.text);
//                 }
//             });
//         }
//     });
//     output_text
// }

fn message_from_openai_msg(msg: ChatCompletionResponseMessage) -> Message {
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
            if let Ok(c) = try_from_chat_completion_message_tool_call_to_tool_call(&call) {
                calls.push(c);
            }
        }
        if !calls.is_empty() {
            message.tool_calls = Some(calls.into());
        }
    }

    message
}

fn message_to_chat_completion_msg(msg: &Message) -> ChatCompletionRequestMessage {
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
                    use async_openai::types::{
                        ChatCompletionRequestMessageContentPartImage,
                        ChatCompletionRequestMessageContentPartText, ImageUrl,
                    };

                    let image_url = ImageUrl {
                        url: image.get_base64(),
                        detail: Some(async_openai::types::ImageDetail::Auto),
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

// fn message_to_response_input_item(msg: &Message) -> responses::InputItem {
//     responses::InputItem::Message(responses::InputMessage {
//         kind: responses::InputMessageType::Message,
//         role: match msg.role.as_str() {
//             "system" => responses::Role::System,
//             "user" => responses::Role::User,
//             "assistant" => responses::Role::Assistant,
//             "developer" => responses::Role::Developer,
//             _ => responses::Role::Developer,
//         },
//         content: responses::InputContent::TextInput(msg.content.clone()),
//     })
// }

// fn output_content_to_message(content: OutputContent) -> Message {
//     match content {
//         OutputContent::Message(msg) => output_message_to_message(msg),
//         _ => Message::new("unknown".to_string(), "".to_string()),
//     }
// }

// fn output_message_to_message(msg: OutputMessage) -> Message {
//     let role = match msg.role {
//         responses::Role::System => "system",
//         responses::Role::User => "user",
//         responses::Role::Assistant => "assistant",
//         responses::Role::Developer => "developer",
//     };
//     let content = msg
//         .content
//         .into_iter()
//         .map(|c| match c {
//             responses::Content::OutputText(t) => t.text,
//             responses::Content::Refusal(r) => format!("Refusal: {}", r.refusal),
//         })
//         .collect::<Vec<String>>()
//         .join(" ");
//     let mut message = Message::new(role.to_string(), content);
//     message.id = Some(msg.id);
//     message
// }

impl TryFrom<tool::ToolInfo> for ChatCompletionTool {
    type Error = AgentError;

    fn try_from(info: tool::ToolInfo) -> Result<Self, Self::Error> {
        let mut function = FunctionObjectArgs::default();
        function.name(info.name);
        if !info.description.is_empty() {
            function.description(info.description);
        }
        if let Some(params) = info.parameters {
            // function.parameters(serde_json::to_value(params).map_err(|e| {
            //     AgentError::InvalidValue(format!("Failed to serialize tool parameters: {}", e))
            // })?);
            function.parameters(params);
        }
        Ok(ChatCompletionToolArgs::default()
            .function(function.build().map_err(|e| {
                AgentError::InvalidValue(format!("Failed to build tool function: {}", e))
            })?)
            .build()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to build tool: {}", e)))?)
    }
}

impl TryFrom<tool::ToolInfo> for ToolDefinition {
    type Error = AgentError;

    fn try_from(info: tool::ToolInfo) -> Result<Self, Self::Error> {
        let mut function = FunctionArgs::default();
        function.name(info.name);
        if !info.description.is_empty() {
            function.description(info.description);
        }
        if let Some(params) = info.parameters {
            // function.parameters(serde_json::to_value(params).map_err(|e| {
            //     AgentError::InvalidValue(format!("Failed to serialize tool parameters: {}", e))
            // })?);
            function.parameters(params);
        }
        Ok(ToolDefinition::Function(function.build().map_err(|e| {
            AgentError::InvalidValue(format!("Failed to build tool function: {}", e))
        })?))
    }
}

fn try_from_chat_completion_message_tool_call_chunk_to_tool_call(
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
