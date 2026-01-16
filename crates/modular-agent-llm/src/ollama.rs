#![cfg(feature = "ollama")]

use std::sync::{Arc, Mutex};
use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec,
    AgentValue, AsAgent, Message, ToolCall, ToolCallFunction, askit_agent, async_trait,
};

use im::{Vector, vector};
use ollama_rs::generation::completion::GenerationContext;
use ollama_rs::generation::embeddings::request::EmbeddingsInput;
use ollama_rs::{
    Ollama,
    generation::{
        chat::{ChatMessage, MessageRole, request::ChatMessageRequest},
        completion::request::GenerationRequest,
        embeddings::request::GenerateEmbeddingsRequest,
    },
    models::ModelOptions,
};
use schemars::{Schema, json_schema};
use tokio_stream::StreamExt;

use crate::tool::{self, list_tool_infos_patterns};

const CATEGORY: &str = "LLM/Ollama";

const PIN_DOC: &str = "doc";
const PIN_EMBEDDING: &str = "embedding";
const PIN_MESSAGE: &str = "message";
const PIN_MODEL_INFO: &str = "model_info";
const PIN_MODEL_LIST: &str = "model_list";
const PIN_MODEL_NAME: &str = "model_name";
const PIN_PROMPT: &str = "prompt";
const PIN_RESET: &str = "reset";
const PIN_RESPONSE: &str = "response";
const PIN_STRING: &str = "string";
const PIN_UNIT: &str = "unit";

const CONFIG_MODEL: &str = "model";
const CONFIG_OLLAMA_URL: &str = "ollama_url";
const CONFIG_OPTIONS: &str = "options";
const CONFIG_STREAM: &str = "stream";
const CONFIG_SYSTEM: &str = "system";
const CONFIG_TOOLS: &str = "tools";
const CONFIG_USE_CONTEXT: &str = "use_context";

const DEFAULT_CONFIG_MODEL: &str = "gpt-oss:20b";
const DEFAULT_CONFIG_EMBEDDINGS_MODEL: &str = "nomic-embed-text-v2-moe:latest";
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

// Shared client management for Ollama agents
struct OllamaManager {
    client: Arc<Mutex<Option<Ollama>>>,
}

impl OllamaManager {
    fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }

    fn get_ollama_url(global_config: Option<AgentConfigs>) -> String {
        if let Some(ollama_url) =
            global_config.and_then(|cfg| cfg.get_string(CONFIG_OLLAMA_URL).ok())
        {
            if !ollama_url.is_empty() {
                return ollama_url;
            }
        }
        if let Ok(ollama_api_base_url) = std::env::var("OLLAMA_API_BASE_URL") {
            return ollama_api_base_url;
        } else if let Ok(ollama_host) = std::env::var("OLLAMA_HOST") {
            return format!("http://{}:11434", ollama_host);
        }
        DEFAULT_OLLAMA_URL.to_string()
    }

    fn get_client(&self, askit: &ASKit) -> Result<Ollama, AgentError> {
        let mut client_guard = self.client.lock().unwrap();

        if let Some(client) = client_guard.as_ref() {
            return Ok(client.clone());
        }

        let global_config =
            askit.get_global_configs(crate::ollama::OllamaCompletionAgent::DEF_NAME);
        let api_base_url = Self::get_ollama_url(global_config);
        let new_client = Ollama::try_new(api_base_url)
            .map_err(|e| AgentError::IoError(format!("Ollama Client Error: {}", e)))?;
        *client_guard = Some(new_client.clone());

        Ok(new_client)
    }
}

// Ollama Completion Agent
#[askit_agent(
    title="Completion",
    category=CATEGORY,
    inputs=[PIN_PROMPT, PIN_RESET],
    outputs=[PIN_MESSAGE, PIN_RESPONSE],
    string_config(name=CONFIG_MODEL, default=DEFAULT_CONFIG_MODEL),
    text_config(name=CONFIG_SYSTEM, default=""),
    boolean_config(name=CONFIG_USE_CONTEXT),
    object_config(name=CONFIG_OPTIONS),
    string_global_config(name=CONFIG_OLLAMA_URL, default=DEFAULT_OLLAMA_URL, title="Ollama URL"),
)]
pub struct OllamaCompletionAgent {
    data: AgentData,
    manager: OllamaManager,
    context: Option<GenerationContext>,
}

