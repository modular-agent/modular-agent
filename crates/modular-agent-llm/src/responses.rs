use im::vector;
use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AgentValueMap,
    AsAgent, Message, MessageEvent, ModularAgent, ToolCall, ToolCallFunction, Usage, async_trait,
    modular_agent,
};

use crate::openai_client;
use crate::provider::{ModelIdentifier, ProviderKind};
use crate::retry::RetryPolicy;

const CATEGORY: &str = "LLM";

const PORT_EVENT: &str = "event";
const PORT_MESSAGE: &str = "message";
const PORT_RESPONSE: &str = "response";
const PORT_RESET: &str = "reset";

const CONFIG_EMIT_PARTIAL_MESSAGES: &str = "emit_partial_messages";
const CONFIG_MAX_RETRIES: &str = "max_retries";
const CONFIG_MAX_TOKENS: &str = "max_tokens";
const CONFIG_MODEL: &str = "model";
const CONFIG_OPTIONS: &str = "options";
const CONFIG_RETRY_BASE_DELAY_MS: &str = "retry_base_delay_ms";
const CONFIG_STREAM: &str = "stream";
const CONFIG_TEMPERATURE: &str = "temperature";
const CONFIG_TIMEOUT_SECS: &str = "timeout_secs";
const CONFIG_TOOLS: &str = "tools";
const CONFIG_TOP_P: &str = "top_p";
const CONFIG_USE_CONVERSATION_STATE: &str = "use_conversation_state";

const DEFAULT_MODEL: &str = "openai/gpt-5-mini";

/// Responses Agent using OpenAI Responses API.
///
/// The Responses API is OpenAI's new API primitive that provides:
/// - Server-side conversation state via `previous_response_id`
/// - Built-in tools (web_search, file_search, code_interpreter) - future support
/// - Semantic streaming events
/// - Better performance with reasoning models
///
/// # Configuration
/// - `model`: Model name (default: "gpt-5-mini")
/// - `stream`: Enable streaming mode
/// - `use_conversation_state`: Use server-side conversation state
/// - `tools`: Tool patterns to enable (regex, newline-separated)
/// - `max_tokens`: Maximum output tokens (sent as `max_output_tokens`). `0`
///   omits it and uses the API default; a positive value is clamped to the
///   model's known limit (unknown models are left unclamped).
/// - `temperature`: Sampling temperature (-1: use API default)
/// - `top_p`: Nucleus sampling parameter (-1: use API default)
/// - `options`: Additional request options as JSON
/// - `max_retries`: Maximum automatic retries for retryable errors such as
///   rate limits, server overload, and timeouts (default: 2)
/// - `retry_base_delay_ms`: Base delay for exponential backoff between
///   retries; a server-provided Retry-After takes precedence (default: 1000)
/// - `timeout_secs`: Per-attempt deadline in seconds; for streaming it covers
///   stream establishment only (default: 300, 0 = disabled)
/// - `emit_partial_messages`: When false, skip the accumulated partial
///   (streaming = true) emissions on the `message` port; the final message is
///   still emitted there. The `event` port is unaffected (default: true)
///
/// # Ports
/// - Input `message`: Message or array of messages to send
/// - Input `reset`: Any value to reset conversation state
/// - Output `message`: Assistant's response message. A mid-stream failure or
///   a cancelled flow (`ModularAgent::abort_context`) still emits one final
///   message: same id, the partial content so far, and `stop_reason` "error"
///   or "aborted"
/// - Output `response`: Raw API response
/// - Output `event`: Typed stream events (`MessageEvent` object with a `type`
///   field: `start`, `text_delta`, `tool_call_start`, `tool_call_delta`,
///   `tool_call_end`, `done`, `error`). Streaming turns emit the full
///   sequence; non-streaming turns emit a single `done`. `done`/`error` are
///   emitted after the corresponding final message on the `message` port
#[modular_agent(
    title = "Responses",
    category = CATEGORY,
    inputs = [PORT_MESSAGE, PORT_RESET],
    outputs = [PORT_MESSAGE, PORT_RESPONSE, PORT_EVENT],
    string_config(name = CONFIG_MODEL, default = DEFAULT_MODEL),
    boolean_config(name = CONFIG_STREAM, title = "Stream"),
    boolean_config(name = CONFIG_USE_CONVERSATION_STATE, title = "Use Conversation State"),
    boolean_config(name = CONFIG_EMIT_PARTIAL_MESSAGES, title = "Emit Partial Messages", default = true, description = "Re-send partial messages on the message port while streaming", detail),
    text_config(name = CONFIG_TOOLS),
    integer_config(name = CONFIG_MAX_TOKENS, title = "Max Tokens", default = 0, description = "0: use API default", detail),
    number_config(name = CONFIG_TEMPERATURE, title = "Temperature", default = -1.0, description = "-1: use API default (0.0-2.0)", detail),
    number_config(name = CONFIG_TOP_P, title = "Top P", default = -1.0, description = "-1: use API default (0.0-1.0)", detail),
    object_config(name = CONFIG_OPTIONS, title = "Options", description = "Additional request options as JSON", detail),
    integer_config(name = CONFIG_MAX_RETRIES, title = "Max Retries", default = 2, description = "Automatic retries for retryable errors", detail),
    integer_config(name = CONFIG_RETRY_BASE_DELAY_MS, title = "Retry Base Delay (ms)", default = 1000, description = "Base delay for exponential backoff", detail),
    integer_config(name = CONFIG_TIMEOUT_SECS, title = "Timeout (secs)", default = 300, description = "Per-attempt deadline; 0: disabled", detail),
    hint(width = 2, height = 2),
)]
pub struct ResponsesAgent {
    data: AgentData,
    openai_manager: openai_client::OpenAIManager,
    last_response_id: Option<String>,
}

