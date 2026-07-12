use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AgentValueMap,
    AsAgent, Message, ModularAgent, ToolCall, ToolCallFunction, Usage, async_trait, modular_agent,
};

use crate::provider::{
    CONFIG_CLAUDE_API_BASE, CONFIG_CLAUDE_API_KEY, CONFIG_OLLAMA_API_KEY, CONFIG_OLLAMA_URL,
    CONFIG_OPENAI_API_BASE, CONFIG_OPENAI_API_KEY, CacheRetention, DEFAULT_CLAUDE_API_BASE,
    DEFAULT_OLLAMA_URL, DEFAULT_OPENAI_API_BASE, ModelIdentifier, ProviderKind,
};
use crate::retry::RetryPolicy;

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
const CONFIG_CACHE_RETENTION: &str = "cache_retention";
const CONFIG_MAX_RETRIES: &str = "max_retries";
const CONFIG_MAX_TOKENS: &str = "max_tokens";
const CONFIG_OPTIONS: &str = "options";
const CONFIG_RETRY_BASE_DELAY_MS: &str = "retry_base_delay_ms";
const CONFIG_STREAM: &str = "stream";
const CONFIG_TEMPERATURE: &str = "temperature";
const CONFIG_TIMEOUT_SECS: &str = "timeout_secs";
const CONFIG_TOOLS: &str = "tools";
const CONFIG_TOP_P: &str = "top_p";

const DEFAULT_CONFIG_MODEL: &str = "openai/gpt-5-nano";