#[async_trait]
impl AsAgent for OllamaCompletionAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            manager: OllamaManager::new(),
            context: None,
        })
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        self.context = None;
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if pin == PIN_RESET {
            self.context = None;
            return Ok(());
        }

        let config_model = &self.configs()?.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        let prompt = value.as_str().unwrap_or("");
        if prompt.is_empty() {
            return Ok(());
        }

        let mut request = GenerationRequest::new(config_model.to_string(), prompt);

        let config_system = self.configs()?.get_string_or_default(CONFIG_SYSTEM);
        if !config_system.is_empty() {
            request = request.system(config_system);
        }

        let config_options = self.configs()?.get_object_or_default(CONFIG_OPTIONS);
        if !config_options.is_empty() {
            let config_options = serde_json::to_value(&config_options)
                .map_err(|e| AgentError::InvalidValue(format!("Invalid JSON in options: {}", e)))?;
            if let Ok(options_json) = serde_json::from_value::<ModelOptions>(config_options) {
                request = request.options(options_json);
            } else {
                return Err(AgentError::InvalidValue(
                    "Invalid JSON in options".to_string(),
                ));
            }
        }

        let use_context = self.configs()?.get_bool_or_default(CONFIG_USE_CONTEXT);
        if use_context {
            if let Some(context) = &self.context {
                request = request.context(context.clone());
            }
        }

        let client = self.manager.get_client(self.askit())?;
        let res = client
            .generate(request)
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

        if use_context {
            self.context = res.context.clone().or(self.context.clone());
        }

        let message = Message::assistant(res.response.clone());
        self.output(ctx.clone(), PIN_MESSAGE, message.into())
            .await?;

        let out_response = AgentValue::from_serialize(&res)?;
        self.output(ctx, PIN_RESPONSE, out_response).await?;

        Ok(())
    }
}

// Ollama Chat Agent
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
pub struct OllamaChatAgent {
    data: AgentData,
    manager: OllamaManager,
}

#[async_trait]
impl AsAgent for OllamaChatAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            manager: OllamaManager::new(),
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

        // If the last message isn’t a user/tool message, just return
        let role = &messages.last().unwrap().as_message().unwrap().role;
        if role != "user" && role != "tool" {
            return Ok(());
        }

        let config_options = self.configs()?.get_object_or_default(CONFIG_OPTIONS);
        let options_json = if !config_options.is_empty() {
            let value = serde_json::to_value(&config_options).map_err(|e| {
                AgentError::InvalidConfig(format!("Invalid JSON in options: {}", e))
            })?;
            Some(serde_json::from_value::<ModelOptions>(value).map_err(|e| {
                AgentError::InvalidConfig(format!("Invalid JSON in options: {}", e))
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
                .map(|tool| tool.into())
                .collect::<Vec<ollama_rs::generation::tools::ToolInfo>>()
        };

        let use_stream = self.configs()?.get_bool_or_default(CONFIG_STREAM);

        let client = self.manager.get_client(self.askit())?;

        let mut request = ChatMessageRequest::new(
            config_model.to_string(),
            messages
                .iter()
                .cloned()
                .map(|m| message_to_chat(m.as_message().unwrap().clone()))
                .collect(),
        );

        if options_json.is_some() {
            request = request.options(options_json.clone().unwrap());
        }

        if !tool_infos.is_empty() {
            request = request.tools(tool_infos.clone());
        }

        let id = uuid::Uuid::new_v4().to_string();
        if use_stream {
            let mut stream = client
                .send_chat_messages_stream(request)
                .await
                .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

            let mut message = Message::assistant("".to_string());
            let mut content = String::new();
            let mut thinking = String::new();
            let mut tool_calls: Vec<ToolCall> = vec![];
            while let Some(res) = stream.next().await {
                let res = res.map_err(|_| AgentError::IoError(format!("Ollama Stream Error")))?;

                content.push_str(&res.message.content);
                if let Some(thinking_str) = res.message.thinking.as_ref() {
                    thinking.push_str(thinking_str);
                }
                for call in &res.message.tool_calls {
                    let mut parameters = call.function.arguments.clone();
                    if parameters.is_object() {
                        if let Some(obj) = parameters.as_object() {
                            if let Some(props) = obj.get("properties") {
                                parameters = props.clone();
                            }
                        }
                    }

                    let tool_call = ToolCall {
                        function: ToolCallFunction {
                            id: None,
                            name: call.function.name.clone(),
                            parameters,
                        },
                    };
                    tool_calls.push(tool_call);
                }

                message.content = content.clone();
                if !thinking.is_empty() {
                    message.thinking = Some(thinking.clone());
                }
                if tool_calls.len() > 0 {
                    message.tool_calls = Some(tool_calls.clone().into());
                }
                message.id = Some(id.clone());

                self.output(ctx.clone(), PIN_MESSAGE, message.clone().into())
                    .await?;

                let out_response = AgentValue::from_serialize(&res)?;
                self.output(ctx.clone(), PIN_RESPONSE, out_response).await?;

                if res.done {
                    break;
                }
            }

            return Ok(());
        } else {
            let res = client
                .send_chat_messages(request)
                .await
                .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

            let mut message: Message = message_from_ollama(res.message.clone());
            message.id = Some(id.clone());

            self.output(ctx.clone(), PIN_MESSAGE, message.clone().into())
                .await?;

            let out_response = AgentValue::from_serialize(&res)?;
            self.output(ctx.clone(), PIN_RESPONSE, out_response).await?;

            return Ok(());
        }
    }
}

#[askit_agent(
    title="Embeddings",
    category=CATEGORY,
    inputs=[PIN_STRING, PIN_DOC],
    outputs=[PIN_EMBEDDING, PIN_DOC],
    string_config(name=CONFIG_MODEL, default=DEFAULT_CONFIG_EMBEDDINGS_MODEL),
    text_config(name=CONFIG_OPTIONS, default="{}")
)]
pub struct OllamaEmbeddingsAgent {
    data: AgentData,
    manager: OllamaManager,
}

