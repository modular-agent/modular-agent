#![cfg(feature = "ollama")]

use std::sync::{Arc, Mutex};
use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec,
    AgentValue, AsAgent, askit_agent, async_trait,
};

use ollama_rs::{
    Ollama,
    generation::{
        chat::{ChatMessage, MessageRole, request::ChatMessageRequest},
        completion::request::GenerationRequest,
        embeddings::request::GenerateEmbeddingsRequest,
    },
    history::ChatHistory,
    models::ModelOptions,
};
use schemars::{Schema, json_schema, schema_for_value};
use tokio_stream::StreamExt;

use crate::message::{Message, MessageHistory, ToolCall};
use crate::tool::{self, list_tool_infos, list_tool_infos_regex};

static CATEGORY: &str = "LLM/Ollama";

static PIN_EMBEDDINGS: &str = "embeddings";
static PIN_HISTORY: &str = "history";
static PIN_INPUT: &str = "input";
static PIN_MESSAGE: &str = "message";
static PIN_MODEL_INFO: &str = "model_info";
static PIN_MODEL_LIST: &str = "model_list";
static PIN_MODEL_NAME: &str = "model_name";
static PIN_RESET: &str = "reset";
static PIN_RESPONSE: &str = "response";
static PIN_UNIT: &str = "unit";

static CONFIG_MODEL: &str = "model";
static CONFIG_OLLAMA_URL: &str = "ollama_url";
static CONFIG_OPTIONS: &str = "options";
static CONFIG_STREAM: &str = "stream";
static CONFIG_SYSTEM: &str = "system";
static CONFIG_TOOLS: &str = "tools";

const DEFAULT_CONFIG_MODEL: &str = "gpt-oss:20b";
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

        let global_config = askit.get_global_configs("ollama_completion");
        let api_base_url = Self::get_ollama_url(global_config);
        let new_client = Ollama::try_new(api_base_url)
            .map_err(|e| AgentError::IoError(format!("Ollama Client Error: {}", e)))?;
        *client_guard = Some(new_client.clone());

        Ok(new_client)
    }
}

// Ollama Completion Agent
#[askit_agent(
    title="Ollama Completion",
    category=CATEGORY,
    inputs=[PIN_MESSAGE],
    outputs=[PIN_MESSAGE, PIN_RESPONSE],
    string_config(name=CONFIG_MODEL, default=DEFAULT_CONFIG_MODEL),
    text_config(name=CONFIG_SYSTEM, default=""),
    text_config(name=CONFIG_OPTIONS, default="{}"),
    string_global_config(name=CONFIG_OLLAMA_URL, default=DEFAULT_OLLAMA_URL, title="Ollama URL"),
)]
pub struct OllamaCompletionAgent {
    data: AgentData,
    manager: OllamaManager,
}

#[async_trait]
impl AsAgent for OllamaCompletionAgent {
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

        let message = value.as_str().unwrap_or("");
        if message.is_empty() {
            return Ok(());
        }

        let mut request = GenerationRequest::new(config_model.to_string(), message);

        let config_system = self.configs()?.get_string_or_default(CONFIG_SYSTEM);
        if !config_system.is_empty() {
            request = request.system(config_system);
        }

        let config_options = self.configs()?.get_string_or_default(CONFIG_OPTIONS);
        if !config_options.is_empty() && config_options != "{}" {
            if let Ok(options_json) = serde_json::from_str::<ModelOptions>(&config_options) {
                request = request.options(options_json);
            } else {
                return Err(AgentError::InvalidValue(
                    "Invalid JSON in options".to_string(),
                ));
            }
        }

        let client = self.manager.get_client(self.askit())?;
        let res = client
            .generate(request)
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

        let message = Message::assistant(res.response.clone());
        self.try_output(ctx.clone(), PIN_MESSAGE, message.into())?;

        let out_response = AgentValue::from_serialize(&res)?;
        self.try_output(ctx, PIN_RESPONSE, out_response)?;

        Ok(())
    }
}