#[async_trait]
impl AsAgent for ResponsesAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            openai_manager: openai_client::OpenAIManager::new(),
            last_response_id: None,
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        self.last_response_id = None;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        self.last_response_id = None;
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        // Handle reset port
        if port == PORT_RESET {
            self.last_response_id = None;
            return Ok(());
        }

        // An aborted flow feeds synthetic "Operation aborted" tool results
        // back into this agent; without this guard each such trigger would
        // issue one more full-price LLM request.
        if ctx.is_cancelled() {
            return Err(AgentError::Cancelled);
        }

        let config = self.configs()?;
        let config_model = config.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        let model_id = ModelIdentifier::parse(&config_model)?;
        if model_id.provider != ProviderKind::OpenAI {
            return Err(AgentError::InvalidConfig(
                "ResponsesAgent only supports OpenAI models".into(),
            ));
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

        // If the last message isn't a user/tool message, just return
        let role = &messages.last().unwrap().as_message().unwrap().role;
        if role != "user" && role != "tool" {
            return Ok(());
        }

        // Get configs
        let config_options = config.get_object_or_default(CONFIG_OPTIONS);
        let config_tools = config.get_string_or_default(CONFIG_TOOLS);
        let use_stream = config.get_bool_or_default(CONFIG_STREAM);
        let use_conversation_state = config.get_bool_or_default(CONFIG_USE_CONVERSATION_STATE);
        let max_tokens = config.get_integer_or_default(CONFIG_MAX_TOKENS);
        let temperature = config.get_number_or_default(CONFIG_TEMPERATURE);
        let top_p = config.get_number_or_default(CONFIG_TOP_P);

        // Snapshot retry/timeout configs once per turn so a mid-turn config
        // change cannot alter an in-flight retry loop.
        let retry = RetryPolicy::from_configs(
            config.get_integer_or_default(CONFIG_MAX_RETRIES),
            config.get_integer_or_default(CONFIG_RETRY_BASE_DELAY_MS),
            config.get_integer_or_default(CONFIG_TIMEOUT_SECS),
        );

        // Resolve the model's registry entry once per turn; a `None`
        // max_tokens (unknown model) leaves a configured max_tokens unclamped.
        let caps = crate::capabilities::resolve_entry(&model_id);
        let model_max_tokens = caps.max_tokens;

        // Single cross-provider normalization boundary (P-02); see ChatAgent
        // for the image-demotion rationale.
        let messages = crate::prepare::prepare_messages(
            &messages,
            model_id.provider,
            caps.image_input == Some(false),
        );

        self.process_response(
            ctx,
            messages,
            &model_id.model_name,
            config_options,
            config_tools,
            use_stream,
            use_conversation_state,
            max_tokens,
            model_max_tokens,
            temperature,
            top_p,
            retry,
        )
        .await
    }
}

impl ResponsesAgent {
    #[allow(clippy::too_many_arguments)]
    async fn process_response(
        &mut self,
        ctx: AgentContext,
        messages: im::Vector<AgentValue>,
        model_name: &str,
        config_options: AgentValueMap<String, AgentValue>,
        config_tools: String,
        use_stream: bool,
        use_conversation_state: bool,
        max_tokens: i64,
        model_max_tokens: Option<u32>,
        temperature: f64,
        top_p: f64,
        retry: RetryPolicy,
    ) -> Result<(), AgentError> {
        use modular_agent_core::tool::list_tool_infos_patterns;

        let client = self.openai_manager.get_client(self.ma())?;

        // Build input from messages
        let input = openai_client::messages_to_response_input(&messages)?;

        // Build tools array
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
                .map(|info| {
                    serde_json::json!({
                        "type": "function",
                        "name": info.name,
                        "description": if info.description.is_empty() {
                            serde_json::Value::Null
                        } else {
                            serde_json::Value::String(info.description)
                        },
                        "parameters": info.parameters,
                    })
                })
                .collect()
        };