impl OllamaEmbeddingsAgent {
    async fn generate_embeddings(
        &self,
        input: EmbeddingsInput,
        model_name: String,
        model_options: Option<ModelOptions>,
    ) -> Result<Vec<Vec<f32>>, AgentError> {
        let client = self.manager.get_client(self.askit())?;
        let mut request = GenerateEmbeddingsRequest::new(model_name, input);
        if let Some(options) = model_options {
            request = request.options(options);
        }
        let res = client
            .generate_embeddings(request)
            .await
            .map_err(|e| AgentError::IoError(format!("generate_embeddings: {}", e)))?;
        Ok(res.embeddings)
    }
}

#[async_trait]
impl AsAgent for OllamaEmbeddingsAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            manager: OllamaManager::new(),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config_model = &self.configs()?.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Err(AgentError::InvalidConfig("model is not set".to_string()));
        }

        let config_options = self.configs()?.get_string_or_default(CONFIG_OPTIONS);
        let model_options = if config_options.is_empty() || config_options == "{}" {
            None
        } else {
            Some(
                serde_json::from_str::<ModelOptions>(&config_options).map_err(|e| {
                    AgentError::InvalidConfig(format!("Invalid JSON in options: {}", e))
                })?,
            )
        };

        if pin == PIN_STRING {
            let text = value.as_str().unwrap_or_default();
            if text.is_empty() {
                return Err(AgentError::InvalidValue(
                    "Input text is an empty string".to_string(),
                ));
            }
            let input: EmbeddingsInput = text.into();
            let embeddings = self
                .generate_embeddings(input, config_model.to_string(), model_options)
                .await?;
            if embeddings.len() != 1 {
                return Err(AgentError::Other(
                    "Expected exactly one embedding for single string input".to_string(),
                ));
            }
            return self
                .output(
                    ctx.clone(),
                    PIN_EMBEDDING,
                    AgentValue::tensor(embeddings.into_iter().next().unwrap()),
                )
                .await;
        }

        if pin == PIN_DOC {
            if !value.is_object() {
                return Err(AgentError::InvalidValue(
                    "Input must be an object with 'text' or 'chunks' field".to_string(),
                ));
            }

            if let Some(chunks) = value.get_array("chunks") {
                // input is chunks

                if chunks.is_empty() {
                    return self.output(ctx.clone(), PIN_DOC, value).await;
                }

                let first_chunk = &chunks[0];
                if !first_chunk.is_array() {
                    return Err(AgentError::InvalidValue(
                        "Input chunks must be (offset, string) pairs".to_string(),
                    ));
                }
                let first_chunk_arr = first_chunk.as_array().unwrap();
                if first_chunk_arr.is_empty()
                    || !first_chunk_arr[0].is_integer()
                    || !first_chunk_arr[1].is_string()
                {
                    return Err(AgentError::InvalidValue(
                        "Input chunks must be (offset, string) pairs".to_string(),
                    ));
                }

                let mut offsets = vec![];
                let mut inputs = vec![];
                for item in chunks {
                    let Some(chunk) = item.as_array() else {
                        continue;
                    };
                    if chunk.len() != 2 {
                        continue;
                    }
                    let Some(offset) = chunk[0].as_i64() else {
                        continue;
                    };
                    let Some(s) = chunk[1].as_str() else {
                        continue;
                    };
                    if !s.is_empty() {
                        offsets.push(offset);
                        inputs.push(s.to_string());
                    }
                }
                if inputs.is_empty() {
                    return self.output(ctx.clone(), PIN_DOC, value).await;
                }

                let embeddings = self
                    .generate_embeddings(inputs.into(), config_model.to_string(), model_options)
                    .await?;
                let embedding_values_with_offsets: Vector<AgentValue> = offsets
                    .into_iter()
                    .zip(embeddings)
                    .map(|(offset, emb)| {
                        AgentValue::array(vector![
                            AgentValue::integer(offset),
                            AgentValue::tensor(emb)
                        ])
                    })
                    .collect();

                let mut output = value.clone();
                output.set(
                    "embeddings".to_string(),
                    AgentValue::array(embedding_values_with_offsets),
                )?;
                return self.output(ctx.clone(), PIN_DOC, output).await;
            }

            let text = value.get_str("text").unwrap_or("");
            if text.is_empty() {
                return self.output(ctx.clone(), PIN_DOC, value).await;
            }
            let input: EmbeddingsInput = text.into();
            let embeddings = self
                .generate_embeddings(input, config_model.to_string(), model_options)
                .await?;
            if embeddings.is_empty() {
                return self.output(ctx.clone(), PIN_DOC, value).await;
            }
            let offsets = vec![0i64];
            let embedding_values_with_offsets: Vector<AgentValue> = offsets
                .into_iter()
                .zip(embeddings)
                .map(|(offset, emb)| {
                    AgentValue::array(vector![
                        AgentValue::integer(offset),
                        AgentValue::tensor(emb)
                    ])
                })
                .collect();
            let mut output = value.clone();
            output.set(
                "embeddings".to_string(),
                AgentValue::array(embedding_values_with_offsets),
            )?;
            return self.output(ctx.clone(), PIN_DOC, output).await;
        }

        Err(AgentError::InvalidPin(pin))
    }
}