// Ollama Chat Agent
#[askit_agent(
    title="Ollama Chat",
    category=CATEGORY,
    inputs=[PIN_MESSAGE, PIN_RESET],
    outputs=[PIN_MESSAGE, PIN_HISTORY, PIN_RESPONSE],
    string_config(name=CONFIG_MODEL, default=DEFAULT_CONFIG_MODEL),
    boolean_config(name=CONFIG_STREAM, title="Stream"),
    string_config(name=CONFIG_TOOLS, default=""),
    text_config(name=CONFIG_OPTIONS, default="{}")
)]
pub struct OllamaChatAgent {
    data: AgentData,
    manager: OllamaManager,
    history: MessageHistory,
}

impl OllamaChatAgent {
    async fn call_tools(
        &mut self,
        ctx: AgentContext,
        tool_calls: &Vec<ToolCall>,
    ) -> Result<(), AgentError> {
        let resp_messages = tool::call_tools(&ctx, tool_calls).await?;
        self.history.push_all(resp_messages);
        Ok(())
    }
}

#[async_trait]
impl AsAgent for OllamaChatAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            manager: OllamaManager::new(),
            history: Default::default(),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if pin == PIN_RESET {
            self.history = MessageHistory::default();
            self.try_output(ctx, PIN_HISTORY, self.history.clone().into())?;
            return Ok(());
        }

        let config_model = &self.configs()?.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        let messages = MessageHistory::from_value(value)?.messages();
        if messages.is_empty() {
            return Ok(());
        }

        for message in messages {
            self.try_output(ctx.clone(), PIN_MESSAGE, message.clone().into())?;
            self.history.push(message);
        }
        self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;

        if self.history.messages().last().unwrap().role != "user" {
            // If the last message isn’t a user message, just return
            return Ok(());
        }

        let config_options = self.configs()?.get_string_or_default(CONFIG_OPTIONS);
        let options_json = if !config_options.is_empty() && config_options != "{}" {
            Some(
                serde_json::from_str::<ModelOptions>(&config_options).map_err(|e| {
                    AgentError::InvalidValue(format!("Invalid JSON in options: {}", e))
                })?,
            )
        } else {
            None
        };

        let config_tools = self.configs()?.get_string_or_default(CONFIG_TOOLS);
        let tool_infos = if config_tools.is_empty() {
            list_tool_infos()
        } else {
            let regex = regex::Regex::new(&config_tools).map_err(|e| {
                AgentError::InvalidValue(format!("Invalid regex in tools config: {}", e))
            })?;
            list_tool_infos_regex(&regex)
        }
        .into_iter()
        .map(|tool| tool.into())
        .collect::<Vec<ollama_rs::generation::tools::ToolInfo>>();

        let use_stream = self.configs()?.get_bool_or_default(CONFIG_STREAM);

        let client = self.manager.get_client(self.askit())?;

        loop {
            let mut request = ChatMessageRequest::new(
                config_model.to_string(),
                self.history
                    .messages_for_prompt()
                    .into_iter()
                    .map(|m| m.into())
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
                let mut tool_calls: Vec<crate::message::ToolCall> = vec![];
                while let Some(res) = stream.next().await {
                    let res =
                        res.map_err(|_| AgentError::IoError(format!("Ollama Stream Error")))?;

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

                        let tool_call = crate::message::ToolCall {
                            function: crate::message::ToolCallFunction {
                                id: None,
                                name: call.function.name.clone(),
                                parameters,
                            },
                        };
                        tool_calls.push(tool_call);
                    }

                    message = Message::assistant(content.clone());
                    message.thinking = thinking.clone();
                    if tool_calls.len() > 0 {
                        message.tool_calls = Some(tool_calls.clone());
                    }
                    message.id = Some(id.clone());

                    self.history.push(message.clone());

                    self.try_output(ctx.clone(), PIN_MESSAGE, message.clone().into())?;

                    let out_response = AgentValue::from_serialize(&res)?;
                    self.try_output(ctx.clone(), PIN_RESPONSE, out_response)?;

                    self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;

                    if res.done {
                        break;
                    }
                }

                // Call tools if any
                if let Some(tool_calls) = &message.tool_calls {
                    self.call_tools(ctx.clone(), tool_calls).await?;
                    self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;
                } else {
                    return Ok(());
                }
            } else {
                let res = client
                    .send_chat_messages(request)
                    .await
                    .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

                let mut message: Message = res.message.clone().into();
                message.id = Some(id.clone());

                self.history.push(message.clone());

                self.try_output(ctx.clone(), PIN_MESSAGE, message.clone().into())?;

                let out_response = AgentValue::from_serialize(&res)?;
                self.try_output(ctx.clone(), PIN_RESPONSE, out_response)?;

                self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;

                // Call tools if any
                if let Some(tool_calls) = &message.tool_calls {
                    self.call_tools(ctx.clone(), tool_calls).await?;
                    self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;
                } else {
                    return Ok(());
                }
            }
        }
    }
}