        // Build request
        let mut request = serde_json::json!({
            "model": model_name,
            "input": input,
            "stream": use_stream,
        });

        // Add previous_response_id for conversation continuity
        if use_conversation_state && let Some(prev_id) = &self.last_response_id {
            request["previous_response_id"] = serde_json::Value::String(prev_id.clone());
        }

        // Add tools if configured
        if !tools_json.is_empty() {
            request["tools"] = serde_json::Value::Array(tools_json);
        }

        // Merge options
        openai_client::merge_options(&mut request, &config_options)?;
        if let Some(v) = crate::capabilities::clamp_max_tokens(max_tokens, model_max_tokens) {
            request["max_output_tokens"] = v.into();
        }
        if temperature >= 0.0 {
            request["temperature"] = temperature.into();
        }
        if top_p >= 0.0 {
            request["top_p"] = top_p.into();
        }

        let id = uuid::Uuid::new_v4().to_string();
        if use_stream {
            self.process_streaming(ctx, &client, &request, &id, use_conversation_state, retry)
                .await
        } else {
            self.process_non_streaming(ctx, &client, &request, &id, use_conversation_state, retry)
                .await
        }
    }

    async fn process_non_streaming(
        &mut self,
        ctx: AgentContext,
        client: &openai_client::OpenAIClient,
        request: &serde_json::Value,
        id: &str,
        use_conversation_state: bool,
        retry: RetryPolicy,
    ) -> Result<(), AgentError> {
        let url = client.responses_url();
        let response: serde_json::Value = crate::chat::request_or_cancelled(
            ctx.cancel_token(),
            retry.run(|| client.post_json(&url, request)),
        )
        .await?;

        // Store response ID for conversation continuity
        if use_conversation_state && let Some(resp_id) = response.get("id").and_then(|v| v.as_str())
        {
            self.last_response_id = Some(resp_id.to_string());
        }

        // Convert response to message
        let output = response
            .get("output")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut message = openai_client::response_output_to_message(&output)?;
        message.id = Some(id.to_string());
        message.stop_reason = stop_reason_from_response(&response, message.tool_calls.is_some());
        message.usage = usage_from_response(&response);

        // Output message before the Done event: ChatAgent's ordering, so a
        // `done`-triggered consumer always sees the final message routed.
        self.output(
            ctx.clone(),
            PORT_MESSAGE.to_string(),
            message.clone().into(),
        )
        .await?;

        // Non-streaming turns still surface completion on the event port as a
        // single Done, so event consumers work regardless of the stream config.
        self.emit_event(&ctx, MessageEvent::Done { message })
            .await?;

        // Output raw response
        let out_response = AgentValue::from_serialize(&response)?;
        self.output(ctx, PORT_RESPONSE.to_string(), out_response)
            .await?;

        Ok(())
    }

    async fn process_streaming(
        &mut self,
        ctx: AgentContext,
        client: &openai_client::OpenAIClient,
        request: &serde_json::Value,
        id: &str,
        use_conversation_state: bool,
        retry: RetryPolicy,
    ) -> Result<(), AgentError> {
        let url = client.responses_url();
        // Retry covers stream establishment only: once chunks have been
        // emitted downstream they cannot be rolled back, so any failure
        // after this point must propagate instead of being retried.
        let stream = crate::chat::request_or_cancelled(
            ctx.cancel_token(),
            retry.run(|| client.post_stream(&url, request)),
        )
        .await?;

        let mut message = Message::assistant(String::new());
        message.id = Some(id.to_string());
        // Partial emits during streaming must be skipped by CallToolMessageAgent so tools
        // are executed only once against the final message.
        message.streaming = true;

        if let Err(e) = self
            .run_stream(&ctx, stream, &mut message, use_conversation_state)
            .await
        {
            // Mark the mid-stream failure (stop_reason "error") or
            // cancellation (stop_reason "aborted") on a final same-id emit so
            // message history replaces the dangling partial with a terminated
            // one. Best effort: the stream error is the more useful signal,
            // so an emit failure here must not mask it.
            if let Some(message) = crate::chat::stream_error_final(message, &e) {
                let _ = self
                    .output(
                        ctx.clone(),
                        PORT_MESSAGE.to_string(),
                        message.clone().into(),
                    )
                    .await;
                let _ = self
                    .emit_event(
                        &ctx,
                        MessageEvent::Error {
                            message,
                            error: e.to_string(),
                        },
                    )
                    .await;
            }
            return Err(e);
        }

        Ok(())
    }

    /// Consume an established Responses API SSE stream, emitting partial
    /// messages and exactly one finalized message. Extracted so the caller
    /// can intercept a mid-stream Err and emit an error-marked final message.
    async fn run_stream(
        &mut self,
        ctx: &AgentContext,
        mut stream: impl futures::Stream<Item = Result<Option<String>, AgentError>> + Unpin,
        message: &mut Message,
        use_conversation_state: bool,
    ) -> Result<(), AgentError> {
        // get_bool_or with an explicit true keeps the fallback aligned with
        // the declared config default when the key is absent (old spec not
        // yet reconciled).
        let emit_partial = self
            .configs()?
            .get_bool_or(CONFIG_EMIT_PARTIAL_MESSAGES, true);

        let mut content = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut current_tool_name: Option<String> = None;
        let mut current_tool_call_id: Option<String> = None;
        let mut current_tool_arguments = String::new();

        // Exactly one Start per streamed turn, before any delta arrives.
        self.emit_event(
            ctx,
            MessageEvent::Start {
                partial: message.clone(),
            },
        )
        .await?;

        while let Some(res) =
            crate::chat::next_or_cancelled(&mut stream, ctx.cancel_token()).await?
        {
            let Some(data) = res? else {
                continue; // [DONE] sentinel
            };
            let event: openai_client::ResponseStreamEvent =
                serde_json::from_str(&data).unwrap_or(openai_client::ResponseStreamEvent::Other);

            match event {
                openai_client::ResponseStreamEvent::OutputTextDelta { delta } => {
                    content.push_str(&delta);
                    message.content = content.clone().into();
                    self.emit_event(
                        ctx,
                        MessageEvent::TextDelta {
                            delta,
                            partial: message.clone(),
                        },
                    )
                    .await?;
                    if emit_partial {
                        self.output(
                            ctx.clone(),
                            PORT_MESSAGE.to_string(),
                            message.clone().into(),
                        )
                        .await?;
                    }
                }
                openai_client::ResponseStreamEvent::FunctionCallArgumentsDelta { delta } => {
                    current_tool_arguments.push_str(&delta);
                    // A stray delta without a preceding output_item.added has
                    // no tool call to attribute to, so no event for it.
                    if current_tool_name.is_some() {
                        self.emit_event(
                            ctx,
                            MessageEvent::ToolCallDelta {
                                index: tool_calls.len(),
                                delta,
                                partial: message.clone(),
                            },
                        )
                        .await?;
                    }
                }
                openai_client::ResponseStreamEvent::OutputItemAdded { item } => {
                    if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                        current_tool_name = Some(name.to_string());
                        current_tool_call_id = item
                            .get("call_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        current_tool_arguments.clear();
                        // The in-flight call lands at tool_calls.len() once
                        // finalized, so Start/Delta/End share that index.
                        self.emit_event(
                            ctx,
                            MessageEvent::ToolCallStart {
                                index: tool_calls.len(),
                                partial: message.clone(),
                            },
                        )
                        .await?;
                    }
                }
                openai_client::ResponseStreamEvent::OutputItemDone { .. } => {
                    // Handle completed function call
                    if let Some(name) = current_tool_name.take() {
                        let (parameters, parse_error) =
                            crate::json_repair::parse_tool_arguments(&current_tool_arguments);
                        let tool_call = ToolCall {
                            function: ToolCallFunction {
                                id: current_tool_call_id.take(),
                                name,
                                parameters,
                                parse_error,
                            },
                        };
                        let index = tool_calls.len();
                        tool_calls.push(tool_call.clone());
                        message.tool_calls = Some(tool_calls.clone().into());
                        current_tool_arguments.clear();

                        self.emit_event(
                            ctx,
                            MessageEvent::ToolCallEnd {
                                index,
                                tool_call,
                                partial: message.clone(),
                            },
                        )
                        .await?;
                        if emit_partial {
                            self.output(
                                ctx.clone(),
                                PORT_MESSAGE.to_string(),
                                message.clone().into(),
                            )
                            .await?;
                        }
                    }
                }
                openai_client::ResponseStreamEvent::Incomplete { response }
                | openai_client::ResponseStreamEvent::Failed { response } => {
                    // Terminal but unsuccessful; record the reason and let the
                    // trailing guard below emit the single final message.
                    // Usage may legitimately be absent here, leaving None.
                    message.stop_reason =
                        stop_reason_from_response(&response, !tool_calls.is_empty());
                    message.usage = usage_from_response(&response);
                }
                openai_client::ResponseStreamEvent::Completed { response } => {
                    // Emit the final, non-streaming message so downstream agents act on it once.
                    message.content = content.clone().into();
                    if !tool_calls.is_empty() {
                        message.tool_calls = Some(tool_calls.clone().into());
                    }
                    message.streaming = false;
                    message.stop_reason =
                        stop_reason_from_response(&response, !tool_calls.is_empty());
                    message.usage = usage_from_response(&response);
                    // Final message first, Done after (ChatAgent's ordering),
                    // so a `done`-triggered consumer sees the final routed.
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

                    // Store response ID for conversation continuity
                    if use_conversation_state
                        && let Some(resp_id) = response.get("id").and_then(|v| v.as_str())
                    {
                        self.last_response_id = Some(resp_id.to_string());
                    }

                    let out_response = AgentValue::from_serialize(&response)?;
                    self.output(ctx.clone(), PORT_RESPONSE.to_string(), out_response)
                        .await?;
                }
                openai_client::ResponseStreamEvent::Other => {}
            }
        }

        // The Responses API can terminate a stream with response.incomplete or
        // response.failed instead of response.completed (their stop_reason is
        // recorded in the arms above). Guarantee exactly one streaming=false
        // final emit per turn so accumulated tool_calls still run.
        if message.streaming {
            message.content = content.into();
            if !tool_calls.is_empty() {
                message.tool_calls = Some(tool_calls.into());
            }
            message.streaming = false;
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

        Ok(())
    }

    /// Emit a typed stream event on the `event` port. Unlike message-port
    /// partials, event emission is unconditional: `emit_partial_messages`
    /// only gates the legacy accumulated-Message re-sends.
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
}