// Ollama List Local Models
#[askit_agent(
    title="List Local Models",
    category=CATEGORY,
    inputs=[PIN_UNIT],
    outputs=[PIN_MODEL_LIST],
)]
pub struct OllamaListLocalModelsAgent {
    data: AgentData,
    manager: OllamaManager,
}

#[async_trait]
impl AsAgent for OllamaListLocalModelsAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            manager: OllamaManager::new(),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let client = self.manager.get_client(self.askit())?;
        let model_list = client
            .list_local_models()
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;
        let model_list = AgentValue::from_serialize(&model_list)?;

        self.output(ctx.clone(), PIN_MODEL_LIST, model_list).await?;
        Ok(())
    }
}

// Ollama Show Model Info
#[askit_agent(
    title="Show Model Info",
    category=CATEGORY,
    inputs=[PIN_MODEL_NAME],
    outputs=[PIN_MODEL_INFO],
)]
pub struct OllamaShowModelInfoAgent {
    data: AgentData,
    manager: OllamaManager,
}

#[async_trait]
impl AsAgent for OllamaShowModelInfoAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            manager: OllamaManager::new(),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let model_name = value.as_str().unwrap_or(""); // TODO: other types
        if model_name.is_empty() {
            return Ok(());
        }

        let client = self.manager.get_client(self.askit())?;
        let model_info = client
            .show_model_info(model_name.to_string())
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;
        let model_info = AgentValue::from_serialize(&model_info)?;

        self.output(ctx.clone(), PIN_MODEL_INFO, model_info).await?;
        Ok(())
    }
}

fn message_from_ollama(msg: ChatMessage) -> Message {
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

    // #[cfg(feature = "image")]
    // {
    //     if let Some(images) = msg.images {
    //         if !images.is_empty() {
    //             let img = images[0].clone();
    //             let img_base64 = format!("data:image/png;base64,{}", img.to_base64());
    //             message.image = Some(crate::message::ImageData::from_base64(img_base64));
    //         }
    //     }
    // }
}

fn message_to_chat(msg: Message) -> ChatMessage {
    let mut cmsg = match msg.role.as_str() {
        "user" => ChatMessage::user(msg.content),
        "assistant" => ChatMessage::assistant(msg.content),
        "system" => ChatMessage::system(msg.content),
        "tool" => ChatMessage::tool(msg.content),
        _ => ChatMessage::user(msg.content), // Default to user if unknown role
    };
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

impl From<tool::ToolInfo> for ollama_rs::generation::tools::ToolInfo {
    fn from(info: tool::ToolInfo) -> Self {
        let schema: Schema = if let Some(params) = info.parameters {
            Schema::try_from(params).unwrap_or_else(|_| json_schema!({}))
        } else {
            json_schema!({})
        };
        Self {
            tool_type: ollama_rs::generation::tools::ToolType::Function,
            function: ollama_rs::generation::tools::ToolFunctionInfo {
                name: info.name,
                description: info.description,
                parameters: schema,
            },
        }
    }
}
