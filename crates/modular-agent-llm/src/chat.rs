use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AgentValueMap,
    AsAgent, Message, ModularAgent, ToolCall, ToolCallFunction, async_trait, modular_agent,
};

use crate::provider::{ModelIdentifier, ProviderKind};

#[cfg(feature = "openai")]
use crate::openai_client;

#[cfg(feature = "ollama")]
use crate::ollama_client;

use im::vector;

const CATEGORY: &str = "LLM";

const PORT_MESSAGE: &str = "message";
const PORT_RESPONSE: &str = "response";

const CONFIG_MODEL: &str = "model";
const CONFIG_OPENAI_API_KEY: &str = "openai_api_key";
const CONFIG_OPENAI_API_BASE: &str = "openai_api_base";
const CONFIG_OLLAMA_URL: &str = "ollama_url";
const CONFIG_OPTIONS: &str = "options";
const CONFIG_STREAM: &str = "stream";
const CONFIG_TOOLS: &str = "tools";

const DEFAULT_CONFIG_MODEL: &str = "gpt-5-nano";
const DEFAULT_OPENAI_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Unified Chat Agent that routes to different LLM providers based on model prefix.
///
/// # Model Format
/// - `openai/gpt-5-mini` - Uses OpenAI API
/// - `ollama/llama3.2:1b` - Uses Ollama
/// - `gpt-5-nano` - No prefix defaults to OpenAI (backward compatible)
#[modular_agent(
    title = "Chat",
    category = CATEGORY,
    inputs = [PORT_MESSAGE],
    outputs = [PORT_MESSAGE, PORT_RESPONSE],
    boolean_config(name = CONFIG_STREAM, title = "Stream"),
    string_config(name = CONFIG_MODEL, default = DEFAULT_CONFIG_MODEL),
    text_config(name = CONFIG_TOOLS),
    object_config(name = CONFIG_OPTIONS),
    string_global_config(name = CONFIG_OPENAI_API_KEY, title = "OpenAI API Key"),
    string_global_config(name = CONFIG_OPENAI_API_BASE, title = "OpenAI API Base URL", default = DEFAULT_OPENAI_API_BASE),
    string_global_config(name = CONFIG_OLLAMA_URL, title = "Ollama URL", default = DEFAULT_OLLAMA_URL),
)]
pub struct ChatAgent {
    data: AgentData,
    #[cfg(feature = "openai")]
    openai_manager: openai_client::OpenAIManager,
    #[cfg(feature = "ollama")]
    ollama_manager: ollama_client::OllamaManager,
}

#[async_trait]
impl AsAgent for ChatAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            #[cfg(feature = "openai")]
            openai_manager: openai_client::OpenAIManager::new(),
            #[cfg(feature = "ollama")]
            ollama_manager: ollama_client::OllamaManager::new(),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config_model = self.configs()?.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        // Parse model identifier to determine provider
        let model_id = ModelIdentifier::parse(&config_model)?;

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

        // If the last message isn't a user/tool message, just return
        let role = &messages.last().unwrap().as_message().unwrap().role;
        if role != "user" && role != "tool" {
            return Ok(());
        }

        // Get common configs
        let config_options = self.configs()?.get_object_or_default(CONFIG_OPTIONS);
        let config_tools = self.configs()?.get_string_or_default(CONFIG_TOOLS);
        let use_stream = self.configs()?.get_bool_or_default(CONFIG_STREAM);

        // Route to appropriate provider
        match model_id.provider {
            #[cfg(feature = "openai")]
            ProviderKind::OpenAI => {
                self.process_openai(
                    ctx,
                    messages,
                    &model_id.model_name,
                    config_options,
                    config_tools,
                    use_stream,
                )
                .await
            }
            #[cfg(feature = "ollama")]
            ProviderKind::Ollama => {
                self.process_ollama(
                    ctx,
                    messages,
                    &model_id.model_name,
                    config_options,
                    config_tools,
                    use_stream,
                )
                .await
            }
            #[allow(unreachable_patterns)]
            _ => Err(AgentError::InvalidConfig(format!(
                "Provider {:?} not enabled. Enable the corresponding feature.",
                model_id.provider
            ))),
        }
    }
}

