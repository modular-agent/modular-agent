use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AgentValueMap,
    AsAgent, Message, ModularAgent, ToolCall, ToolCallFunction, async_trait, modular_agent,
};

use crate::provider::{ModelIdentifier, ProviderKind};

#[cfg(feature = "openai")]
use crate::openai_client;

#[cfg(feature = "claude")]
use crate::claude_client;

#[cfg(feature = "ollama")]
use crate::ollama_client;

use im::vector;

const CATEGORY: &str = "LLM";

const PORT_MESSAGE: &str = "message";
const PORT_RESPONSE: &str = "response";

const CONFIG_MODEL: &str = "model";
const CONFIG_CLAUDE_API_KEY: &str = "claude_api_key";
const CONFIG_CLAUDE_API_BASE: &str = "claude_api_base";
const CONFIG_OPENAI_API_KEY: &str = "openai_api_key";
const CONFIG_OPENAI_API_BASE: &str = "openai_api_base";
const CONFIG_OLLAMA_URL: &str = "ollama_url";
const CONFIG_MAX_TOKENS: &str = "max_tokens";
const CONFIG_OPTIONS: &str = "options";
const CONFIG_STREAM: &str = "stream";
const CONFIG_TEMPERATURE: &str = "temperature";
const CONFIG_TOOLS: &str = "tools";
const CONFIG_TOP_P: &str = "top_p";

const DEFAULT_CONFIG_MODEL: &str = "openai/gpt-5-nano";
const DEFAULT_CLAUDE_API_BASE: &str = "https://api.anthropic.com";
const DEFAULT_OPENAI_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Chat Agent that routes to different LLM providers based on model prefix.
///
/// # Model Format
/// - `openai/gpt-5-mini` - Uses OpenAI API
/// - `ollama/llama3.2:1b` - Uses Ollama
/// - `claude/claude-sonnet-4-5-20250514` - Uses Claude API
/// - `openai/qwen/qwen3-vl-8b` - Slashes after the prefix are preserved in model name
#[modular_agent(
    title = "Chat",
    category = CATEGORY,
    inputs = [PORT_MESSAGE],
    outputs = [PORT_MESSAGE, PORT_RESPONSE],
    string_config(name = CONFIG_MODEL, default = DEFAULT_CONFIG_MODEL),
    boolean_config(name = CONFIG_STREAM, title = "Stream"),
    text_config(name = CONFIG_TOOLS),
    integer_config(name = CONFIG_MAX_TOKENS, title = "Max Tokens", default = 0, description = "0: use API default", detail),
    number_config(name = CONFIG_TEMPERATURE, title = "Temperature", default = -1.0, description = "-1: use API default (0.0-2.0)", detail),
    number_config(name = CONFIG_TOP_P, title = "Top P", default = -1.0, description = "-1: use API default (0.0-1.0)", detail),
    object_config(name = CONFIG_OPTIONS, title = "Options", description = "Additional request options as JSON", detail),
    custom_global_config(name = CONFIG_CLAUDE_API_KEY, type_ = "password", default = AgentValue::string(""), title = "Claude API Key"),
    string_global_config(name = CONFIG_CLAUDE_API_BASE, title = "Claude API Base URL", default = DEFAULT_CLAUDE_API_BASE),
    custom_global_config(name = CONFIG_OPENAI_API_KEY, type_ = "password", default = AgentValue::string(""), title = "OpenAI API Key"),
    string_global_config(name = CONFIG_OPENAI_API_BASE, title = "OpenAI API Base URL", default = DEFAULT_OPENAI_API_BASE),
    string_global_config(name = CONFIG_OLLAMA_URL, title = "Ollama URL", default = DEFAULT_OLLAMA_URL),
    hint(width = 2, height = 2),
)]
pub struct ChatAgent {
    data: AgentData,
    #[cfg(feature = "claude")]
    claude_manager: claude_client::ClaudeManager,
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
            #[cfg(feature = "claude")]
            claude_manager: claude_client::ClaudeManager::new(),
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
        let config = self.configs()?;
        let config_options = config.get_object_or_default(CONFIG_OPTIONS);
        let config_tools = config.get_string_or_default(CONFIG_TOOLS);
        let use_stream = config.get_bool_or_default(CONFIG_STREAM);
        let max_tokens = config.get_integer_or_default(CONFIG_MAX_TOKENS);
        let temperature = config.get_number_or_default(CONFIG_TEMPERATURE);
        let top_p = config.get_number_or_default(CONFIG_TOP_P);

