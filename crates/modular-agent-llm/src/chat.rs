use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AgentValueMap,
    AsAgent, Message, MessageEvent, ModularAgent, ToolCall, ToolCallFunction, Usage, async_trait,
    modular_agent,
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
const PORT_EVENT: &str = "event";

const CONFIG_MODEL: &str = "model";
const CONFIG_CACHE_RETENTION: &str = "cache_retention";
const CONFIG_EMIT_PARTIAL_MESSAGES: &str = "emit_partial_messages";
const CONFIG_MAX_RETRIES: &str = "max_retries";
const CONFIG_MAX_TOKENS: &str = "max_tokens";
const CONFIG_OPTIONS: &str = "options";
const CONFIG_RETRY_BASE_DELAY_MS: &str = "retry_base_delay_ms";
const CONFIG_STREAM: &str = "stream";
const CONFIG_TEMPERATURE: &str = "temperature";
const CONFIG_THINKING_LEVEL: &str = "thinking_level";
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
/// # Ports
/// - Input `message`: User/tool message (or message array) to send to the model
/// - Output `message`: Assistant message. When streaming, accumulated partial
///   messages (`streaming` = true) are re-sent per delta (unless
///   `emit_partial_messages` is false), always followed by exactly one final
///   message (`streaming` = false). A mid-stream failure or a cancelled flow
///   (`ModularAgent::abort_context`) still gets that final: same id, the
///   partial content so far, and `stop_reason` "error" or "aborted"
/// - Output `response`: Raw provider response (per-chunk when streaming)
/// - Output `event`: Typed `MessageEvent` object whose `type` field is one of
///   `start`, `text_delta`, `thinking_delta`, `tool_call_start`,
///   `tool_call_delta`, `tool_call_end`, `done`, `error`. Incremental events
///   carry both the `delta` and the accumulated `partial` message; `done`
///   carries the same final message emitted on the `message` port and is
///   emitted after it (as is `error` relative to its error-marked final).
///   Always emitted regardless of `emit_partial_messages`; a non-streaming
///   turn emits `done` only — one per choice when the OpenAI `n` option
///   requests several
///
/// # Configuration
/// - `model`: Provider-prefixed model name (default: "openai/gpt-5-nano")
/// - `stream`: Enable streaming mode
/// - `emit_partial_messages`: Re-send accumulated partial messages on the
///   `message` port while streaming. When false, only the final message is
///   emitted there; the `event` port is unaffected (default: true)
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
/// - `thinking_level`: Reasoning intensity: "off", "minimal", "low",
///   "medium", or "high" (default: "off"; unrecognized values act as "off").
///   Clamped to the nearest level the model supports according to the
///   capability registry; models without thinking support silently run with
///   thinking off, so one patch works across models. Provider mapping:
///   Claude budget-mechanism models get `thinking.budget_tokens`
///   (minimal=1024, low=2048, medium=8192, high=16384; the budget is added
///   to max_tokens, re-clamped to the model limit, and shrunk when needed
///   so it stays below the final max_tokens), Claude adaptive-thinking
///   models get `thinking: adaptive` + `output_config.effort`, OpenAI gets
///   `reasoning_effort`, Ollama gets `think: true` (support is probed from
///   the server per model, taking effect from the model's second turn
///   unless a models.json entry declares it). When thinking is enabled on a
///   Claude request,
///   `temperature` and `top_p` are not sent (the API rejects them); if
///   configured, a warning is logged
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
///   `prompt_cache_key` derived from the patch and agent IDs to improve
///   cache routing. No-op for Ollama.
#[modular_agent(
    title = "Chat",
    category = CATEGORY,
    inputs = [PORT_MESSAGE],
    outputs = [PORT_MESSAGE, PORT_RESPONSE, PORT_EVENT],
    string_config(name = CONFIG_MODEL, default = DEFAULT_CONFIG_MODEL),
    boolean_config(name = CONFIG_STREAM, title = "Stream"),
    boolean_config(name = CONFIG_EMIT_PARTIAL_MESSAGES, title = "Emit Partial Messages", default = true, description = "Re-send partial messages on the message port while streaming", detail),
    text_config(name = CONFIG_TOOLS),
    integer_config(name = CONFIG_MAX_TOKENS, title = "Max Tokens", default = 0, description = "0: use API default", detail),
    number_config(name = CONFIG_TEMPERATURE, title = "Temperature", default = -1.0, description = "-1: use API default (0.0-2.0)", detail),
    number_config(name = CONFIG_TOP_P, title = "Top P", default = -1.0, description = "-1: use API default (0.0-1.0)", detail),
    string_config(name = CONFIG_THINKING_LEVEL, title = "Thinking Level", default = "off", description = "Reasoning intensity: off / minimal / low / medium / high", detail),
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
        // An aborted flow feeds synthetic "Operation aborted" tool results
        // back into this agent; without this guard each such trigger would
        // issue one more full-price LLM request (indefinitely with
        // stream=false, since the non-streaming request has no later
        // cancellation point).
        if ctx.is_cancelled() {
            return Err(AgentError::Cancelled);
        }

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

        // Clamp the requested thinking level to what the model supports; an
        // unknown or non-reasoning model degrades to "off" so one patch
        // works across models.
        let thinking = crate::capabilities::clamp_thinking_level(
            crate::capabilities::ThinkingLevel::parse(
                &config.get_string_or_default(CONFIG_THINKING_LEVEL),
            ),
            caps.thinking_levels.as_deref().unwrap_or(&[]),
        );

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
                    thinking,
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
                    thinking,
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
                    thinking,
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
        thinking: Option<(crate::capabilities::ThinkingLevel, Option<String>)>,
        retry: RetryPolicy,
        cache_retention: CacheRetention,
    ) -> Result<(), AgentError> {
        use modular_agent_core::tool::list_tool_infos_patterns;

        // Captured before building the request because a stable cache key must
        // come from the agent's identity, not per-turn state.
        let prompt_cache_key = (cache_retention != CacheRetention::None)
            .then(|| openai_client::prompt_cache_key(self.patch_id(), self.id()));

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
        apply_openai_thinking(&mut request, &thinking);
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
            let stream = request_or_cancelled(
                ctx.cancel_token(),
                retry.run(|| client.post_stream(&url, &request)),
            )
            .await?;

            let mut message = Message::assistant("".to_string());
            message.id = Some(id.clone());
            // Partial emits carry streaming=true so downstream agents (e.g. tool
            // execution) act only on the final message.
            message.streaming = true;

            if let Err(e) = self.run_openai_stream(&ctx, stream, &mut message).await {
                self.emit_stream_error_message(&ctx, message, &e).await;
                return Err(e);
            }

            Ok(())
        } else {
            let url = client.chat_completions_url();
            let res: openai_client::ChatCompletionResponse = request_or_cancelled(
                ctx.cancel_token(),
                retry.run(|| client.post_json(&url, &request)),
            )
            .await?;

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

                self.emit_event(&ctx, MessageEvent::Done { message })
                    .await?;

                let out_response = AgentValue::from_serialize(&res)?;
                self.output(ctx.clone(), PORT_RESPONSE.to_string(), out_response)
                    .await?;
            }

            Ok(())
        }
    }

    /// Emit a typed [`MessageEvent`] on the `event` port. Unlike the
    /// `message` port this is never gated by `emit_partial_messages`, so
    /// downstream consumers get the full delta stream.
    #[cfg(any(feature = "openai", feature = "claude", feature = "ollama"))]
    async fn emit_event(
        &mut self,
        ctx: &AgentContext,
        event: MessageEvent,
    ) -> Result<(), AgentError> {
        // Events are dropped by routing when nothing is connected, but only
        // after the per-delta serde conversion below has been paid; skip it
        // up front so patches that ignore the event port pay nothing.
        if !self.ma().has_connections(self.id(), PORT_EVENT) {
            return Ok(());
        }
        let value: AgentValue = event.try_into()?;
        self.output(ctx.clone(), PORT_EVENT.to_string(), value)
            .await
    }

    /// Emit a final same-id message marking a mid-stream failure
    /// (stop_reason "error") or cancellation (stop_reason "aborted") so
    /// message history replaces the dangling partial with a terminated one,
    /// plus a matching `Error` event on the `event` port.
    /// Best effort: the original stream error is the more useful signal, so
    /// an emit failure here must not mask it.
    #[cfg(any(feature = "openai", feature = "claude", feature = "ollama"))]
    async fn emit_stream_error_message(
        &mut self,
        ctx: &AgentContext,
        message: Message,
        error: &AgentError,
    ) {
        let Some(message) = stream_error_final(message, error) else {
            return;
        };
        let _ = self
            .output(
                ctx.clone(),
                PORT_MESSAGE.to_string(),
                message.clone().into(),
            )
            .await;
        let _ = self
            .emit_event(
                ctx,
                MessageEvent::Error {
                    message,
                    error: error.to_string(),
                },
            )
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
        // get_bool_or with an explicit true keeps the fallback aligned with
        // the declared config default when the key is absent (old spec not
        // yet reconciled).
        let emit_partials = self
            .configs()?
            .get_bool_or(CONFIG_EMIT_PARTIAL_MESSAGES, true);

        self.emit_event(
            ctx,
            MessageEvent::Start {
                partial: message.clone(),
            },
        )
        .await?;

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
        while let Some(res) = next_or_cancelled(&mut stream, ctx.cancel_token()).await? {
            let Some(data) = res? else {
                continue; // [DONE] sentinel
            };
            let chunk: openai_client::ChatStreamChunk = serde_json::from_str(&data)
                .map_err(|e| AgentError::IoError(format!("OpenAI stream parse error: {}", e)))?;

            for c in &chunk.choices {
                if let Some(reasoning) = c
                    .delta
                    .reasoning_content
                    .as_ref()
                    .or(c.delta.reasoning.as_ref())
                    && !reasoning.is_empty()
                {
                    thinking.push_str(reasoning);
                    message.content = crate::content::content_with_thinking(&thinking, &content);
                    self.emit_event(
                        ctx,
                        MessageEvent::ThinkingDelta {
                            delta: reasoning.clone(),
                            partial: message.clone(),
                        },
                    )
                    .await?;
                }
                if let Some(ref delta_content) = c.delta.content {
                    content.push_str(delta_content);
                    if !delta_content.is_empty() {
                        message.content =
                            crate::content::content_with_thinking(&thinking, &content);
                        self.emit_event(
                            ctx,
                            MessageEvent::TextDelta {
                                delta: delta_content.clone(),
                                partial: message.clone(),
                            },
                        )
                        .await?;
                    }
                }
                if let Some(tc) = &c.delta.tool_calls {
                    for call in tc {
                        let is_new = !pending.contains_key(&call.index);
                        openai_client::accumulate_tool_call_chunks(
                            &mut pending,
                            std::slice::from_ref(call),
                        );
                        // OpenAI-compatible servers may number tool_call
                        // chunks from a non-zero base, so the event index is
                        // the call's rank in the pending map — the position
                        // it takes in the final tool_calls array — keeping
                        // Start/Delta consistent with ToolCallEnd below.
                        let index = pending.range(..=call.index).count() - 1;
                        if is_new {
                            self.emit_event(
                                ctx,
                                MessageEvent::ToolCallStart {
                                    index,
                                    partial: message.clone(),
                                },
                            )
                            .await?;
                        }
                        if let Some(args) =
                            call.function.as_ref().and_then(|f| f.arguments.as_ref())
                            && !args.is_empty()
                        {
                            self.emit_event(
                                ctx,
                                MessageEvent::ToolCallDelta {
                                    index,
                                    delta: args.clone(),
                                    partial: message.clone(),
                                },
                            )
                            .await?;
                        }
                    }
                }
                if let Some(refusal) = &c.delta.refusal {
                    let delta = format!("Refusal: {}", refusal);
                    thinking.push_str(&delta);
                    message.content = crate::content::content_with_thinking(&thinking, &content);
                    self.emit_event(
                        ctx,
                        MessageEvent::ThinkingDelta {
                            delta,
                            partial: message.clone(),
                        },
                    )
                    .await?;
                }
                if let Some(reason) = &c.finish_reason {
                    finish_reason = Some(reason.clone());
                }
            }
            if let Some(u) = &chunk.usage {
                usage = Some(openai_client::usage_from_openai(u));
            }

            message.content = crate::content::content_with_thinking(&thinking, &content);

            if emit_partials {
                self.output(
                    ctx.clone(),
                    PORT_MESSAGE.to_string(),
                    message.clone().into(),
                )
                .await?;
            }

            let out_response: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
            let out_response = AgentValue::from_serialize(&out_response)?;
            self.output(ctx.clone(), PORT_RESPONSE.to_string(), out_response)
                .await?;
        }

        // All in-loop emits are partial; emit the finalized message exactly
        // once so tool calls are executed a single time per turn.
        message.content = crate::content::content_with_thinking(&thinking, &content);
        // The Chat Completions stream has no per-call end marker, so
        // ToolCallEnd events fire at finalization time, each partial extended
        // by the calls finalized so far (still streaming=true, before Done).
        let tool_calls = openai_client::finalize_pending_tool_calls(pending);
        let mut finalized: Vec<ToolCall> = Vec::with_capacity(tool_calls.len());
        for (index, tool_call) in tool_calls.into_iter().enumerate() {
            finalized.push(tool_call.clone());
            message.tool_calls = Some(finalized.clone().into());
            self.emit_event(
                ctx,
                MessageEvent::ToolCallEnd {
                    index,
                    tool_call,
                    partial: message.clone(),
                },
            )
            .await?;
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

        self.emit_event(
            ctx,
            MessageEvent::Done {
                message: message.clone(),
            },
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
        thinking: Option<(crate::capabilities::ThinkingLevel, Option<String>)>,
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
            output_config: None,
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

        // Applied after the first-class overrides so the budget rides on the
        // final max_tokens value, and so temperature/top_p suppression sees
        // everything that would otherwise be sent.
        apply_claude_thinking(&mut request, thinking, model_max_tokens);

        // Thinking blocks may only be sent while the request enables
        // extended thinking (the API rejects them otherwise), so a history
        // recorded with thinking on must degrade to text once the option is
        // removed. Decided after the options merge, where the final
        // thinking config is known.
        if !matches!(
            request.thinking,
            Some(
                claude_client::ClaudeThinkingConfig::Enabled { .. }
                    | claude_client::ClaudeThinkingConfig::Adaptive {}
            )
        ) {
            claude_client::strip_thinking_blocks(&mut request.messages);
        }

        // Applied after the options merge so markers survive the round-trip
        // and options cannot accidentally strip them.
        claude_client::apply_cache_control(&mut request, cache_retention);

        let id = uuid::Uuid::new_v4().to_string();
        if use_stream {
            // Retry covers stream establishment only: once chunks have been
            // emitted downstream they cannot be rolled back, so any failure
            // after this point must propagate instead of being retried.
            let stream = request_or_cancelled(
                ctx.cancel_token(),
                retry.run(|| client.create_message_stream(&request)),
            )
            .await?;

            let mut message = Message::assistant(String::new());
            message.id = Some(id.clone());
            // Partial emits carry streaming=true; MessageStop flips it to false.
            message.streaming = true;

            if let Err(e) = self.run_claude_stream(&ctx, stream, &mut message).await {
                self.emit_stream_error_message(&ctx, message, &e).await;
                return Err(e);
            }

            Ok(())
        } else {
            let response = request_or_cancelled(
                ctx.cancel_token(),
                retry.run(|| client.create_message(&request)),
            )
            .await?;

            let mut message = claude_client::message_from_claude_response(&response);
            message.id = Some(id.clone());

            self.output(
                ctx.clone(),
                PORT_MESSAGE.to_string(),
                message.clone().into(),
            )
            .await?;

            self.emit_event(&ctx, MessageEvent::Done { message })
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
        use modular_agent_core::ContentBlock;

        // get_bool_or with an explicit true keeps the fallback aligned with
        // the declared config default when the key is absent (old spec not
        // yet reconciled).
        let emit_partials = self
            .configs()?
            .get_bool_or(CONFIG_EMIT_PARTIAL_MESSAGES, true);

        self.emit_event(
            ctx,
            MessageEvent::Start {
                partial: message.clone(),
            },
        )
        .await?;

        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut stop_reason: Option<String> = None;
        // Accumulated field-wise: input/cache token counts arrive once on
        // message_start, cumulative output_tokens on message_delta. Held
        // back until MessageStop so partial emissions never carry usage.
        let mut usage: Option<Usage> = None;

        // Content accumulates as ordered blocks so thinking signatures and
        // redacted payloads survive for replay. block_pos maps the
        // provider's content-block index to the position in `blocks`;
        // tool_use blocks are accumulated separately and have no entry.
        let mut blocks: Vec<ContentBlock> = Vec::new();
        let mut block_pos: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        let mut current_tool_id: Option<String> = None;
        let mut current_tool_name: Option<String> = None;
        let mut current_tool_arguments = String::new();

        while let Some(event) = next_or_cancelled(&mut stream, ctx.cancel_token()).await? {
            let event = event?;

            match event {
                claude_client::ClaudeStreamEvent::ContentBlockStart {
                    index,
                    content_block,
                } => match &content_block {
                    claude_client::ClaudeResponseBlock::Text { text } => {
                        block_pos.insert(index, blocks.len());
                        blocks.push(ContentBlock::Text { text: text.clone() });
                    }
                    claude_client::ClaudeResponseBlock::ToolUse { id, name, .. } => {
                        current_tool_id = Some(id.clone());
                        current_tool_name = Some(name.clone());
                        current_tool_arguments.clear();
                        // The event index is the position this call will
                        // occupy in tool_calls once ContentBlockStop
                        // finalizes it, not the content-block index (text /
                        // thinking blocks share that space).
                        self.emit_event(
                            ctx,
                            MessageEvent::ToolCallStart {
                                index: tool_calls.len(),
                                partial: message.clone(),
                            },
                        )
                        .await?;
                    }
                    claude_client::ClaudeResponseBlock::Thinking {
                        thinking,
                        signature,
                    } => {
                        block_pos.insert(index, blocks.len());
                        blocks.push(ContentBlock::Thinking {
                            thinking: thinking.clone(),
                            signature: (!signature.is_empty()).then(|| signature.clone()),
                            redacted: false,
                        });
                    }
                    claude_client::ClaudeResponseBlock::RedactedThinking { data } => {
                        // The encrypted payload is stored verbatim for
                        // replay; the human-readable event stream gets a
                        // placeholder instead.
                        let delta = if blocks
                            .iter()
                            .any(|b| matches!(b, ContentBlock::Thinking { .. }))
                        {
                            "\n[redacted]".to_string()
                        } else {
                            "[redacted]".to_string()
                        };
                        block_pos.insert(index, blocks.len());
                        blocks.push(ContentBlock::Thinking {
                            thinking: data.clone(),
                            signature: None,
                            redacted: true,
                        });
                        message.content = crate::content::content_from_blocks(&blocks);
                        self.emit_event(
                            ctx,
                            MessageEvent::ThinkingDelta {
                                delta,
                                partial: message.clone(),
                            },
                        )
                        .await?;
                    }
                },
                claude_client::ClaudeStreamEvent::ContentBlockDelta { index, delta } => {
                    let block = block_pos.get(&index).map(|pos| &mut blocks[*pos]);
                    match delta {
                        claude_client::ClaudeDelta::TextDelta { text } => {
                            if let Some(ContentBlock::Text { text: acc }) = block {
                                acc.push_str(&text);
                                message.content = crate::content::content_from_blocks(&blocks);
                                if !tool_calls.is_empty() {
                                    message.tool_calls = Some(tool_calls.clone().into());
                                }
                                self.emit_event(
                                    ctx,
                                    MessageEvent::TextDelta {
                                        delta: text,
                                        partial: message.clone(),
                                    },
                                )
                                .await?;
                                if emit_partials {
                                    self.output(
                                        ctx.clone(),
                                        PORT_MESSAGE.to_string(),
                                        message.clone().into(),
                                    )
                                    .await?;
                                }
                            }
                        }
                        claude_client::ClaudeDelta::ThinkingDelta { thinking: thought } => {
                            if let Some(ContentBlock::Thinking { thinking: acc, .. }) = block {
                                acc.push_str(&thought);
                                message.content = crate::content::content_from_blocks(&blocks);
                                self.emit_event(
                                    ctx,
                                    MessageEvent::ThinkingDelta {
                                        delta: thought,
                                        partial: message.clone(),
                                    },
                                )
                                .await?;
                            }
                        }
                        claude_client::ClaudeDelta::InputJsonDelta { partial_json } => {
                            current_tool_arguments.push_str(&partial_json);
                            // Same index rule as ToolCallStart above: the
                            // in-flight call finalizes at tool_calls.len().
                            self.emit_event(
                                ctx,
                                MessageEvent::ToolCallDelta {
                                    index: tool_calls.len(),
                                    delta: partial_json,
                                    partial: message.clone(),
                                },
                            )
                            .await?;
                        }
                        claude_client::ClaudeDelta::SignatureDelta { signature } => {
                            // Signatures may arrive fragmented; accumulate
                            // them onto the current thinking block so the
                            // turn can be replayed with tool use.
                            if let Some(ContentBlock::Thinking { signature: sig, .. }) = block {
                                sig.get_or_insert_with(String::new).push_str(&signature);
                            }
                        }
                    }
                }
                claude_client::ClaudeStreamEvent::ContentBlockStop { .. } => {
                    // Finalize tool call if one was being built
                    if let Some(name) = current_tool_name.take() {
                        let (parameters, parse_error) =
                            crate::json_repair::parse_tool_arguments(&current_tool_arguments);
                        let tool_call = ToolCall {
                            function: ToolCallFunction {
                                id: current_tool_id.take(),
                                name,
                                parameters,
                                parse_error,
                            },
                        };
                        tool_calls.push(tool_call.clone());
                        current_tool_arguments.clear();

                        message.content = crate::content::content_from_blocks(&blocks);
                        message.tool_calls = Some(tool_calls.clone().into());
                        self.emit_event(
                            ctx,
                            MessageEvent::ToolCallEnd {
                                index: tool_calls.len() - 1,
                                tool_call,
                                partial: message.clone(),
                            },
                        )
                        .await?;
                        if emit_partials {
                            self.output(
                                ctx.clone(),
                                PORT_MESSAGE.to_string(),
                                message.clone().into(),
                            )
                            .await?;
                        }
                    }
                }
                claude_client::ClaudeStreamEvent::MessageStart { message: start } => {
                    if let Some(u) = &start.usage {
                        usage = Some(claude_client::usage_from_claude(u));
                    }
                }
                claude_client::ClaudeStreamEvent::MessageStop {} => {
                    // claude_client also maps an SSE "[DONE]" terminator to
                    // MessageStop (some gateways append it after
                    // message_stop), so re-entry after the turn is finalized
                    // must be a no-op or tools would execute twice.
                    if !message.streaming {
                        continue;
                    }
                    // Final output with all accumulated data
                    message.streaming = false;
                    message.stop_reason = stop_reason.clone();
                    message.usage = usage;
                    message.content = crate::content::content_from_blocks(&blocks);
                    if !tool_calls.is_empty() {
                        message.tool_calls = Some(tool_calls.clone().into());
                    }
                    self.output(
                        ctx.clone(),
                        PORT_MESSAGE.to_string(),
                        message.clone().into(),
                    )
                    .await?;
                    self.emit_event(
                        ctx,
                        MessageEvent::Done {
                            message: message.clone(),
                        },
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
        thinking: Option<(crate::capabilities::ThinkingLevel, Option<String>)>,
        retry: RetryPolicy,
        // Ollama has no prompt cache API; accepted only to keep call sites uniform.
        _cache_retention: CacheRetention,
    ) -> Result<(), AgentError> {
        use modular_agent_core::tool::list_tool_infos_patterns;

        let client = self.ollama_manager.get_client(self.ma())?;

        // Best-effort: probe /api/show once per model to cache its context
        // length and vision/thinking capabilities for later capability
        // lookups. Never fails the request.
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
        apply_ollama_thinking(&mut request, &thinking);

        let id = uuid::Uuid::new_v4().to_string();
        if use_stream {
            let url = client.chat_url();
            // Retry covers stream establishment only: once chunks have been
            // emitted downstream they cannot be rolled back, so any failure
            // after this point must propagate instead of being retried.
            let stream = request_or_cancelled(
                ctx.cancel_token(),
                retry.run(|| {
                    client.post_ndjson_stream::<ollama_client::ChatResponse>(&url, &request)
                }),
            )
            .await?;

            let mut message = Message::assistant("".to_string());
            message.id = Some(id.clone());
            // Partial emits carry streaming=true; the done=true chunk flips it.
            message.streaming = true;

            if let Err(e) = self.run_ollama_stream(&ctx, stream, &mut message).await {
                self.emit_stream_error_message(&ctx, message, &e).await;
                return Err(e);
            }

            Ok(())
        } else {
            let url = client.chat_url();
            let res: ollama_client::ChatResponse = request_or_cancelled(
                ctx.cancel_token(),
                retry.run(|| client.post_json(&url, &request)),
            )
            .await?;

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

            self.emit_event(&ctx, MessageEvent::Done { message })
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
        // get_bool_or with an explicit true keeps the fallback aligned with
        // the declared config default when the key is absent (old spec not
        // yet reconciled).
        let emit_partials = self
            .configs()?
            .get_bool_or(CONFIG_EMIT_PARTIAL_MESSAGES, true);

        self.emit_event(
            ctx,
            MessageEvent::Start {
                partial: message.clone(),
            },
        )
        .await?;

        let mut content = String::new();
        let mut thinking = String::new();
        let mut tool_calls: Vec<ToolCall> = vec![];
        while let Some(res) = next_or_cancelled(&mut stream, ctx.cancel_token()).await? {
            let res = res?;

            content.push_str(&res.message.content);
            if let Some(thinking_str) = res.message.thinking.as_ref() {
                thinking.push_str(thinking_str);
            }

            // Delta events are emitted before message.streaming is flipped
            // for a done=true chunk, so their partials always carry
            // streaming=true even when the data arrived on the final chunk.
            message.content = crate::content::content_with_thinking(&thinking, &content);
            if !res.message.content.is_empty() {
                self.emit_event(
                    ctx,
                    MessageEvent::TextDelta {
                        delta: res.message.content.clone(),
                        partial: message.clone(),
                    },
                )
                .await?;
            }
            if let Some(thinking_str) = res.message.thinking.as_ref()
                && !thinking_str.is_empty()
            {
                self.emit_event(
                    ctx,
                    MessageEvent::ThinkingDelta {
                        delta: thinking_str.clone(),
                        partial: message.clone(),
                    },
                )
                .await?;
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
                // Ollama delivers each tool call whole, so Start and End are
                // emitted back to back; End's partial includes the call.
                let index = tool_calls.len();
                self.emit_event(
                    ctx,
                    MessageEvent::ToolCallStart {
                        index,
                        partial: message.clone(),
                    },
                )
                .await?;
                tool_calls.push(tool_call.clone());
                message.tool_calls = Some(tool_calls.clone().into());
                self.emit_event(
                    ctx,
                    MessageEvent::ToolCallEnd {
                        index,
                        tool_call,
                        partial: message.clone(),
                    },
                )
                .await?;
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

            if res.done || emit_partials {
                self.output(
                    ctx.clone(),
                    PORT_MESSAGE.to_string(),
                    message.clone().into(),
                )
                .await?;
            }

            if res.done {
                self.emit_event(
                    ctx,
                    MessageEvent::Done {
                        message: message.clone(),
                    },
                )
                .await?;
            }

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

/// Convert a clamped thinking level into the Claude request shape.
///
/// Budget-mechanism entries (registry param `None`) enable extended thinking
/// with a per-level token budget; the budget is added on top of max_tokens —
/// thinking tokens are spent before the visible answer, so keeping max_tokens
/// unchanged would shrink the answer — and re-clamped to the model limit.
/// Anthropic requires `budget_tokens` strictly below `max_tokens`, so when
/// the model limit caps max_tokens at or below the level's budget (reachable
/// via models.json `max_tokens` overrides) the budget shrinks to half the
/// final max_tokens, and thinking degrades to off entirely when even the
/// API-minimum budget leaves no answer room — both with a warning.
/// Adaptive entries (param `Some(effort)`) switch to adaptive thinking
/// steered by `output_config.effort`. Either way the API rejects
/// temperature/top_p alongside thinking, so both are dropped with a warning
/// when set.
#[cfg(feature = "claude")]
fn apply_claude_thinking(
    request: &mut claude_client::ClaudeRequest,
    thinking: Option<(crate::capabilities::ThinkingLevel, Option<String>)>,
    model_max_tokens: Option<u32>,
) {
    let Some((level, param)) = thinking else {
        return;
    };
    match param {
        Some(effort) => {
            request.thinking = Some(claude_client::ClaudeThinkingConfig::Adaptive {});
            request.output_config = Some(claude_client::ClaudeOutputConfig { effort });
        }
        None => {
            let mut budget = claude_client::thinking_budget_tokens(level);
            let raised = request.max_tokens.saturating_add(budget);
            // Same rule as clamp_max_tokens: an unknown model limit leaves
            // the value unclamped.
            let max_tokens = match model_max_tokens {
                Some(limit) => raised.min(limit),
                None => raised,
            };
            // Anthropic rejects budget_tokens >= max_tokens, which the clamp
            // alone allows when the model limit is at or below the level's
            // budget: split the window instead so the visible answer keeps
            // room, or degrade to off when even the minimum budget can't fit.
            if budget >= max_tokens {
                let shrunk = max_tokens / 2;
                if shrunk < claude_client::MIN_THINKING_BUDGET_TOKENS {
                    log::warn!(
                        "Disabling thinking: max_tokens {max_tokens} leaves no room for the minimum thinking budget"
                    );
                    return;
                }
                log::warn!(
                    "Shrinking thinking budget from {budget} to {shrunk}: it must stay below max_tokens {max_tokens}"
                );
                budget = shrunk;
            }
            request.thinking = Some(claude_client::ClaudeThinkingConfig::Enabled {
                budget_tokens: budget,
            });
            request.max_tokens = max_tokens;
        }
    }
    if request.temperature.take().is_some() {
        log::warn!("Ignoring temperature: Claude rejects it when thinking is enabled");
    }
    if request.top_p.take().is_some() {
        log::warn!("Ignoring top_p: Claude rejects it when thinking is enabled");
    }
}

/// Send the clamped thinking level as the Chat Completions `reasoning_effort`
/// parameter. Registry entries without a provider-side value cannot be
/// expressed on OpenAI and are skipped.
#[cfg(feature = "openai")]
fn apply_openai_thinking(
    request: &mut serde_json::Value,
    thinking: &Option<(crate::capabilities::ThinkingLevel, Option<String>)>,
) {
    if let Some((_, Some(effort))) = thinking {
        request["reasoning_effort"] = effort.clone().into();
    }
}

/// Ollama thinking is a boolean switch: any enabled level maps to a
/// top-level `think: true` (per-level intensity is not expressible there).
#[cfg(feature = "ollama")]
fn apply_ollama_thinking(
    request: &mut serde_json::Value,
    thinking: &Option<(crate::capabilities::ThinkingLevel, Option<String>)>,
) {
    if thinking.is_some() {
        request["think"] = true.into();
    }
}

/// Await the next item of an established stream, racing it against the
/// flow's cancellation token. The moment the token fires (e.g. via
/// `ModularAgent::abort_context`) this returns `Err(AgentError::Cancelled)`
/// so the stream loop bails out and its caller can emit the
/// `stop_reason = "aborted"` final message. Without a token this is a plain
/// `stream.next().await`. Biased so a fired token wins even when the next
/// chunk is already buffered.
#[cfg(any(feature = "openai", feature = "claude", feature = "ollama"))]
pub(crate) async fn next_or_cancelled<S>(
    stream: &mut S,
    cancel: Option<&modular_agent_core::CancellationToken>,
) -> Result<Option<S::Item>, AgentError>
where
    S: futures::Stream + Unpin,
{
    use futures::StreamExt;
    match cancel {
        Some(token) => tokio::select! {
            biased;
            _ = token.cancelled() => Err(AgentError::Cancelled),
            item = stream.next() => Ok(item),
        },
        None => Ok(stream.next().await),
    }
}

/// Race an LLM request — including its whole retry/backoff loop — against
/// the flow's cancellation token, so an aborted flow drops the in-flight
/// request (and any remaining retries) instead of running them to
/// completion. Establishment-only: nothing has been emitted downstream yet,
/// so dropping here is history-safe. Without a token this is a plain await.
#[cfg(any(feature = "openai", feature = "claude", feature = "ollama"))]
pub(crate) async fn request_or_cancelled<T>(
    cancel: Option<&modular_agent_core::CancellationToken>,
    fut: impl std::future::Future<Output = Result<T, AgentError>>,
) -> Result<T, AgentError> {
    match cancel {
        Some(token) => tokio::select! {
            biased;
            _ = token.cancelled() => Err(AgentError::Cancelled),
            r = fut => r,
        },
        None => fut.await,
    }
}

/// Build the final message for a stream that failed or was cancelled
/// mid-turn, or `None` if the turn already delivered its final message
/// (`streaming` flips to false only at final-emit time, so emitting again
/// would clobber a successful final with the same id). The stop_reason is
/// "aborted" for a cancellation and "error" otherwise; either way the same
/// message id and the partial content accumulated so far are preserved so
/// history keeps the id-dedup replacement working. Accumulated tool_calls
/// from partial emits are dropped: the model never finished the turn, so
/// they must not reach the tool executor.
#[cfg(any(feature = "openai", feature = "claude", feature = "ollama"))]
pub(crate) fn stream_error_final(mut message: Message, error: &AgentError) -> Option<Message> {
    if !message.streaming {
        return None;
    }
    message.streaming = false;
    message.stop_reason = Some(
        if matches!(error, AgentError::Cancelled) {
            "aborted"
        } else {
            "error"
        }
        .to_string(),
    );
    message.tool_calls = None;
    Some(message)
}

#[cfg(test)]
#[cfg(any(feature = "openai", feature = "claude", feature = "ollama"))]
mod tests {
    use super::*;

    use modular_agent_core::ConnectionSpec;
    use modular_agent_core::test_utils::{ProbeReceiver, TestProbeAgent, probe_receiver};

    /// Build a running patch with a ChatAgent whose `source_port` feeds a
    /// probe, so stream-loop emits can be observed end to end.
    async fn setup_chat_probe_on(source_port: &str) -> (ModularAgent, String, ProbeReceiver) {
        let ma = ModularAgent::init().unwrap();
        ma.ready().await.unwrap();

        let patch_id = ma.new_patch().unwrap();
        let chat_def = ma.get_agent_definition(ChatAgent::DEF_NAME).unwrap();
        let chat_id = ma
            .add_agent(patch_id.clone(), chat_def.to_spec())
            .await
            .unwrap();
        let probe_def = ma.get_agent_definition(TestProbeAgent::DEF_NAME).unwrap();
        let probe_id = ma
            .add_agent(patch_id.clone(), probe_def.to_spec())
            .await
            .unwrap();
        ma.add_connection(
            &patch_id,
            ConnectionSpec {
                source: chat_id.clone(),
                source_handle: source_port.into(),
                target: probe_id.clone(),
                target_handle: "value".into(),
            },
        )
        .await
        .unwrap();
        ma.start_patch(&patch_id).await.unwrap();
        let probe_rx = probe_receiver(&ma, &probe_id).await.unwrap();

        (ma, chat_id, probe_rx)
    }

    async fn setup_chat_with_probe() -> (ModularAgent, String, ProbeReceiver) {
        setup_chat_probe_on(PORT_MESSAGE).await
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
        assert_eq!(final_msg.text(), "Hello");
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
            let err = result.unwrap_err();
            chat.emit_stream_error_message(&ctx, message, &err).await;
        }

        let final_msg = recv_final_message(&probe_rx).await;
        assert_eq!(final_msg.stop_reason.as_deref(), Some("error"));
        assert_eq!(final_msg.text(), "partial");
        assert_eq!(final_msg.id.as_deref(), Some("m1"));

        ma.quit();
    }

    /// Chain a tail onto `items` that fires `token` when polled and then
    /// stays pending, deterministically simulating a flow abort arriving
    /// mid-stream: the loop consumes every item, then blocks on the next
    /// chunk until the cancellation wakes it.
    fn cancel_after<T>(
        items: Vec<T>,
        token: modular_agent_core::CancellationToken,
    ) -> impl futures::Stream<Item = T> + Unpin {
        use futures::StreamExt;
        futures::stream::iter(items).chain(futures::stream::poll_fn(move |_| {
            token.cancel();
            std::task::Poll::Pending
        }))
    }

    #[cfg(feature = "openai")]
    #[tokio::test]
    async fn openai_stream_cancel_emits_same_id_aborted_final() {
        let (ma, chat_id, probe_rx) = setup_chat_with_probe().await;

        let token = modular_agent_core::CancellationToken::new();
        let chunks: Vec<Result<Option<String>, AgentError>> = vec![Ok(Some(
            r#"{"choices":[{"index":0,"delta":{"content":"partial"},"finish_reason":null}]}"#
                .to_string(),
        ))];
        let stream = cancel_after(chunks, token.clone());
        let mut message = streaming_seed_message("m1");

        {
            let agent = ma.get_agent(&chat_id).unwrap();
            let mut guard = agent.lock().await;
            let chat = guard.as_agent_mut::<ChatAgent>().unwrap();
            let ctx = AgentContext::new().with_cancel_token(token);
            // Same sequence as the process_* call sites: intercept the
            // Cancelled, then emit the aborted-marked final for the
            // dangling partial.
            let err = chat
                .run_openai_stream(&ctx, stream, &mut message)
                .await
                .unwrap_err();
            assert!(matches!(err, AgentError::Cancelled));
            chat.emit_stream_error_message(&ctx, message, &err).await;
        }

        let final_msg = recv_final_message(&probe_rx).await;
        assert_eq!(final_msg.stop_reason.as_deref(), Some("aborted"));
        assert_eq!(final_msg.text(), "partial");
        assert_eq!(final_msg.id.as_deref(), Some("m1"));

        ma.quit();
    }

    #[cfg(feature = "claude")]
    #[tokio::test]
    async fn claude_stream_cancel_emits_same_id_aborted_final() {
        let (ma, chat_id, probe_rx) = setup_chat_with_probe().await;

        let token = modular_agent_core::CancellationToken::new();
        let events: Vec<Result<claude_client::ClaudeStreamEvent, AgentError>> = [
            r#"{"type":"message_start","message":{"id":"msg_1","usage":{"input_tokens":5,"output_tokens":1}}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"partial"}}"#,
        ]
        .iter()
        .map(|json| Ok(serde_json::from_str(json).unwrap()))
        .collect();
        let stream = cancel_after(events, token.clone());
        let mut message = streaming_seed_message("m1");

        {
            let agent = ma.get_agent(&chat_id).unwrap();
            let mut guard = agent.lock().await;
            let chat = guard.as_agent_mut::<ChatAgent>().unwrap();
            let ctx = AgentContext::new().with_cancel_token(token);
            let err = chat
                .run_claude_stream(&ctx, stream, &mut message)
                .await
                .unwrap_err();
            assert!(matches!(err, AgentError::Cancelled));
            chat.emit_stream_error_message(&ctx, message, &err).await;
        }

        let final_msg = recv_final_message(&probe_rx).await;
        assert_eq!(final_msg.stop_reason.as_deref(), Some("aborted"));
        assert_eq!(final_msg.text(), "partial");
        assert_eq!(final_msg.id.as_deref(), Some("m1"));

        ma.quit();
    }

    #[cfg(feature = "ollama")]
    #[tokio::test]
    async fn ollama_stream_cancel_emits_same_id_aborted_final() {
        let (ma, chat_id, probe_rx) = setup_chat_with_probe().await;

        let token = modular_agent_core::CancellationToken::new();
        let chunks: Vec<Result<ollama_client::ChatResponse, AgentError>> = vec![Ok(
            serde_json::from_str(
                r#"{"model":"m","created_at":"t","message":{"role":"assistant","content":"partial"},"done":false}"#,
            )
            .unwrap(),
        )];
        let stream = cancel_after(chunks, token.clone());
        let mut message = streaming_seed_message("m1");

        {
            let agent = ma.get_agent(&chat_id).unwrap();
            let mut guard = agent.lock().await;
            let chat = guard.as_agent_mut::<ChatAgent>().unwrap();
            let ctx = AgentContext::new().with_cancel_token(token);
            let err = chat
                .run_ollama_stream(&ctx, Box::pin(stream), &mut message)
                .await
                .unwrap_err();
            assert!(matches!(err, AgentError::Cancelled));
            chat.emit_stream_error_message(&ctx, message, &err).await;
        }

        let final_msg = recv_final_message(&probe_rx).await;
        assert_eq!(final_msg.stop_reason.as_deref(), Some("aborted"));
        assert_eq!(final_msg.text(), "partial");
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
        assert_eq!(final_msg.text(), "Hi");
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
        assert_eq!(final_msg.text(), "Hello");
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
        let message = stream_error_final(
            partial_message_with_tool_calls(),
            &AgentError::IoError("connection reset".into()),
        )
        .expect("should emit final");
        assert!(!message.streaming);
        assert_eq!(message.stop_reason.as_deref(), Some("error"));
        assert!(message.tool_calls.is_none());
        assert_eq!(message.text(), "partial");
        assert_eq!(message.id.as_deref(), Some("msg-1"));
    }

    #[test]
    fn stream_error_final_marks_cancellation_as_aborted() {
        let message = stream_error_final(partial_message_with_tool_calls(), &AgentError::Cancelled)
            .expect("should emit final");
        assert!(!message.streaming);
        assert_eq!(message.stop_reason.as_deref(), Some("aborted"));
        assert!(message.tool_calls.is_none());
        assert_eq!(message.text(), "partial");
        assert_eq!(message.id.as_deref(), Some("msg-1"));
    }

    #[test]
    fn stream_error_final_skips_already_finalized_turn() {
        let mut message = partial_message_with_tool_calls();
        message.streaming = false;
        message.stop_reason = Some("stop".to_string());
        assert!(stream_error_final(message, &AgentError::Cancelled).is_none());
    }

    /// Drain event-port emits until a terminal `done`/`error` event,
    /// returning each event as its JSON representation.
    async fn recv_events_until_terminal(probe_rx: &ProbeReceiver) -> Vec<serde_json::Value> {
        let mut events = Vec::new();
        loop {
            let (_ctx, value) = probe_rx.recv().await.unwrap();
            let json = value.to_json();
            let terminal = matches!(json["type"].as_str(), Some("done") | Some("error"));
            events.push(json);
            if terminal {
                return events;
            }
        }
    }

    fn event_types(events: &[serde_json::Value]) -> Vec<&str> {
        events
            .iter()
            .map(|e| e["type"].as_str().unwrap_or("<untyped>"))
            .collect()
    }

    #[cfg(feature = "openai")]
    #[tokio::test]
    async fn openai_stream_emits_typed_event_sequence() {
        let (ma, chat_id, probe_rx) = setup_chat_probe_on(PORT_EVENT).await;

        let chunks: Vec<Result<Option<String>, AgentError>> = vec![
            Ok(Some(
                r#"{"choices":[{"index":0,"delta":{"content":"Hel"},"finish_reason":null}]}"#
                    .to_string(),
            )),
            // First fragment of the tool call carries id/name plus a partial
            // argument string; the second completes the arguments.
            Ok(Some(
                r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_a","function":{"name":"get_weather","arguments":"{\"city\":"}}]},"finish_reason":null}]}"#
                    .to_string(),
            )),
            Ok(Some(
                r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"Tokyo\"}"}}]},"finish_reason":"tool_calls"}]}"#
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

        let events = recv_events_until_terminal(&probe_rx).await;
        assert_eq!(
            event_types(&events),
            vec![
                "start",
                "text_delta",
                "tool_call_start",
                "tool_call_delta",
                "tool_call_delta",
                "tool_call_end",
                "done",
            ]
        );

        // Incremental events carry streaming=true partials.
        assert_eq!(events[1]["delta"], serde_json::json!("Hel"));
        assert_eq!(events[1]["partial"]["content"], serde_json::json!("Hel"));
        assert_eq!(events[1]["partial"]["streaming"], serde_json::json!(true));

        // The tool call finalizes with parsed arguments before Done.
        assert_eq!(events[5]["index"], serde_json::json!(0));
        assert_eq!(
            events[5]["tool_call"]["function"]["name"],
            serde_json::json!("get_weather")
        );
        assert_eq!(
            events[5]["tool_call"]["function"]["parameters"],
            serde_json::json!({"city": "Tokyo"})
        );
        assert_eq!(events[5]["partial"]["streaming"], serde_json::json!(true));

        // Done carries the same final message as the message port.
        let done = &events[6]["message"];
        assert_eq!(done["content"], serde_json::json!("Hel"));
        assert_eq!(done["stop_reason"], serde_json::json!("tool_use"));
        assert!(done["streaming"].is_null(), "final must not be streaming");
        assert_eq!(
            done["tool_calls"][0]["function"]["name"],
            serde_json::json!("get_weather")
        );

        ma.quit();
    }

    #[cfg(feature = "openai")]
    #[tokio::test]
    async fn openai_stream_suppresses_partials_when_config_disabled() {
        let (ma, chat_id, probe_rx) = setup_chat_with_probe().await;

        let chunks: Vec<Result<Option<String>, AgentError>> = vec![
            Ok(Some(
                r#"{"choices":[{"index":0,"delta":{"content":"Hel"},"finish_reason":null}]}"#
                    .to_string(),
            )),
            Ok(Some(
                r#"{"choices":[{"index":0,"delta":{"content":"lo"},"finish_reason":"stop"}]}"#
                    .to_string(),
            )),
            Ok(None),
        ];
        let mut message = streaming_seed_message("m1");

        {
            let agent = ma.get_agent(&chat_id).unwrap();
            let mut guard = agent.lock().await;
            let chat = guard.as_agent_mut::<ChatAgent>().unwrap();
            chat.set_config(
                CONFIG_EMIT_PARTIAL_MESSAGES.to_string(),
                AgentValue::boolean(false),
            )
            .unwrap();
            chat.run_openai_stream(
                &AgentContext::new(),
                futures::stream::iter(chunks),
                &mut message,
            )
            .await
            .unwrap();
        }

        // With partials suppressed the very first message-port emit is
        // already the final one.
        let (_ctx, value) = probe_rx.recv().await.unwrap();
        let msg = value.as_message().unwrap();
        assert!(!msg.streaming);
        assert_eq!(msg.text(), "Hello");
        assert_eq!(msg.stop_reason.as_deref(), Some("stop"));

        ma.quit();
    }

    #[cfg(feature = "claude")]
    #[tokio::test]
    async fn claude_stream_emits_typed_event_sequence() {
        let (ma, chat_id, probe_rx) = setup_chat_probe_on(PORT_EVENT).await;

        let events_in: Vec<Result<claude_client::ClaudeStreamEvent, AgentError>> = [
            r#"{"type":"message_start","message":{"id":"msg_1","usage":{"input_tokens":5,"output_tokens":1}}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Let me think"}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"sig-"}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"abc"}}"#,
            r#"{"type":"content_block_stop","index":0}"#,
            r#"{"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Hi"}}"#,
            r#"{"type":"content_block_stop","index":1}"#,
            r#"{"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"tu_1","name":"get_weather","input":{}}}"#,
            r#"{"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"city\":\"Tokyo\"}"}}"#,
            r#"{"type":"content_block_stop","index":2}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":9}}"#,
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
                futures::stream::iter(events_in),
                &mut message,
            )
            .await
            .unwrap();
        }

        let events = recv_events_until_terminal(&probe_rx).await;
        assert_eq!(
            event_types(&events),
            vec![
                "start",
                "thinking_delta",
                "text_delta",
                "tool_call_start",
                "tool_call_delta",
                "tool_call_end",
                "done",
            ]
        );

        assert_eq!(events[1]["delta"], serde_json::json!("Let me think"));
        // Thinking-bearing partials serialize content as a block array.
        assert_eq!(
            events[1]["partial"]["content"][0]["thinking"],
            serde_json::json!("Let me think")
        );

        // Tool-call events index by position in tool_calls, not by the
        // provider's content-block index (2 here).
        assert_eq!(events[3]["index"], serde_json::json!(0));
        assert_eq!(events[4]["index"], serde_json::json!(0));
        assert_eq!(events[5]["index"], serde_json::json!(0));
        assert_eq!(
            events[5]["tool_call"]["function"]["parameters"],
            serde_json::json!({"city": "Tokyo"})
        );

        let done = &events[6]["message"];
        // Ordered blocks with the accumulated signature, ready for replay.
        assert_eq!(
            done["content"],
            serde_json::json!([
                {"type": "thinking", "thinking": "Let me think", "signature": "sig-abc"},
                {"type": "text", "text": "Hi"},
            ])
        );
        assert_eq!(done["stop_reason"], serde_json::json!("tool_use"));
        assert!(done["streaming"].is_null(), "final must not be streaming");

        ma.quit();
    }

    #[cfg(feature = "ollama")]
    #[tokio::test]
    async fn ollama_stream_emits_typed_event_sequence() {
        let (ma, chat_id, probe_rx) = setup_chat_probe_on(PORT_EVENT).await;

        let chunks: Vec<Result<ollama_client::ChatResponse, AgentError>> = [
            r#"{"model":"m","created_at":"t","message":{"role":"assistant","content":"Hel"},"done":false}"#,
            // Ollama delivers the tool call whole in a single chunk.
            r#"{"model":"m","created_at":"t","message":{"role":"assistant","content":"","tool_calls":[{"function":{"name":"get_weather","arguments":{"city":"Tokyo"}}}]},"done":false}"#,
            r#"{"model":"m","created_at":"t","message":{"role":"assistant","content":"lo"},"done":true,"done_reason":"stop"}"#,
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

        let events = recv_events_until_terminal(&probe_rx).await;
        assert_eq!(
            event_types(&events),
            vec![
                "start",
                "text_delta",
                "tool_call_start",
                "tool_call_end",
                "text_delta",
                "done",
            ]
        );

        // Even the delta arriving on the done=true chunk is a streaming partial.
        assert_eq!(events[4]["delta"], serde_json::json!("lo"));
        assert_eq!(events[4]["partial"]["streaming"], serde_json::json!(true));

        assert_eq!(
            events[3]["tool_call"]["function"]["name"],
            serde_json::json!("get_weather")
        );

        let done = &events[5]["message"];
        assert_eq!(done["content"], serde_json::json!("Hello"));
        assert_eq!(done["stop_reason"], serde_json::json!("stop"));
        assert!(done["streaming"].is_null(), "final must not be streaming");

        ma.quit();
    }

    // -- thinking_level request building --

    #[cfg(any(feature = "claude", feature = "openai", feature = "ollama"))]
    use crate::capabilities::ThinkingLevel;

    #[cfg(feature = "claude")]
    fn base_claude_request() -> claude_client::ClaudeRequest {
        claude_client::ClaudeRequest {
            model: "claude-test".to_string(),
            max_tokens: 8192,
            messages: vec![],
            system: None,
            stream: None,
            tools: None,
            thinking: None,
            output_config: None,
            temperature: Some(0.7),
            top_p: Some(0.9),
        }
    }

    #[cfg(feature = "claude")]
    #[test]
    fn claude_budget_thinking_adds_budget_and_suppresses_sampling() {
        let mut request = base_claude_request();
        apply_claude_thinking(
            &mut request,
            Some((ThinkingLevel::Medium, None)),
            Some(64_000),
        );

        assert!(matches!(
            request.thinking,
            Some(claude_client::ClaudeThinkingConfig::Enabled {
                budget_tokens: 8192
            })
        ));
        // Budget rides on top of the configured max_tokens.
        assert_eq!(request.max_tokens, 8192 + 8192);
        assert!(request.output_config.is_none());
        // Anthropic rejects sampling params alongside thinking.
        assert_eq!(request.temperature, None);
        assert_eq!(request.top_p, None);
    }

    #[cfg(feature = "claude")]
    #[test]
    fn claude_budget_thinking_reclamps_to_model_limit() {
        let mut request = base_claude_request();
        request.max_tokens = 60_000;
        apply_claude_thinking(
            &mut request,
            Some((ThinkingLevel::High, None)),
            Some(64_000),
        );

        assert!(matches!(
            request.thinking,
            Some(claude_client::ClaudeThinkingConfig::Enabled {
                budget_tokens: 16_384
            })
        ));
        assert_eq!(request.max_tokens, 64_000);
    }

    #[cfg(feature = "claude")]
    #[test]
    fn claude_budget_thinking_shrinks_budget_below_small_model_limit() {
        // models.json can cap a budget-mechanism model's max_tokens at or
        // below the level budget; Anthropic requires budget < max_tokens.
        let mut request = base_claude_request();
        apply_claude_thinking(&mut request, Some((ThinkingLevel::High, None)), Some(8192));

        assert!(matches!(
            request.thinking,
            Some(claude_client::ClaudeThinkingConfig::Enabled {
                budget_tokens: 4096
            })
        ));
        assert_eq!(request.max_tokens, 8192);
        // Thinking stays on, so sampling params are still suppressed.
        assert_eq!(request.temperature, None);
        assert_eq!(request.top_p, None);
    }

    #[cfg(feature = "claude")]
    #[test]
    fn claude_budget_thinking_degrades_to_off_when_no_budget_room() {
        // A limit too small for even the API-minimum budget (1024) plus
        // answer room must disable thinking rather than send a 400-bound
        // request.
        let mut request = base_claude_request();
        request.max_tokens = 2000;
        apply_claude_thinking(&mut request, Some((ThinkingLevel::High, None)), Some(2000));

        assert!(request.thinking.is_none());
        assert_eq!(request.max_tokens, 2000);
        // Thinking is off, so sampling params remain valid and are kept.
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.top_p, Some(0.9));
    }

    #[cfg(feature = "claude")]
    #[test]
    fn claude_budget_thinking_budget_always_below_max_tokens() {
        use ThinkingLevel::*;
        // The Anthropic invariant budget_tokens < max_tokens must hold for
        // every level/limit combination that keeps thinking enabled.
        for level in [Minimal, Low, Medium, High] {
            for limit in [2049_u32, 4096, 8192, 16_384, 64_000] {
                let mut request = base_claude_request();
                request.max_tokens = limit.min(8192);
                apply_claude_thinking(&mut request, Some((level, None)), Some(limit));
                if let Some(claude_client::ClaudeThinkingConfig::Enabled { budget_tokens }) =
                    request.thinking
                {
                    assert!(
                        budget_tokens < request.max_tokens,
                        "level {level:?} limit {limit}: budget {budget_tokens} >= max_tokens {}",
                        request.max_tokens
                    );
                }
            }
        }
    }

    #[cfg(feature = "claude")]
    #[test]
    fn claude_budget_thinking_unknown_limit_not_clamped() {
        let mut request = base_claude_request();
        apply_claude_thinking(&mut request, Some((ThinkingLevel::Minimal, None)), None);
        assert_eq!(request.max_tokens, 8192 + 1024);
    }

    #[cfg(feature = "claude")]
    #[test]
    fn claude_adaptive_thinking_sets_effort() {
        let mut request = base_claude_request();
        apply_claude_thinking(
            &mut request,
            Some((ThinkingLevel::High, Some("high".to_string()))),
            Some(64_000),
        );

        // Adaptive mode: no budget arithmetic, effort in output_config.
        assert_eq!(request.max_tokens, 8192);
        assert_eq!(request.temperature, None);
        assert_eq!(request.top_p, None);
        let json = serde_json::to_value(&request).expect("serialize");
        assert_eq!(json["thinking"], serde_json::json!({ "type": "adaptive" }));
        assert_eq!(
            json["output_config"],
            serde_json::json!({ "effort": "high" })
        );
    }

    #[cfg(feature = "claude")]
    #[test]
    fn claude_thinking_off_leaves_request_unchanged() {
        let mut request = base_claude_request();
        apply_claude_thinking(&mut request, None, Some(64_000));

        assert!(request.thinking.is_none());
        assert!(request.output_config.is_none());
        assert_eq!(request.max_tokens, 8192);
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.top_p, Some(0.9));
    }

    #[cfg(feature = "openai")]
    #[test]
    fn openai_thinking_sets_reasoning_effort() {
        let mut request = serde_json::json!({ "model": "gpt-5" });
        apply_openai_thinking(
            &mut request,
            &Some((ThinkingLevel::Low, Some("low".to_string()))),
        );
        assert_eq!(request["reasoning_effort"], serde_json::json!("low"));
    }

    #[cfg(feature = "openai")]
    #[test]
    fn openai_thinking_off_by_default() {
        let mut request = serde_json::json!({ "model": "gpt-5" });
        apply_openai_thinking(&mut request, &None);
        assert!(request.get("reasoning_effort").is_none());

        // A registry entry without a provider-side value has nothing to send.
        apply_openai_thinking(&mut request, &Some((ThinkingLevel::Low, None)));
        assert!(request.get("reasoning_effort").is_none());
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_thinking_sets_think_flag() {
        let mut request = serde_json::json!({ "model": "qwen3" });
        apply_ollama_thinking(&mut request, &Some((ThinkingLevel::Medium, None)));
        assert_eq!(request["think"], serde_json::json!(true));
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_thinking_off_by_default() {
        let mut request = serde_json::json!({ "model": "qwen3" });
        apply_ollama_thinking(&mut request, &None);
        assert!(request.get("think").is_none());
    }
}