impl ChatAgent {
    #[cfg(feature = "openai")]
    async fn process_openai(
        &mut self,
        ctx: AgentContext,
        messages: im::Vector<AgentValue>,
        model_name: &str,
        config_options: AgentValueMap<String, AgentValue>,
        config_tools: String,
        use_stream: bool,
    ) -> Result<(), AgentError> {
        use async_openai::types::{
            ChatCompletionTool, CreateChatCompletionRequest, CreateChatCompletionRequestArgs,
        };
        use futures::StreamExt;
        use modular_agent_core::tool::list_tool_infos_patterns;

        let client = self.openai_manager.get_client(self.ma())?;

        let options_json =
            if !config_options.is_empty() {
                Some(serde_json::to_value(&config_options).map_err(|e| {
                    AgentError::InvalidValue(format!("Invalid JSON in options: {}", e))
                })?)
            } else {
                None
            };

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
                .map(|info| openai_client::try_from_tool_info_to_chat_completion_tool(info))
                .collect::<Result<Vec<ChatCompletionTool>, AgentError>>()?
        };

        let mut request = CreateChatCompletionRequestArgs::default()
            .model(model_name)
            .messages(
                messages
                    .iter()
                    .filter_map(|m| m.as_message())
                    .map(openai_client::message_to_chat_completion_msg)
                    .collect::<Vec<_>>(),
            )
            .tools(tool_infos.clone())
            .stream(use_stream)
            .build()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

        if let Some(options_json) = &options_json {
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
                let res =
                    res.map_err(|_| AgentError::IoError("OpenAI Stream Error".to_string()))?;

                for c in &res.choices {
                    if let Some(ref delta_content) = c.delta.content {
                        content.push_str(delta_content);
                    }
                    if let Some(tc) = &c.delta.tool_calls {
                        for call in tc {
                            if let Ok(c) =
                                openai_client::try_from_chat_completion_message_tool_call_chunk_to_tool_call(call)
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

                self.output(
                    ctx.clone(),
                    PORT_MESSAGE.to_string(),
                    message.clone().into(),
                )
                .await?;

                let out_response = AgentValue::from_serialize(&res)?;
                self.output(ctx.clone(), PORT_RESPONSE.to_string(), out_response)
                    .await?;
            }

            Ok(())
        } else {
            let res = client
                .chat()
                .create(request)
                .await
                .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

            for c in &res.choices {
                let mut message: Message =
                    openai_client::message_from_openai_msg(c.message.clone());
                message.id = Some(id.clone());

                self.output(
                    ctx.clone(),
                    PORT_MESSAGE.to_string(),
                    message.clone().into(),
                )
                .await?;

                let out_response = AgentValue::from_serialize(&res)?;
                self.output(ctx.clone(), PORT_RESPONSE.to_string(), out_response)
                    .await?;
            }

            Ok(())
        }
    }

    #[cfg(feature = "ollama")]
    async fn process_ollama(
        &mut self,
        ctx: AgentContext,
        messages: im::Vector<AgentValue>,
        model_name: &str,
        config_options: AgentValueMap<String, AgentValue>,
        config_tools: String,
        use_stream: bool,
    ) -> Result<(), AgentError> {
        use modular_agent_core::tool::list_tool_infos_patterns;
        use ollama_rs::generation::chat::request::ChatMessageRequest;
        use ollama_rs::models::ModelOptions;
        use tokio_stream::StreamExt;

        let client = self.ollama_manager.get_client(self.ma())?;

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
                .map(ollama_client::from_tool_info_to_ollama_tool_info)
                .collect::<Vec<_>>()
        };

        let mut request = ChatMessageRequest::new(
            model_name.to_string(),
            messages
                .iter()
                .cloned()
                .map(|m| ollama_client::message_to_chat(m.as_message().unwrap().clone()))
                .collect(),
        );

        if let Some(options) = options_json.clone() {
            request = request.options(options);
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
                let res =
                    res.map_err(|_| AgentError::IoError("Ollama Stream Error".to_string()))?;

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
                if !tool_calls.is_empty() {
                    message.tool_calls = Some(tool_calls.clone().into());
                }
                message.id = Some(id.clone());

                self.output(
                    ctx.clone(),
                    PORT_MESSAGE.to_string(),
                    message.clone().into(),
                )
                .await?;

                let out_response = AgentValue::from_serialize(&res)?;
                self.output(ctx.clone(), PORT_RESPONSE.to_string(), out_response)
                    .await?;

                if res.done {
                    break;
                }
            }

            Ok(())
        } else {
            let res = client
                .send_chat_messages(request)
                .await
                .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

            let mut message: Message = ollama_client::message_from_ollama(res.message.clone());
            message.id = Some(id.clone());

            self.output(
                ctx.clone(),
                PORT_MESSAGE.to_string(),
                message.clone().into(),
            )
            .await?;

            let out_response = AgentValue::from_serialize(&res)?;
            self.output(ctx.clone(), PORT_RESPONSE.to_string(), out_response)
                .await?;

            Ok(())
        }
    }
}