        // Route to appropriate provider
        match model_id.provider {
            #[cfg(feature = "claude")]
            ProviderKind::Claude => {
                self.process_claude(
                    ctx,
                    messages,
                    &model_id.model_name,
                    config_options,
                    config_tools,
                    use_stream,
                    max_tokens,
                    temperature,
                    top_p,
                )
                .await
            }
            #[cfg(feature = "openai")]
            ProviderKind::OpenAI => {
                self.process_openai(
                    ctx,
                    messages,
                    &model_id.model_name,
                    config_options,
                    config_tools,
                    use_stream,
                    max_tokens,
                    temperature,
                    top_p,
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
                    max_tokens,
                    temperature,
                    top_p,
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
    #[allow(clippy::too_many_arguments)]
    async fn process_openai(
        &mut self,
        ctx: AgentContext,
        messages: im::Vector<AgentValue>,
        model_name: &str,
        config_options: AgentValueMap<String, AgentValue>,
        config_tools: String,
        use_stream: bool,
        max_tokens: i64,
        temperature: f64,
        top_p: f64,
    ) -> Result<(), AgentError> {
        use futures::StreamExt;
        use modular_agent_core::tool::list_tool_infos_patterns;

        let client = self.openai_manager.get_client(self.ma())?;

        let tools_json: Vec<serde_json::Value> = if config_tools.is_empty() {
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
                .map(openai_client::tool_info_to_chat_tool_json)
                .collect()
        };

        let mut request = serde_json::json!({
            "model": model_name,
            "messages": messages
                .iter()
                .filter_map(|m| m.as_message())
                .map(openai_client::message_to_chat_json)
                .collect::<Vec<_>>(),
            "stream": use_stream,
        });
        if !tools_json.is_empty() {
            request["tools"] = serde_json::Value::Array(tools_json);
        }

        openai_client::merge_options(&mut request, &config_options)?;
        if max_tokens > 0 {
            request["max_tokens"] = max_tokens.into();
        }
        if temperature >= 0.0 {
            request["temperature"] = temperature.into();
        }
        if top_p >= 0.0 {
            request["top_p"] = top_p.into();
        }

        let id = uuid::Uuid::new_v4().to_string();
        if use_stream {
            let url = client.chat_completions_url();
            let mut stream = client.post_stream(&url, &request).await?;

            let mut message = Message::assistant("".to_string());
            message.id = Some(id.clone());
            let mut content = String::new();
            let mut thinking = String::new();
            let mut tool_calls: Vec<ToolCall> = Vec::new();
            while let Some(res) = stream.next().await {
                let Some(data) = res? else {
                    continue; // [DONE] sentinel
                };
                let chunk: openai_client::ChatStreamChunk =
                    serde_json::from_str(&data).map_err(|e| {
                        AgentError::IoError(format!("OpenAI stream parse error: {}", e))
                    })?;

                for c in &chunk.choices {
                    if let Some(ref delta_content) = c.delta.content {
                        content.push_str(delta_content);
                    }
                    if let Some(tc) = &c.delta.tool_calls {
                        for call in tc {
                            if let Ok(c) = openai_client::tool_call_from_stream_chunk(call) {
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

                let out_response: serde_json::Value =
                    serde_json::from_str(&data).unwrap_or_default();
                let out_response = AgentValue::from_serialize(&out_response)?;
                self.output(ctx.clone(), PORT_RESPONSE.to_string(), out_response)
                    .await?;
            }

            Ok(())
        } else {
            let res: openai_client::ChatCompletionResponse = client
                .post_json(&client.chat_completions_url(), &request)
                .await?;

            for c in &res.choices {
                let mut message = openai_client::message_from_chat_response(&c.message);
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

    #[cfg(feature = "claude")]
    #[allow(clippy::too_many_arguments)]
    async fn process_claude(
        &mut self,
        ctx: AgentContext,
        messages: im::Vector<AgentValue>,
        model_name: &str,
        config_options: AgentValueMap<String, AgentValue>,
        config_tools: String,
        use_stream: bool,
        max_tokens: i64,
        temperature: f64,
        top_p: f64,
    ) -> Result<(), AgentError> {
        use futures::StreamExt;
        use modular_agent_core::tool::list_tool_infos_patterns;

        let client = self.claude_manager.get_client(self.ma())?;

        // Convert messages (separate system)
        let (system, claude_messages) = claude_client::messages_to_claude(&messages);

        // Build tools
        let tools: Option<Vec<claude_client::ClaudeTool>> = if config_tools.is_empty() {
            None
        } else {
            let infos = list_tool_infos_patterns(&config_tools).map_err(|e| {
                AgentError::InvalidConfig(format!("Invalid regex patterns in tools config: {}", e))
            })?;
            Some(
                infos
                    .into_iter()
                    .map(claude_client::tool_info_to_claude_tool)
                    .collect(),
            )
        };

        // Build request
        let mut request = claude_client::ClaudeRequest {
            model: model_name.to_string(),
            max_tokens: 8192,
            messages: claude_messages,
            system,
            stream: if use_stream { Some(true) } else { None },
            tools,
            thinking: None,
            temperature: None,
            top_p: None,
        };

        // Merge options
        if !config_options.is_empty() {
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
            request = serde_json::from_value::<claude_client::ClaudeRequest>(request_json)
                .map_err(|e| AgentError::InvalidValue(format!("Deserialization error: {}", e)))?;
        }

        // First-class configs override options
        if max_tokens > 0 {
            request.max_tokens = u32::try_from(max_tokens).unwrap_or(8192);
        }
        if temperature >= 0.0 {
            request.temperature = Some(temperature);
        }
        if top_p >= 0.0 {
            request.top_p = Some(top_p);
        }

        let id = uuid::Uuid::new_v4().to_string();
        if use_stream {
            let mut stream = client.create_message_stream(&request).await?;

            let mut message = Message::assistant(String::new());
            message.id = Some(id.clone());

            let mut content = String::new();
            let mut thinking = String::new();
            let mut tool_calls: Vec<ToolCall> = Vec::new();

            // Track block types by index
            let mut block_types: std::collections::HashMap<usize, String> =
                std::collections::HashMap::new();
            let mut current_tool_id: Option<String> = None;
            let mut current_tool_name: Option<String> = None;
            let mut current_tool_arguments = String::new();

            while let Some(event) = stream.next().await {
                let event = event?;

                match event {
                    claude_client::ClaudeStreamEvent::ContentBlockStart {
                        index,
                        content_block,
                    } => match &content_block {
                        claude_client::ClaudeResponseBlock::Text { .. } => {
                            block_types.insert(index, "text".to_string());
                        }
                        claude_client::ClaudeResponseBlock::ToolUse { id, name, .. } => {
                            block_types.insert(index, "tool_use".to_string());
                            current_tool_id = Some(id.clone());
                            current_tool_name = Some(name.clone());
                            current_tool_arguments.clear();
                        }
                        claude_client::ClaudeResponseBlock::Thinking { .. } => {
                            block_types.insert(index, "thinking".to_string());
                        }
                        claude_client::ClaudeResponseBlock::RedactedThinking { .. } => {
                            block_types.insert(index, "redacted_thinking".to_string());
                            if !thinking.is_empty() {
                                thinking.push('\n');
                            }
                            thinking.push_str("[redacted]");
                        }
                    },
                    claude_client::ClaudeStreamEvent::ContentBlockDelta { index, delta } => {
                        let block_type = block_types.get(&index).map(|s| s.as_str());
                        match delta {
                            claude_client::ClaudeDelta::TextDelta { text } => {
                                if block_type == Some("text") {
                                    content.push_str(&text);
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
                                }
                            }
                            claude_client::ClaudeDelta::ThinkingDelta { thinking: thought } => {
                                thinking.push_str(&thought);
                            }
                            claude_client::ClaudeDelta::InputJsonDelta { partial_json } => {
                                current_tool_arguments.push_str(&partial_json);
                            }
                            claude_client::ClaudeDelta::SignatureDelta { .. } => {
                                // Skip signature deltas
                            }
                        }
                    }
                    claude_client::ClaudeStreamEvent::ContentBlockStop { .. } => {
                        // Finalize tool call if one was being built
                        if let Some(name) = current_tool_name.take() {
                            let parameters: serde_json::Value =
                                serde_json::from_str(&current_tool_arguments).unwrap_or_default();
                            tool_calls.push(ToolCall {
                                function: ToolCallFunction {
                                    id: current_tool_id.take(),
                                    name,
                                    parameters,
                                },
                            });
                            current_tool_arguments.clear();

                            message.content = content.clone();
                            if !thinking.is_empty() {
                                message.thinking = Some(thinking.clone());
                            }
                            message.tool_calls = Some(tool_calls.clone().into());
                            self.output(
                                ctx.clone(),
                                PORT_MESSAGE.to_string(),
                                message.clone().into(),
                            )
                            .await?;
                        }
                    }
                    claude_client::ClaudeStreamEvent::MessageStop {} => {
                        // Final output with all accumulated data
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
                    }
                    claude_client::ClaudeStreamEvent::Error { error } => {
                        return Err(AgentError::IoError(format!(
                            "Claude stream error: {}",
                            error.message
                        )));
                    }
                    _ => {
                        // MessageStart, MessageDelta, Ping - skip
                    }
                }
            }

            Ok(())
        } else {
            let response = client.create_message(&request).await?;

            let mut message = claude_client::message_from_claude_response(&response);
            message.id = Some(id.clone());

            self.output(
                ctx.clone(),
                PORT_MESSAGE.to_string(),
                message.clone().into(),
            )
            .await?;

            let out_response = AgentValue::from_serialize(&response)?;
            self.output(ctx.clone(), PORT_RESPONSE.to_string(), out_response)
                .await?;

            Ok(())
        }
    }

    #[cfg(feature = "ollama")]
    #[allow(clippy::too_many_arguments)]
    async fn process_ollama(
        &mut self,
        ctx: AgentContext,
        messages: im::Vector<AgentValue>,
        model_name: &str,
        config_options: AgentValueMap<String, AgentValue>,
        config_tools: String,
        use_stream: bool,
        max_tokens: i64,
        temperature: f64,
        top_p: f64,
    ) -> Result<(), AgentError> {
        use futures::StreamExt;
        use modular_agent_core::tool::list_tool_infos_patterns;

        let client = self.ollama_manager.get_client(self.ma())?;

        let tools: Vec<ollama_client::OllamaToolInfo> = if config_tools.is_empty() {
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
                .map(ollama_client::tool_info_to_ollama)
                .collect()
        };

        let ollama_messages: Vec<serde_json::Value> = messages
            .iter()
            .filter_map(|m| m.as_message())
            .map(|m| {
                serde_json::to_value(ollama_client::message_to_ollama(m))
                    .unwrap_or(serde_json::json!({}))
            })
            .collect();

        let mut request = serde_json::json!({
            "model": model_name,
            "messages": ollama_messages,
            "stream": use_stream,
        });

        if !tools.is_empty() {
            request["tools"] = serde_json::to_value(&tools).unwrap_or(serde_json::json!([]));
        }

        ollama_client::merge_options(&mut request, &config_options)?;
        if max_tokens > 0 || temperature >= 0.0 || top_p >= 0.0 {
            if !request.get("options").is_some_and(|v| v.is_object()) {
                request["options"] = serde_json::json!({});
            }
            let opts = request["options"].as_object_mut().unwrap();
            if max_tokens > 0 {
                opts.insert("num_predict".into(), max_tokens.into());
            }
            if temperature >= 0.0 {
                opts.insert("temperature".into(), temperature.into());
            }
            if top_p >= 0.0 {
                opts.insert("top_p".into(), top_p.into());
            }
        }

        let id = uuid::Uuid::new_v4().to_string();
        if use_stream {
            let mut stream = client
                .post_ndjson_stream::<ollama_client::ChatResponse>(&client.chat_url(), &request)
                .await?;

            let mut message = Message::assistant("".to_string());
            let mut content = String::new();
            let mut thinking = String::new();
            let mut tool_calls: Vec<ToolCall> = vec![];
            while let Some(res) = stream.next().await {
                let res = res?;

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
            let res: ollama_client::ChatResponse =
                client.post_json(&client.chat_url(), &request).await?;

            let mut message = ollama_client::message_from_ollama(&res.message);
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