/// Map a terminal Responses API `response` object to a normalized stop_reason.
///
/// The Responses API reports completion via `status` plus
/// `incomplete_details.reason` instead of a finish_reason; unknown provider
/// values pass through unchanged.
fn stop_reason_from_response(response: &serde_json::Value, has_tool_calls: bool) -> Option<String> {
    let status = response.get("status").and_then(|v| v.as_str())?;
    let reason = match status {
        "completed" => {
            if has_tool_calls {
                "tool_use"
            } else {
                "stop"
            }
        }
        "incomplete" => {
            match response
                .get("incomplete_details")
                .and_then(|d| d.get("reason"))
                .and_then(|v| v.as_str())
            {
                Some("max_output_tokens") => "length",
                Some("content_filter") => "error",
                Some(other) => other,
                None => "incomplete",
            }
        }
        "failed" => "error",
        other => other,
    };
    Some(reason.to_string())
}

/// Extract normalized usage from a terminal Responses API `response` object.
///
/// Like Chat Completions, the API's `input_tokens` INCLUDES cached tokens,
/// so cached is subtracted out to match the Anthropic-style accounting used
/// by `Usage.input_tokens`. Lenient: `None` only when the `usage` object is
/// absent entirely; missing individual fields default to 0.
fn usage_from_response(response: &serde_json::Value) -> Option<Usage> {
    let usage = response.get("usage")?.as_object()?;
    let input = usage
        .get("input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cached = usage
        .get("input_tokens_details")
        .and_then(|d| d.get("cached_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Some(Usage {
        input_tokens: input.saturating_sub(cached),
        output_tokens: output,
        cache_read_tokens: cached,
        cache_write_tokens: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use modular_agent_core::ConnectionSpec;
    use modular_agent_core::test_utils::{ProbeReceiver, TestProbeAgent, probe_receiver};

    /// Build a running patch with a ResponsesAgent whose `port` output
    /// feeds a probe, so stream-loop emits can be observed end to end.
    async fn setup_responses_with_probe(port: &str) -> (ModularAgent, String, ProbeReceiver) {
        let ma = ModularAgent::init().unwrap();
        ma.ready().await.unwrap();

        let patch_id = ma.new_patch().unwrap();
        let responses_def = ma.get_agent_definition(ResponsesAgent::DEF_NAME).unwrap();
        let responses_id = ma
            .add_agent(patch_id.clone(), responses_def.to_spec())
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
                source: responses_id.clone(),
                source_handle: port.into(),
                target: probe_id.clone(),
                target_handle: "value".into(),
            },
        )
        .await
        .unwrap();
        ma.start_patch(&patch_id).await.unwrap();
        let probe_rx = probe_receiver(&ma, &probe_id).await.unwrap();

        (ma, responses_id, probe_rx)
    }

    #[tokio::test]
    async fn responses_stream_incomplete_status_lands_on_final_message() {
        let (ma, responses_id, probe_rx) = setup_responses_with_probe(PORT_MESSAGE).await;

        // An incomplete turn: text deltas, then response.incomplete instead
        // of response.completed. The trailing guard must still emit exactly
        // one streaming=false final carrying the mapped stop_reason.
        let chunks: Vec<Result<Option<String>, AgentError>> = vec![
            Ok(Some(
                r#"{"type":"response.output_text.delta","delta":"Hi"}"#.to_string(),
            )),
            Ok(Some(
                r#"{"type":"response.incomplete","response":{"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"}}}"#
                    .to_string(),
            )),
            Ok(None), // [DONE] sentinel
        ];
        let mut message = Message::assistant(String::new());
        message.id = Some("m1".to_string());
        message.streaming = true;

        {
            let agent = ma.get_agent(&responses_id).unwrap();
            let mut guard = agent.lock().await;
            let responses = guard.as_agent_mut::<ResponsesAgent>().unwrap();
            responses
                .run_stream(
                    &AgentContext::new(),
                    futures::stream::iter(chunks),
                    &mut message,
                    false,
                )
                .await
                .unwrap();
        }

        let final_msg = loop {
            let (_ctx, value) = probe_rx.recv().await.unwrap();
            let msg = value.as_message().unwrap().clone();
            if !msg.streaming {
                break msg;
            }
            assert_eq!(msg.stop_reason, None, "partial emits must keep None");
        };
        assert_eq!(final_msg.stop_reason.as_deref(), Some("length"));
        assert_eq!(final_msg.text(), "Hi");
        assert_eq!(final_msg.id.as_deref(), Some("m1"));
        // No usage object on this terminal event, so it legitimately stays None
        assert_eq!(final_msg.usage, None);

        ma.quit();
    }

    #[tokio::test]
    async fn responses_stream_cancel_returns_cancelled_and_builds_aborted_final() {
        let (ma, responses_id, probe_rx) = setup_responses_with_probe(PORT_MESSAGE).await;

        // One text delta, then a tail that fires the flow token when polled
        // and stays pending — a deterministic mid-stream abort.
        let token = modular_agent_core::CancellationToken::new();
        let fire = token.clone();
        let chunks: Vec<Result<Option<String>, AgentError>> = vec![Ok(Some(
            r#"{"type":"response.output_text.delta","delta":"partial"}"#.to_string(),
        ))];
        let stream = {
            use futures::StreamExt;
            futures::stream::iter(chunks).chain(futures::stream::poll_fn(move |_| {
                fire.cancel();
                std::task::Poll::Pending
            }))
        };
        let mut message = Message::assistant(String::new());
        message.id = Some("m1".to_string());
        message.streaming = true;

        {
            let agent = ma.get_agent(&responses_id).unwrap();
            let mut guard = agent.lock().await;
            let responses = guard.as_agent_mut::<ResponsesAgent>().unwrap();
            let ctx = AgentContext::new().with_cancel_token(token);
            let err = responses
                .run_stream(&ctx, stream, &mut message, false)
                .await
                .unwrap_err();
            assert!(matches!(err, AgentError::Cancelled));

            // Same sequence as process_streaming: the aborted-marked final
            // replaces the dangling partial in message history.
            let final_msg =
                crate::chat::stream_error_final(message, &err).expect("should build final");
            assert_eq!(final_msg.stop_reason.as_deref(), Some("aborted"));
            assert_eq!(final_msg.text(), "partial");
            assert_eq!(final_msg.id.as_deref(), Some("m1"));
            responses
                .output(ctx, PORT_MESSAGE.to_string(), final_msg.into())
                .await
                .unwrap();
        }

        let final_msg = loop {
            let (_ctx, value) = probe_rx.recv().await.unwrap();
            let msg = value.as_message().unwrap().clone();
            if !msg.streaming {
                break msg;
            }
        };
        assert_eq!(final_msg.stop_reason.as_deref(), Some("aborted"));
        assert_eq!(final_msg.text(), "partial");
        assert_eq!(final_msg.id.as_deref(), Some("m1"));

        ma.quit();
    }

    #[tokio::test]
    async fn responses_stream_completed_usage_lands_on_final_message() {
        let (ma, responses_id, probe_rx) = setup_responses_with_probe(PORT_MESSAGE).await;

        let chunks: Vec<Result<Option<String>, AgentError>> = vec![
            Ok(Some(
                r#"{"type":"response.output_text.delta","delta":"Hi"}"#.to_string(),
            )),
            Ok(Some(
                r#"{"type":"response.completed","response":{"id":"resp_1","status":"completed","output":[],"usage":{"input_tokens":100,"output_tokens":20,"input_tokens_details":{"cached_tokens":60}}}}"#
                    .to_string(),
            )),
            Ok(None), // [DONE] sentinel
        ];
        let mut message = Message::assistant(String::new());
        message.id = Some("m1".to_string());
        message.streaming = true;

        {
            let agent = ma.get_agent(&responses_id).unwrap();
            let mut guard = agent.lock().await;
            let responses = guard.as_agent_mut::<ResponsesAgent>().unwrap();
            responses
                .run_stream(
                    &AgentContext::new(),
                    futures::stream::iter(chunks),
                    &mut message,
                    false,
                )
                .await
                .unwrap();
        }

        let final_msg = loop {
            let (_ctx, value) = probe_rx.recv().await.unwrap();
            let msg = value.as_message().unwrap().clone();
            if !msg.streaming {
                break msg;
            }
            assert_eq!(msg.usage, None, "partial emits must not carry usage");
        };
        assert_eq!(final_msg.stop_reason.as_deref(), Some("stop"));
        assert_eq!(
            final_msg.usage,
            Some(Usage {
                input_tokens: 40,
                output_tokens: 20,
                cache_read_tokens: 60,
                cache_write_tokens: 0,
            })
        );

        ma.quit();
    }

    #[tokio::test]
    async fn responses_stream_emits_typed_event_sequence() {
        let (ma, responses_id, probe_rx) = setup_responses_with_probe(PORT_EVENT).await;

        // A streamed turn with text and one tool call must produce the full
        // event contract: Start, TextDelta, ToolCallStart, ToolCallDelta,
        // ToolCallEnd, Done — in that order.
        let chunks: Vec<Result<Option<String>, AgentError>> = vec![
            Ok(Some(
                r#"{"type":"response.output_text.delta","delta":"Hi"}"#.to_string(),
            )),
            Ok(Some(
                r#"{"type":"response.output_item.added","item":{"type":"function_call","name":"get_weather","call_id":"call_1"}}"#
                    .to_string(),
            )),
            Ok(Some(
                r#"{"type":"response.function_call_arguments.delta","delta":"{\"city\":\"Tokyo\"}"}"#
                    .to_string(),
            )),
            Ok(Some(
                r#"{"type":"response.output_item.done","item":{}}"#.to_string(),
            )),
            Ok(Some(
                r#"{"type":"response.completed","response":{"id":"resp_1","status":"completed","output":[]}}"#
                    .to_string(),
            )),
            Ok(None), // [DONE] sentinel
        ];
        let mut message = Message::assistant(String::new());
        message.id = Some("m1".to_string());
        message.streaming = true;

        {
            let agent = ma.get_agent(&responses_id).unwrap();
            let mut guard = agent.lock().await;
            let responses = guard.as_agent_mut::<ResponsesAgent>().unwrap();
            responses
                .run_stream(
                    &AgentContext::new(),
                    futures::stream::iter(chunks),
                    &mut message,
                    false,
                )
                .await
                .unwrap();
        }

        let mut events = Vec::new();
        for _ in 0..6 {
            let (_ctx, value) = probe_rx.recv().await.unwrap();
            events.push(value);
        }
        let types: Vec<&str> = events.iter().map(|v| v.get_str("type").unwrap()).collect();
        assert_eq!(
            types,
            vec![
                "start",
                "text_delta",
                "tool_call_start",
                "tool_call_delta",
                "tool_call_end",
                "done",
            ]
        );

        let start_partial = events[0].get("partial").unwrap();
        assert_eq!(start_partial.get_str("role"), Some("assistant"));
        assert_eq!(start_partial.get_bool("streaming"), Some(true));

        assert_eq!(events[1].get_str("delta"), Some("Hi"));
        assert_eq!(
            events[1].get("partial").unwrap().get_str("content"),
            Some("Hi")
        );

        assert_eq!(events[2].get("index").unwrap().as_i64(), Some(0));
        assert_eq!(events[3].get_str("delta"), Some(r#"{"city":"Tokyo"}"#));
        assert_eq!(
            events[4]
                .get("tool_call")
                .unwrap()
                .get("function")
                .unwrap()
                .get_str("name"),
            Some("get_weather")
        );

        let done_msg = events[5].get("message").unwrap();
        // Message serialization omits `streaming` when false, so the final
        // message must lack the key entirely rather than carry `false`.
        assert_eq!(done_msg.get_bool("streaming"), None);
        assert_eq!(done_msg.get_str("content"), Some("Hi"));
        assert_eq!(done_msg.get_str("stop_reason"), Some("tool_use"));

        ma.quit();
    }

    #[tokio::test]
    async fn responses_stream_emit_partial_messages_false_skips_partials() {
        let (ma, responses_id, probe_rx) = setup_responses_with_probe(PORT_MESSAGE).await;

        let chunks: Vec<Result<Option<String>, AgentError>> = vec![
            Ok(Some(
                r#"{"type":"response.output_text.delta","delta":"Hi"}"#.to_string(),
            )),
            Ok(Some(
                r#"{"type":"response.completed","response":{"id":"resp_1","status":"completed","output":[]}}"#
                    .to_string(),
            )),
            Ok(None), // [DONE] sentinel
        ];
        let mut message = Message::assistant(String::new());
        message.id = Some("m1".to_string());
        message.streaming = true;

        {
            let agent = ma.get_agent(&responses_id).unwrap();
            let mut guard = agent.lock().await;
            guard
                .set_config(
                    CONFIG_EMIT_PARTIAL_MESSAGES.to_string(),
                    AgentValue::boolean(false),
                )
                .unwrap();
            let responses = guard.as_agent_mut::<ResponsesAgent>().unwrap();
            responses
                .run_stream(
                    &AgentContext::new(),
                    futures::stream::iter(chunks),
                    &mut message,
                    false,
                )
                .await
                .unwrap();
        }

        // The very first message-port emit must already be the final one:
        // partials are suppressed while the final always goes out.
        let (_ctx, value) = probe_rx.recv().await.unwrap();
        let msg = value.as_message().unwrap();
        assert!(!msg.streaming);
        assert_eq!(msg.text(), "Hi");
        assert_eq!(msg.stop_reason.as_deref(), Some("stop"));

        ma.quit();
    }

    #[test]
    fn test_usage_from_response_normalizes_cached_tokens() {
        let response = serde_json::json!({
            "status": "completed",
            "usage": {
                "input_tokens": 100,
                "output_tokens": 20,
                "total_tokens": 120,
                "input_tokens_details": {"cached_tokens": 60}
            }
        });
        assert_eq!(
            usage_from_response(&response),
            Some(Usage {
                input_tokens: 40,
                output_tokens: 20,
                cache_read_tokens: 60,
                cache_write_tokens: 0,
            })
        );
    }

    #[test]
    fn test_usage_from_response_without_details() {
        let response = serde_json::json!({
            "usage": {"input_tokens": 10, "output_tokens": 5}
        });
        assert_eq!(
            usage_from_response(&response),
            Some(Usage {
                input_tokens: 10,
                output_tokens: 5,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            })
        );
    }

    #[test]
    fn test_usage_from_response_missing_usage() {
        let response = serde_json::json!({"status": "completed"});
        assert_eq!(usage_from_response(&response), None);
    }

    #[test]
    fn test_stop_reason_from_response_completed() {
        let response = serde_json::json!({"status": "completed"});
        assert_eq!(
            stop_reason_from_response(&response, false).as_deref(),
            Some("stop")
        );
        assert_eq!(
            stop_reason_from_response(&response, true).as_deref(),
            Some("tool_use")
        );
    }

    #[test]
    fn test_stop_reason_from_response_incomplete() {
        let response = serde_json::json!({
            "status": "incomplete",
            "incomplete_details": {"reason": "max_output_tokens"}
        });
        assert_eq!(
            stop_reason_from_response(&response, false).as_deref(),
            Some("length")
        );

        let response = serde_json::json!({
            "status": "incomplete",
            "incomplete_details": {"reason": "content_filter"}
        });
        assert_eq!(
            stop_reason_from_response(&response, false).as_deref(),
            Some("error")
        );

        // Unknown reason passes through unchanged
        let response = serde_json::json!({
            "status": "incomplete",
            "incomplete_details": {"reason": "something_new"}
        });
        assert_eq!(
            stop_reason_from_response(&response, false).as_deref(),
            Some("something_new")
        );

        // Missing reason falls back to the raw status
        let response = serde_json::json!({"status": "incomplete"});
        assert_eq!(
            stop_reason_from_response(&response, false).as_deref(),
            Some("incomplete")
        );
    }

    #[test]
    fn test_stop_reason_from_response_failed() {
        let response = serde_json::json!({"status": "failed"});
        assert_eq!(
            stop_reason_from_response(&response, false).as_deref(),
            Some("error")
        );
    }

    #[test]
    fn test_stop_reason_from_response_missing_status() {
        let response = serde_json::json!({"output": []});
        assert_eq!(stop_reason_from_response(&response, false), None);
    }
}

// TODO: Future support for built-in tools
// The Responses API supports these built-in tools:
// - web_search: Search the web for information
// - file_search: Search files in vector stores
// - code_interpreter: Execute code in a sandbox
//
// These can be enabled via the options config as JSON:
// {
//   "tools": [
//     { "type": "web_search" },
//     { "type": "file_search", "vector_store_ids": ["vs_abc123"] },
//     { "type": "code_interpreter" }
//   ]
// }