// Ollama Embeddings Agent
#[askit_agent(
    title="Ollama Embeddings",
    category=CATEGORY,
    inputs=[PIN_INPUT],
    outputs=[PIN_EMBEDDINGS],
    string_config(name=CONFIG_MODEL, default=DEFAULT_CONFIG_MODEL),
    text_config(name=CONFIG_OPTIONS, default="{}")
)]
pub struct OllamaEmbeddingsAgent {
    data: AgentData,
    manager: OllamaManager,
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
        let mut request = GenerateEmbeddingsRequest::new(config_model.to_string(), input.into());

        let config_options = self.configs()?.get_string_or_default(CONFIG_OPTIONS);
        if !config_options.is_empty() && config_options != "{}" {
            if let Ok(options_json) = serde_json::from_str::<ModelOptions>(&config_options) {
                request = request.options(options_json);
            } else {
                return Err(AgentError::InvalidValue(
                    "Invalid JSON in options".to_string(),
                ));
            }
        }

        let res = client
            .generate_embeddings(request)
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

        let embeddings = AgentValue::from_serialize(&res.embeddings)?;
        self.try_output(ctx.clone(), PIN_EMBEDDINGS, embeddings)?;

        Ok(())
    }
}

// Ollama List Local Models
#[askit_agent(
    title="Ollama List Local Models",
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

        self.try_output(ctx.clone(), PIN_MODEL_LIST, model_list)?;
        Ok(())
    }
}

// Ollama Show Model Info
#[askit_agent(
    title="Ollama Show Model Info",
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

        self.try_output(ctx.clone(), PIN_MODEL_INFO, model_info)?;
        Ok(())
    }
}

impl From<ChatMessage> for Message {
    fn from(msg: ChatMessage) -> Self {
        let role = match msg.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::Tool => "tool",
        };
        let mut message = Message::new(role.to_string(), msg.content);
        if !msg.tool_calls.is_empty() {
            let mut calls = vec![];
            for call in msg.tool_calls {
                let tool_call = crate::message::ToolCall {
                    function: crate::message::ToolCallFunction {
                        id: None,
                        name: call.function.name,
                        parameters: call.function.arguments,
                    },
                };
                calls.push(tool_call);
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
}

impl From<Message> for ChatMessage {
    fn from(msg: Message) -> Self {
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
}

impl ChatHistory for MessageHistory {
    fn push(&mut self, message: ChatMessage) {
        self.push(message.into());
    }

    fn messages(&self) -> std::borrow::Cow<'_, [ChatMessage]> {
        let messages: Vec<ChatMessage> = self
            .messages()
            .iter()
            .map(|msg| msg.clone().into())
            .collect();
        std::borrow::Cow::Owned(messages)
    }
}

impl From<tool::ToolInfo> for ollama_rs::generation::tools::ToolInfo {
    fn from(info: tool::ToolInfo) -> Self {
        let schema: Schema = if let Some(params) = info.parameters {
            schema_for_value!(params)
        } else {
            json_schema!({})
        };
        // let schema = json_schema!({});
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