/// Chat Agent that routes to different LLM providers based on model prefix.
///
/// # Model Format
/// - `openai/gpt-5-mini` - Uses OpenAI API
/// - `ollama/llama3.2:1b` - Uses Ollama
/// - `claude/claude-sonnet-4-5-20250514` - Uses Claude API
/// - `openai/qwen/qwen3-vl-8b` - Slashes after the prefix are preserved in model name
///
/// # Configuration
/// - `model`: Provider-prefixed model name (default: "openai/gpt-5-nano")
/// - `stream`: Enable streaming mode
/// - `tools`: Tool patterns to enable (regex, newline-separated)
/// - `max_tokens`: Maximum output tokens. `0`: Claude uses the model's
///   registry limit when streaming (8192 for unknown models, since Claude
///   requires the field), but caps the default at 8192 when not streaming so
///   a long generation cannot run past the per-attempt `timeout_secs`;
///   OpenAI and Ollama omit the field and use the API default. A positive
///   value is clamped to the model's known limit; models unknown to the
///   capability registry are left unclamped.
/// - `temperature`: Sampling temperature (-1: use API default)
/// - `top_p`: Nucleus sampling parameter (-1: use API default)
/// - `options`: Additional request options as JSON. For OpenAI, a `null`
///   value removes the key from the request (e.g. `{"stream_options": null}`
///   for OpenAI-compatible servers that reject the parameter)
/// - `max_retries`: Maximum automatic retries for retryable errors such as
///   rate limits, server overload, and timeouts (default: 2)
/// - `retry_base_delay_ms`: Base delay for exponential backoff between
///   retries; a server-provided Retry-After takes precedence (default: 1000)
/// - `timeout_secs`: Per-attempt deadline in seconds; for streaming it covers
///   stream establishment only (default: 300, 0 = disabled)
/// - `cache_retention`: Prompt cache retention: "none", "short", or "long"
///   (default: "short"; unrecognized values fall back to "short"). For Claude,
///   attaches ephemeral cache_control markers ("long" uses a 1h TTL) only when
///   tools are configured or the history is multi-turn, so single-shot
///   requests avoid the cache-write surcharge. For OpenAI, sends a
///   `prompt_cache_key` derived from the preset and agent IDs to improve
///   cache routing. No-op for Ollama.
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
    integer_config(name = CONFIG_MAX_RETRIES, title = "Max Retries", default = 2, description = "Automatic retries for retryable errors", detail),
    integer_config(name = CONFIG_RETRY_BASE_DELAY_MS, title = "Retry Base Delay (ms)", default = 1000, description = "Base delay for exponential backoff", detail),
    integer_config(name = CONFIG_TIMEOUT_SECS, title = "Timeout (secs)", default = 300, description = "Per-attempt deadline; 0: disabled", detail),
    string_config(name = CONFIG_CACHE_RETENTION, title = "Cache Retention", default = "short", description = "Prompt cache retention: none / short / long", detail),
    custom_global_config(name = CONFIG_CLAUDE_API_KEY, type_ = "password", default = AgentValue::string(""), title = "Claude API Key"),
    string_global_config(name = CONFIG_CLAUDE_API_BASE, title = "Claude API Base URL", default = DEFAULT_CLAUDE_API_BASE),
    custom_global_config(name = CONFIG_OPENAI_API_KEY, type_ = "password", default = AgentValue::string(""), title = "OpenAI API Key"),
    string_global_config(name = CONFIG_OPENAI_API_BASE, title = "OpenAI API Base URL", default = DEFAULT_OPENAI_API_BASE),
    custom_global_config(name = CONFIG_OLLAMA_API_KEY, type_ = "password", default = AgentValue::string(""), title = "Ollama API Key"),
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

    // With no provider features enabled, every routing arm below is compiled
    // out and the per-turn config snapshot goes unused; mirror the
    // capabilities/retry dead-code allowance in lib.rs for that build.
    #[cfg_attr(
        not(any(feature = "openai", feature = "claude", feature = "ollama")),
        allow(unused_variables)
    )]
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
        let cache_retention =
            CacheRetention::parse(&config.get_string_or_default(CONFIG_CACHE_RETENTION));

        // Snapshot retry/timeout configs once per turn so a mid-turn config
        // change cannot alter an in-flight retry loop.
        let retry = RetryPolicy::from_configs(
            config.get_integer_or_default(CONFIG_MAX_RETRIES),
            config.get_integer_or_default(CONFIG_RETRY_BASE_DELAY_MS),
            config.get_integer_or_default(CONFIG_TIMEOUT_SECS),
        );

        // Resolve the model's registry entry once per turn. A `None`
        // max_tokens means the registry doesn't know this model, in which
        // case max_tokens is left unclamped (see clamp_max_tokens) and
        // Claude uses its default.
        let caps = crate::capabilities::resolve_entry(&model_id);
        let model_max_tokens = caps.max_tokens;

        // Single cross-provider normalization boundary (P-02), applied right
        // before the provider-specific conversion below. Images are demoted
        // only when the registry positively knows image_input == false
        // (models.json or the Ollama /api/show capability probe): the
        // conservative default would otherwise strip images from every model
        // the registry doesn't list (e.g. unprobed local vision models).
        let messages = crate::prepare::prepare_messages(
            &messages,
            model_id.provider,
            caps.image_input == Some(false),
        );

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
                    model_max_tokens,
                    temperature,
                    top_p,
                    retry,
                    cache_retention,
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
                    model_max_tokens,
                    temperature,
                    top_p,
                    retry,
                    cache_retention,
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
                    model_max_tokens,
                    temperature,
                    top_p,
                    retry,
                    cache_retention,
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
        model_max_tokens: Option<u32>,
        temperature: f64,
        top_p: f64,
        retry: RetryPolicy,
        cache_retention: CacheRetention,
    ) -> Result<(), AgentError> {
        use modular_agent_core::tool::list_tool_infos_patterns;

        // Captured before building the request because a stable cache key must
        // come from the agent's identity, not per-turn state.
        let prompt_cache_key = (cache_retention != CacheRetention::None)
            .then(|| openai_client::prompt_cache_key(self.preset_id(), self.id()));

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
        if use_stream {
            // The usage-bearing final chunk is only sent when asked for. Set
            // before merge_options so options `"stream_options": null` can
            // strip the key for OpenAI-compatible servers that reject it.
            request["stream_options"] = serde_json::json!({ "include_usage": true });
        }

        openai_client::merge_options(&mut request, &config_options)?;
        if let Some(v) = crate::capabilities::clamp_max_tokens(max_tokens, model_max_tokens) {
            request["max_tokens"] = v.into();
        }
        if temperature >= 0.0 {
            request["temperature"] = temperature.into();
        }
        if top_p >= 0.0 {
            request["top_p"] = top_p.into();
        }
        // Set after merge_options so user options cannot strip the key.
        if let Some(key) = prompt_cache_key {
            request["prompt_cache_key"] = key.into();
        }

        let id = uuid::Uuid::new_v4().to_string();
        if use_stream {
            let url = client.chat_completions_url();
            // Retry covers stream establishment only: once chunks have been
            // emitted downstream they cannot be rolled back, so any failure
            // after this point must propagate instead of being retried.
            let stream = retry.run(|| client.post_stream(&url, &request)).await?;

            let mut message = Message::assistant("".to_string());
            message.id = Some(id.clone());
            // Partial emits carry streaming=true so downstream agents (e.g. tool
            // execution) act only on the final message.
            message.streaming = true;

            if let Err(e) = self.run_openai_stream(&ctx, stream, &mut message).await {
                self.emit_stream_error_message(&ctx, message).await;
                return Err(e);
            }

            Ok(())
        } else {
            let url = client.chat_completions_url();
            let res: openai_client::ChatCompletionResponse =
                retry.run(|| client.post_json(&url, &request)).await?;

            let usage = res.usage.as_ref().map(openai_client::usage_from_openai);
            for c in &res.choices {
                let mut message = openai_client::message_from_chat_response(&c.message);
                message.id = Some(id.clone());
                message.stop_reason = c
                    .finish_reason
                    .as_deref()
                    .map(openai_client::normalize_finish_reason);
                message.usage = usage;

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

    /// Emit a final same-id message marking a mid-stream failure so message
    /// history replaces the dangling partial with an error-terminated one.
    /// Best effort: the original stream error is the more useful signal, so
    /// an emit failure here must not mask it.
    #[cfg(any(feature = "openai", feature = "claude", feature = "ollama"))]
    async fn emit_stream_error_message(&mut self, ctx: &AgentContext, message: Message) {
        let Some(message) = stream_error_final(message) else {
            return;
        };
        let _ = self
            .output(ctx.clone(), PORT_MESSAGE.to_string(), message.into())
            .await;
    }

    /// Consume an established OpenAI SSE stream, emitting partial messages
    /// and exactly one finalized message. Extracted so the caller can
    /// intercept a mid-stream Err and emit an error-marked final message.
    #[cfg(feature = "openai")]
    async fn run_openai_stream(
        &mut self,
        ctx: &AgentContext,
        mut stream: impl futures::Stream<Item = Result<Option<String>, AgentError>> + Unpin,
        message: &mut Message,
    ) -> Result<(), AgentError> {
        use futures::StreamExt;

        let mut content = String::new();
        let mut thinking = String::new();
        let mut finish_reason: Option<String> = None;
        // With stream_options.include_usage, usage arrives in a final chunk
        // whose choices array is empty; held back until the final emit so
        // partial emissions never carry usage.
        let mut usage: Option<Usage> = None;
        // Tool call fragments are accumulated by index and finalized only
        // after the stream completes; partial emits never carry tool_calls
        // so downstream tool execution acts on the final message alone.
        let mut pending: std::collections::BTreeMap<u32, openai_client::PendingToolCall> =
            std::collections::BTreeMap::new();
        while let Some(res) = stream.next().await {
            let Some(data) = res? else {
                continue; // [DONE] sentinel
            };
            let chunk: openai_client::ChatStreamChunk = serde_json::from_str(&data)
                .map_err(|e| AgentError::IoError(format!("OpenAI stream parse error: {}", e)))?;

            for c in &chunk.choices {
                if let Some(ref delta_content) = c.delta.content {
                    content.push_str(delta_content);
                }
                if let Some(tc) = &c.delta.tool_calls {
                    openai_client::accumulate_tool_call_chunks(&mut pending, tc);
                }
                if let Some(refusal) = &c.delta.refusal {
                    thinking.push_str(&format!("Refusal: {}", refusal));
                }
                if let Some(reason) = &c.finish_reason {
                    finish_reason = Some(reason.clone());
                }
            }
            if let Some(u) = &chunk.usage {
                usage = Some(openai_client::usage_from_openai(u));
            }

            message.content = content.clone();
            if !thinking.is_empty() {
                message.thinking = Some(thinking.clone());
            }

            self.output(
                ctx.clone(),
                PORT_MESSAGE.to_string(),
                message.clone().into(),
            )
            .await?;

            let out_response: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
            let out_response = AgentValue::from_serialize(&out_response)?;
            self.output(ctx.clone(), PORT_RESPONSE.to_string(), out_response)
                .await?;
        }

        // All in-loop emits are partial; emit the finalized message exactly
        // once so tool calls are executed a single time per turn.
        message.content = content;
        if !thinking.is_empty() {
            message.thinking = Some(thinking);
        }
        let tool_calls = openai_client::finalize_pending_tool_calls(pending);
        if !tool_calls.is_empty() {
            message.tool_calls = Some(tool_calls.into());
        }
        message.streaming = false;
        message.stop_reason = finish_reason
            .as_deref()
            .map(openai_client::normalize_finish_reason);
        message.usage = usage;
        self.output(
            ctx.clone(),
            PORT_MESSAGE.to_string(),
            message.clone().into(),
        )
        .await?;

        Ok(())
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
        model_max_tokens: Option<u32>,
        temperature: f64,
        top_p: f64,
        retry: RetryPolicy,
        cache_retention: CacheRetention,
    ) -> Result<(), AgentError> {
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

        // Claude requires max_tokens, so a value must always be synthesized.
        // Streaming requests default to the registry-resolved model cap
        // (replacing the historical 8192 hardcode); Claude bills only actual
        // output tokens, so the higher cap costs nothing on its own.
        // Non-streaming requests keep the conservative 8192 default: a long
        // generation can exceed the per-attempt timeout, which is retried and
        // billed per attempt, and Anthropic itself steers large-max_tokens
        // requests to streaming. An explicit max_tokens config still
        // overrides this below (clamped to the model limit only).
        let registry_max = model_max_tokens.unwrap_or(crate::capabilities::DEFAULT_MAX_TOKENS);
        let default_max_tokens = if use_stream {
            registry_max
        } else {
            registry_max.min(crate::capabilities::DEFAULT_MAX_TOKENS)
        };

        // Build request
        let mut request = claude_client::ClaudeRequest {
            model: model_name.to_string(),
            max_tokens: default_max_tokens,
            messages: claude_messages,
            system: system.map(claude_client::ClaudeContent::Text),
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
        if let Some(v) = crate::capabilities::clamp_max_tokens(max_tokens, model_max_tokens) {
            request.max_tokens = v;
        }
        if temperature >= 0.0 {
            request.temperature = Some(temperature);
        }
        if top_p >= 0.0 {
            request.top_p = Some(top_p);
        }

        // Applied after the options merge so markers survive the round-trip
        // and options cannot accidentally strip them.
        claude_client::apply_cache_control(&mut request, cache_retention);

        let id = uuid::Uuid::new_v4().to_string();
        if use_stream {
            // Retry covers stream establishment only: once chunks have been
            // emitted downstream they cannot be rolled back, so any failure
            // after this point must propagate instead of being retried.
            let stream = retry.run(|| client.create_message_stream(&request)).await?;

            let mut message = Message::assistant(String::new());
            message.id = Some(id.clone());
            // Partial emits carry streaming=true; MessageStop flips it to false.
            message.streaming = true;

            if let Err(e) = self.run_claude_stream(&ctx, stream, &mut message).await {
                self.emit_stream_error_message(&ctx, message).await;
                return Err(e);
            }

            Ok(())
        } else {
            let response = retry.run(|| client.create_message(&request)).await?;

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

    /// Consume an established Claude SSE stream, emitting partial messages
    /// and the finalized message on MessageStop. Extracted so the caller can
    /// intercept a mid-stream Err and emit an error-marked final message.
    #[cfg(feature = "claude")]
    async fn run_claude_stream(
        &mut self,
        ctx: &AgentContext,
        mut stream: impl futures::Stream<Item = Result<claude_client::ClaudeStreamEvent, AgentError>>
        + Unpin,
        message: &mut Message,
    ) -> Result<(), AgentError> {
        use futures::StreamExt;

        let mut content = String::new();
        let mut thinking = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut stop_reason: Option<String> = None;
        // Accumulated field-wise: input/cache token counts arrive once on
        // message_start, cumulative output_tokens on message_delta. Held
        // back until MessageStop so partial emissions never carry usage.
        let mut usage: Option<Usage> = None;

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
                        let (parameters, parse_error) =
                            crate::json_repair::parse_tool_arguments(&current_tool_arguments);
                        tool_calls.push(ToolCall {
                            function: ToolCallFunction {
                                id: current_tool_id.take(),
                                name,
                                parameters,
                                parse_error,
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
                claude_client::ClaudeStreamEvent::MessageStart { message: start } => {
                    if let Some(u) = &start.usage {
                        usage = Some(claude_client::usage_from_claude(u));
                    }
                }
                claude_client::ClaudeStreamEvent::MessageStop {} => {
                    // Final output with all accumulated data
                    message.streaming = false;
                    message.stop_reason = stop_reason.clone();
                    message.usage = usage;
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
                claude_client::ClaudeStreamEvent::MessageDelta {
                    delta,
                    usage: delta_usage,
                } => {
                    if let Some(reason) = delta.stop_reason {
                        stop_reason = Some(claude_client::normalize_stop_reason(&reason));
                    }
                    if let Some(u) = &delta_usage {
                        // message_delta reports output_tokens cumulatively,
                        // so the latest value overwrites.
                        let acc = usage.get_or_insert_with(Usage::default);
                        acc.output_tokens = u64::from(u.output_tokens);
                    }
                }
                claude_client::ClaudeStreamEvent::Error { error } => {
                    return Err(AgentError::IoError(format!(
                        "Claude stream error: {}",
                        error.message
                    )));
                }
                _ => {
                    // Ping - skip
                }
            }
        }

        Ok(())
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
        model_max_tokens: Option<u32>,
        temperature: f64,
        top_p: f64,
        retry: RetryPolicy,
        // Ollama has no prompt cache API; accepted only to keep call sites uniform.
        _cache_retention: CacheRetention,
    ) -> Result<(), AgentError> {
        use modular_agent_core::tool::list_tool_infos_patterns;

        let client = self.ollama_manager.get_client(self.ma())?;

        // Best-effort: probe /api/show once per model to cache its context
        // length for later capability lookups. Never fails the request.
        crate::capabilities::warm_ollama_context(&client, model_name).await;

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
        let num_predict = crate::capabilities::clamp_max_tokens(max_tokens, model_max_tokens);
        if num_predict.is_some() || temperature >= 0.0 || top_p >= 0.0 {
            if !request.get("options").is_some_and(|v| v.is_object()) {
                request["options"] = serde_json::json!({});
            }
            let opts = request["options"].as_object_mut().unwrap();
            if let Some(v) = num_predict {
                opts.insert("num_predict".into(), v.into());
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
            let url = client.chat_url();
            // Retry covers stream establishment only: once chunks have been
            // emitted downstream they cannot be rolled back, so any failure
            // after this point must propagate instead of being retried.
            let stream = retry
                .run(|| client.post_ndjson_stream::<ollama_client::ChatResponse>(&url, &request))
                .await?;

            let mut message = Message::assistant("".to_string());
            message.id = Some(id.clone());
            // Partial emits carry streaming=true; the done=true chunk flips it.
            message.streaming = true;

            if let Err(e) = self.run_ollama_stream(&ctx, stream, &mut message).await {
                self.emit_stream_error_message(&ctx, message).await;
                return Err(e);
            }

            Ok(())
        } else {
            let url = client.chat_url();
            let res: ollama_client::ChatResponse =
                retry.run(|| client.post_json(&url, &request)).await?;

            let mut message = ollama_client::message_from_ollama(&res.message);
            message.id = Some(id.clone());
            message.stop_reason = res
                .done_reason
                .as_deref()
                .map(ollama_client::normalize_done_reason);
            message.usage = ollama_client::usage_from_ollama(&res);

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

    /// Consume an established Ollama NDJSON stream, emitting partial messages
    /// and the finalized message on the done=true chunk. Extracted so the
    /// caller can intercept a mid-stream Err and emit an error-marked final
    /// message.
    #[cfg(feature = "ollama")]
    async fn run_ollama_stream(
        &mut self,
        ctx: &AgentContext,
        mut stream: std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<ollama_client::ChatResponse, AgentError>> + Send>,
        >,
        message: &mut Message,
    ) -> Result<(), AgentError> {
        use futures::StreamExt;

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
                if let Some(props) = parameters.as_object().and_then(|obj| obj.get("properties")) {
                    parameters = props.clone();
                }

                let tool_call = ToolCall {
                    function: ToolCallFunction {
                        // Ollama sends no tool_call id; assign a stable
                        // one at generation time so tool results can be
                        // paired even after a provider switch (P-02).
                        id: Some(uuid::Uuid::new_v4().to_string()),
                        name: call.function.name.clone(),
                        parameters,
                        parse_error: None,
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
            // Only the final chunk (done=true) is a non-streaming emit.
            message.streaming = !res.done;
            if res.done {
                message.stop_reason = res
                    .done_reason
                    .as_deref()
                    .map(ollama_client::normalize_done_reason);
                // Token counts ride only on the done=true chunk, so partial
                // emits keep usage None.
                message.usage = ollama_client::usage_from_ollama(&res);
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

            if res.done {
                break;
            }
        }

        Ok(())
    }
}

/// Build the error-marked final message for a stream that failed mid-turn,
/// or `None` if the turn already delivered its final message (`streaming`
/// flips to false only at final-emit time, so emitting again would clobber a
/// successful final with the same id). Accumulated tool_calls from partial
/// emits are dropped: the model never finished the turn, so they must not
/// reach the tool executor.
#[cfg(any(feature = "openai", feature = "claude", feature = "ollama"))]
pub(crate) fn stream_error_final(mut message: Message) -> Option<Message> {
    if !message.streaming {
        return None;
    }
    message.streaming = false;
    message.stop_reason = Some("error".to_string());
    message.tool_calls = None;
    Some(message)
}

#[cfg(test)]
#[cfg(any(feature = "openai", feature = "claude", feature = "ollama"))]
mod tests {
    use super::*;

    use modular_agent_core::ConnectionSpec;
    use modular_agent_core::test_utils::{ProbeReceiver, TestProbeAgent, probe_receiver};

    /// Build a running preset with a ChatAgent whose `message` port feeds a
    /// probe, so stream-loop emits can be observed end to end.
    async fn setup_chat_with_probe() -> (ModularAgent, String, ProbeReceiver) {
        let ma = ModularAgent::init().unwrap();
        ma.ready().await.unwrap();

        let preset_id = ma.new_preset().unwrap();
        let chat_def = ma.get_agent_definition(ChatAgent::DEF_NAME).unwrap();
        let chat_id = ma
            .add_agent(preset_id.clone(), chat_def.to_spec())
            .await
            .unwrap();
        let probe_def = ma.get_agent_definition(TestProbeAgent::DEF_NAME).unwrap();
        let probe_id = ma
            .add_agent(preset_id.clone(), probe_def.to_spec())
            .await
            .unwrap();
        ma.add_connection(
            &preset_id,
            ConnectionSpec {
                source: chat_id.clone(),
                source_handle: PORT_MESSAGE.into(),
                target: probe_id.clone(),
                target_handle: "value".into(),
            },
        )
        .await
        .unwrap();
        ma.start_preset(&preset_id).await.unwrap();
        let probe_rx = probe_receiver(&ma, &probe_id).await.unwrap();

        (ma, chat_id, probe_rx)
    }

    fn streaming_seed_message(id: &str) -> Message {
        let mut message = Message::assistant(String::new());
        message.id = Some(id.to_string());
        message.streaming = true;
        message
    }

    /// Drain probe emits until the final (streaming=false) message, asserting
    /// every partial keeps stop_reason and usage None.
    async fn recv_final_message(probe_rx: &ProbeReceiver) -> Message {
        loop {
            let (_ctx, value) = probe_rx.recv().await.unwrap();
            let msg = value.as_message().unwrap().clone();
            if !msg.streaming {
                return msg;
            }
            assert_eq!(msg.stop_reason, None, "partial emits must keep None");
            assert_eq!(msg.usage, None, "partial emits must not carry usage");
        }
    }

    #[cfg(feature = "openai")]
    #[tokio::test]
    async fn openai_stream_finish_reason_lands_on_final_message() {
        let (ma, chat_id, probe_rx) = setup_chat_with_probe().await;

        let chunks: Vec<Result<Option<String>, AgentError>> = vec![
            Ok(Some(
                r#"{"choices":[{"index":0,"delta":{"content":"Hel"},"finish_reason":null}]}"#
                    .to_string(),
            )),
            Ok(Some(
                r#"{"choices":[{"index":0,"delta":{"content":"lo"},"finish_reason":"length"}]}"#
                    .to_string(),
            )),
            // stream_options.include_usage: final chunk with EMPTY choices
            Ok(Some(
                r#"{"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":5,"prompt_tokens_details":{"cached_tokens":4}}}"#
                    .to_string(),
            )),
            Ok(None), // [DONE] sentinel
        ];
        let mut message = streaming_seed_message("m1");

        {
            let agent = ma.get_agent(&chat_id).unwrap();
            let mut guard = agent.lock().await;
            let chat = guard.as_agent_mut::<ChatAgent>().unwrap();
            chat.run_openai_stream(
                &AgentContext::new(),
                futures::stream::iter(chunks),
                &mut message,
            )
            .await
            .unwrap();
        }

        let final_msg = recv_final_message(&probe_rx).await;
        assert_eq!(final_msg.stop_reason.as_deref(), Some("length"));
        assert_eq!(final_msg.content, "Hello");
        assert_eq!(final_msg.id.as_deref(), Some("m1"));
        assert_eq!(
            final_msg.usage,
            Some(Usage {
                input_tokens: 6,
                output_tokens: 5,
                cache_read_tokens: 4,
                cache_write_tokens: 0,
            })
        );

        ma.quit();
    }

    #[cfg(feature = "openai")]
    #[tokio::test]
    async fn openai_stream_error_emits_same_id_error_final() {
        let (ma, chat_id, probe_rx) = setup_chat_with_probe().await;

        let chunks: Vec<Result<Option<String>, AgentError>> = vec![
            Ok(Some(
                r#"{"choices":[{"index":0,"delta":{"content":"partial"},"finish_reason":null}]}"#
                    .to_string(),
            )),
            Err(AgentError::IoError("connection reset".into())),
        ];
        let mut message = streaming_seed_message("m1");

        {
            let agent = ma.get_agent(&chat_id).unwrap();
            let mut guard = agent.lock().await;
            let chat = guard.as_agent_mut::<ChatAgent>().unwrap();
            let ctx = AgentContext::new();
            // Same sequence as the process_* call sites: intercept the Err,
            // then emit the error-marked final for the dangling partial.
            let result = chat
                .run_openai_stream(&ctx, futures::stream::iter(chunks), &mut message)
                .await;
            assert!(result.is_err());
            chat.emit_stream_error_message(&ctx, message).await;
        }

        let final_msg = recv_final_message(&probe_rx).await;
        assert_eq!(final_msg.stop_reason.as_deref(), Some("error"));
        assert_eq!(final_msg.content, "partial");
        assert_eq!(final_msg.id.as_deref(), Some("m1"));

        ma.quit();
    }

    #[cfg(feature = "claude")]
    #[tokio::test]
    async fn claude_stream_stop_reason_lands_on_final_message() {
        let (ma, chat_id, probe_rx) = setup_chat_with_probe().await;

        let events: Vec<Result<claude_client::ClaudeStreamEvent, AgentError>> = [
            // input/cache counts arrive on message_start with a small
            // provisional output_tokens...
            r#"{"type":"message_start","message":{"id":"msg_1","usage":{"input_tokens":25,"output_tokens":1,"cache_creation_input_tokens":3,"cache_read_input_tokens":7}}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}"#,
            // ...and message_delta carries the cumulative output_tokens.
            r#"{"type":"message_delta","delta":{"stop_reason":"max_tokens"},"usage":{"output_tokens":12}}"#,
            r#"{"type":"message_stop"}"#,
        ]
        .iter()
        .map(|json| Ok(serde_json::from_str(json).unwrap()))
        .collect();
        let mut message = streaming_seed_message("m1");

        {
            let agent = ma.get_agent(&chat_id).unwrap();
            let mut guard = agent.lock().await;
            let chat = guard.as_agent_mut::<ChatAgent>().unwrap();
            chat.run_claude_stream(
                &AgentContext::new(),
                futures::stream::iter(events),
                &mut message,
            )
            .await
            .unwrap();
        }

        let final_msg = recv_final_message(&probe_rx).await;
        assert_eq!(final_msg.stop_reason.as_deref(), Some("length"));
        assert_eq!(final_msg.content, "Hi");
        assert_eq!(final_msg.id.as_deref(), Some("m1"));
        assert_eq!(
            final_msg.usage,
            Some(Usage {
                input_tokens: 25,
                output_tokens: 12,
                cache_read_tokens: 7,
                cache_write_tokens: 3,
            })
        );

        ma.quit();
    }

    #[cfg(feature = "ollama")]
    #[tokio::test]
    async fn ollama_stream_done_reason_lands_on_final_message() {
        let (ma, chat_id, probe_rx) = setup_chat_with_probe().await;

        let chunks: Vec<Result<ollama_client::ChatResponse, AgentError>> = [
            r#"{"model":"m","created_at":"t","message":{"role":"assistant","content":"Hel"},"done":false}"#,
            // Token counts ride only on the final done=true chunk.
            r#"{"model":"m","created_at":"t","message":{"role":"assistant","content":"lo"},"done":true,"done_reason":"stop","prompt_eval_count":26,"eval_count":42}"#,
        ]
        .iter()
        .map(|json| Ok(serde_json::from_str(json).unwrap()))
        .collect();
        let mut message = streaming_seed_message("m1");

        {
            let agent = ma.get_agent(&chat_id).unwrap();
            let mut guard = agent.lock().await;
            let chat = guard.as_agent_mut::<ChatAgent>().unwrap();
            chat.run_ollama_stream(
                &AgentContext::new(),
                Box::pin(futures::stream::iter(chunks)),
                &mut message,
            )
            .await
            .unwrap();
        }

        let final_msg = recv_final_message(&probe_rx).await;
        assert_eq!(final_msg.stop_reason.as_deref(), Some("stop"));
        assert_eq!(final_msg.content, "Hello");
        assert_eq!(final_msg.id.as_deref(), Some("m1"));
        assert_eq!(
            final_msg.usage,
            Some(Usage {
                input_tokens: 26,
                output_tokens: 42,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            })
        );

        ma.quit();
    }

    fn partial_message_with_tool_calls() -> Message {
        let mut message = Message::assistant("partial".to_string());
        message.id = Some("msg-1".to_string());
        message.streaming = true;
        message.tool_calls = Some(
            vec![ToolCall {
                function: ToolCallFunction {
                    id: Some("call-1".to_string()),
                    name: "do_thing".to_string(),
                    parameters: serde_json::json!({}),
                    parse_error: None,
                },
            }]
            .into(),
        );
        message
    }

    #[test]
    fn stream_error_final_marks_error_and_strips_tool_calls() {
        let message =
            stream_error_final(partial_message_with_tool_calls()).expect("should emit final");
        assert!(!message.streaming);
        assert_eq!(message.stop_reason.as_deref(), Some("error"));
        assert!(message.tool_calls.is_none());
        assert_eq!(message.content, "partial");
        assert_eq!(message.id.as_deref(), Some("msg-1"));
    }

    #[test]
    fn stream_error_final_skips_already_finalized_turn() {
        let mut message = partial_message_with_tool_calls();
        message.streaming = false;
        message.stop_reason = Some("stop".to_string());
        assert!(stream_error_final(message).is_none());
    }
}
