use im::{Vector, hashmap, vector};
use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    Message, ModularAgent, async_trait, estimate_context_tokens, estimate_message_tokens,
    modular_agent,
};

use crate::chat::{DEFAULT_CONFIG_MODEL, request_or_cancelled};
use crate::provider::{ModelIdentifier, ProviderKind};
use crate::retry::RetryPolicy;

#[cfg(feature = "claude")]
use crate::claude_client;
#[cfg(feature = "ollama")]
use crate::ollama_client;
#[cfg(feature = "openai")]
use crate::openai_client;

const CATEGORY: &str = "LLM/Message";

const PORT_MESSAGES: &str = "messages";
const PORT_COMPACTION: &str = "compaction";

const CONFIG_MODEL: &str = "model";
const CONFIG_CONTEXT_WINDOW: &str = "context_window";
const CONFIG_RESERVE_TOKENS: &str = "reserve_tokens";
const CONFIG_KEEP_RECENT_TOKENS: &str = "keep_recent_tokens";
const CONFIG_INSTRUCTIONS: &str = "instructions";

const DEFAULT_RESERVE_TOKENS: i64 = 16384;
const DEFAULT_KEEP_RECENT_TOKENS: i64 = 20000;

/// Must match the prefix `build_context` (modular-agent-core) puts on the
/// injected summary message, both for detecting a previous summary in the
/// received context and for emitting the new one in the same shape.
const SUMMARY_PREFIX: &str = "[Conversation summary]\n";

/// Fewer messages than this always pass through: a compaction needs at least
/// one dropped message and a kept tail, and summarizing a two-message
/// exchange cannot shrink anything meaningfully.
const MIN_COMPACT_MESSAGES: usize = 3;

// Summarization is a single background request; the retry/timeout knobs of
// ChatAgent are not exposed as configs, so its defaults are reused.
const SUMMARY_MAX_RETRIES: i64 = 2;
const SUMMARY_RETRY_BASE_DELAY_MS: i64 = 1000;
const SUMMARY_TIMEOUT_SECS: i64 = 300;

/// Compress an over-long conversation context with an LLM summary.
///
/// Inserted between a `Messages` agent and a `Chat` agent. The token count
/// of the incoming context is estimated with the core hybrid heuristic;
/// while it stays at or below `context_window - reserve_tokens` the input is
/// forwarded unchanged with no LLM call. Above the threshold the agent picks
/// a cut point so the kept tail is roughly `keep_recent_tokens`, summarizes
/// everything before the cut with a single non-streaming LLM request, and
/// forwards `[summary message, kept messages...]`. The summary is a user
/// message prefixed with `"[Conversation summary]\n"` — the exact format
/// `build_context` uses when replaying a compacted session.
///
/// The kept tail never starts with a tool result, and an assistant message
/// carrying tool calls is never separated from its following tool results;
/// user-message boundaries near the token-budget cut are preferred as cut
/// points. When the input already starts with a previous summary message
/// (injected by `build_context` after an earlier compaction), that summary
/// is merged into the new one instead of being counted as a dropped
/// message. Leading system messages get no such protection: once they fall
/// into the dropped prefix they survive only paraphrased inside the
/// summary, so keep the system prompt out of the session store (e.g. add it
/// downstream of this agent) when it must stay verbatim.
///
/// The agent itself is stateless. To make a compaction stick across turns,
/// connect the `compaction` output back to the `message` input of the
/// `Messages` agent, which records it in the session store. Without that
/// connection the node still works as a one-shot context compressor, but
/// re-summarizes on every turn spent over the threshold.
///
/// The input passes through unchanged when it is empty or too small to
/// compact, when the last message is still streaming or is neither a user
/// nor a tool message (such a context is an echo of the assistant reply on
/// the Messages -> Compact -> Chat cycle, which the Chat agent discards),
/// or when the context window cannot be determined. A summarization failure
/// is an error — the oversized context is never forwarded silently.
///
/// # Ports
/// - Input `messages`: Message or array of messages (the conversation
///   context)
/// - Output `messages`: The context, unchanged or compacted
/// - Output `compaction`: On compaction, a record object `{"type":
///   "compaction", "summary": string, "dropped": integer, "tokens_before":
///   integer, "previous_summary": string?}`, emitted before the compacted
///   `messages`. `dropped` counts the summarized messages, excluding a
///   leading previous summary; `previous_summary` carries that leading
///   summary (prefix stripped) when one was present, letting the `Messages`
///   agent detect records computed against a stale baseline. Wire it to the
///   `message` input of the `Messages` agent
///
/// # Configuration
/// - `model`: Provider-prefixed model used for summarization (default:
///   "openai/gpt-5-nano")
/// - `context_window`: Total context window in tokens. 0: resolve from the
///   capability registry for `model`; if unknown there too, the input
///   passes through with a warning (default: 0)
/// - `reserve_tokens`: Headroom below the context window; compaction
///   triggers when the estimate exceeds `context_window - reserve_tokens`
///   (default: 16384)
/// - `keep_recent_tokens`: Approximate token budget of the kept tail
///   (default: 20000)
/// - `instructions`: Extra guidance appended to the summarization prompt
///
/// # Global Configuration
/// Uses the same provider credentials as the `Chat` agent (`claude_api_key`,
/// `openai_api_key`, `ollama_url`, and the corresponding base URLs).
///
/// # Example
/// With messages `[u1, a1, u2, a2, u3]` over the threshold and a cut at
/// `u3`, the agent emits `{"type": "compaction", "summary": S, "dropped":
/// 4, "tokens_before": T}` on `compaction`, then
/// `[user("[Conversation summary]\n" + S), u3]` on `messages`.
#[modular_agent(
    title = "Compact Messages",
    category = CATEGORY,
    inputs = [PORT_MESSAGES],
    outputs = [PORT_MESSAGES, PORT_COMPACTION],
    string_config(name = CONFIG_MODEL, default = DEFAULT_CONFIG_MODEL),
    integer_config(name = CONFIG_CONTEXT_WINDOW, title = "Context Window", default = 0, description = "0: resolve from the capability registry"),
    integer_config(name = CONFIG_RESERVE_TOKENS, title = "Reserve Tokens", default = DEFAULT_RESERVE_TOKENS, description = "Compact when the estimate exceeds context_window - reserve_tokens", detail),
    integer_config(name = CONFIG_KEEP_RECENT_TOKENS, title = "Keep Recent Tokens", default = DEFAULT_KEEP_RECENT_TOKENS, description = "Approximate token budget of the kept tail", detail),
    text_config(name = CONFIG_INSTRUCTIONS, description = "Extra guidance appended to the summarization prompt", detail),
    hint(width = 2, height = 2),
)]
pub struct CompactMessagesAgent {
    data: AgentData,
    #[cfg(feature = "claude")]
    claude_manager: claude_client::ClaudeManager,
    #[cfg(feature = "openai")]
    openai_manager: openai_client::OpenAIManager,
    #[cfg(feature = "ollama")]
    ollama_manager: ollama_client::OllamaManager,
}

#[async_trait]
impl AsAgent for CompactMessagesAgent {
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
        let configs = self.configs()?;
        let config_model = configs.get_string_or_default(CONFIG_MODEL);
        let config_context_window = configs.get_integer_or_default(CONFIG_CONTEXT_WINDOW);
        let reserve_tokens = configs
            .get_integer_or(CONFIG_RESERVE_TOKENS, DEFAULT_RESERVE_TOKENS)
            .max(0) as u64;
        let keep_recent_tokens = configs
            .get_integer_or(CONFIG_KEEP_RECENT_TOKENS, DEFAULT_KEEP_RECENT_TOKENS)
            .max(0) as u64;
        let instructions = configs.get_string_or_default(CONFIG_INSTRUCTIONS);

        let messages_value = value.to_message_value().ok_or_else(|| {
            AgentError::InvalidValue("Input contains non-Message values".to_string())
        })?;
        let messages_arr = if messages_value.is_array() {
            messages_value.into_array().unwrap_or_default()
        } else {
            vector![messages_value]
        };
        let messages: Vec<Message> = messages_arr
            .iter()
            .filter_map(|v| v.as_message().cloned())
            .collect();

        // A context too small to compact, or one still being streamed into,
        // passes through untouched.
        if messages.len() < MIN_COMPACT_MESSAGES || messages.last().is_some_and(|m| m.streaming) {
            return self.output(ctx, PORT_MESSAGES, value).await;
        }

        // Mirror ChatAgent's cycle guard: on the Messages -> Compact -> Chat
        // loop the assistant reply comes back as an assistant-ending context
        // that ChatAgent discards, so summarizing it would pay an LLM call
        // for a context that is never sent.
        let last_role = messages.last().map(|m| m.role.as_str());
        if last_role != Some("user") && last_role != Some("tool") {
            return self.output(ctx, PORT_MESSAGES, value).await;
        }

        // context_window == 0 defers to the capability registry; a model the
        // registry doesn't know cannot give a meaningful threshold, so the
        // input passes through rather than compacting against a guess.
        let resolved_window: Option<u64> = if config_context_window > 0 {
            Some(config_context_window as u64)
        } else if let Ok(id) = ModelIdentifier::parse(&config_model) {
            // The registry learns an Ollama model's window only from the
            // /api/show probe. ChatAgent warms it on its own request path,
            // but a summarizer-only model would otherwise never be probed
            // and compaction would stay silently disabled.
            #[cfg(feature = "ollama")]
            if id.provider == ProviderKind::Ollama
                && let Ok(client) = self.ollama_manager.get_client(self.ma())
            {
                crate::capabilities::warm_ollama_context(&client, &id.model_name).await;
            }
            crate::capabilities::resolve_entry(&id)
                .context_window
                .map(u64::from)
        } else {
            None
        };
        let Some(context_window) = resolved_window else {
            log::warn!(
                "Unknown context window for model '{config_model}'; \
                 passing messages through without compacting"
            );
            return self.output(ctx, PORT_MESSAGES, value).await;
        };

        let (previous_summary, start) = split_previous_summary(&messages);

        // estimate_context_tokens anchors on the last assistant message
        // carrying usage, assuming that usage covers the whole context up to
        // it. A compaction breaks that assumption: the kept tail can retain
        // an assistant message whose usage covers the *pre*-compaction
        // context, which would re-trigger compaction every turn until a
        // fresh usage-bearing reply lands. Once the context starts with an
        // injected summary, only the per-message estimate is trustworthy.
        let tokens_before: u64 = if previous_summary.is_some() {
            messages.iter().map(estimate_message_tokens).sum()
        } else {
            estimate_context_tokens(&messages)
        };
        if !should_compact(tokens_before, context_window, reserve_tokens) {
            return self.output(ctx, PORT_MESSAGES, value).await;
        }
        let Some(cut) = choose_cut_index(&messages, start, keep_recent_tokens) else {
            // Degenerate contexts (e.g. all tool results) have no valid cut.
            log::warn!("No valid compaction cut point found; passing messages through");
            return self.output(ctx, PORT_MESSAGES, value).await;
        };

        let dropped = &messages[start..cut];
        let kept = &messages[cut..];

        let model_id = ModelIdentifier::parse(&config_model)?;
        let prompt = build_summary_prompt(previous_summary.as_deref(), dropped, &instructions);
        let retry = RetryPolicy::from_configs(
            SUMMARY_MAX_RETRIES,
            SUMMARY_RETRY_BASE_DELAY_MS,
            SUMMARY_TIMEOUT_SECS,
        );
        let summary = self.summarize(&ctx, &model_id, prompt, retry).await?;

        // The record goes out first so MessagesAgent has the compaction on
        // file before any downstream turn appends new entries.
        let mut record = hashmap! {
            "type".into() => AgentValue::string("compaction"),
            "summary".into() => AgentValue::string(summary.clone()),
            "dropped".into() => AgentValue::integer(dropped.len() as i64),
            "tokens_before".into() => AgentValue::integer(
                i64::try_from(tokens_before).unwrap_or(i64::MAX)
            ),
        };
        // The baseline this record was computed against: the summary that
        // headed the received context, if any. MessagesAgent compares it
        // with its latest compaction to detect stale records (a session
        // reset or a second compaction racing the summarization call).
        if let Some(previous) = &previous_summary {
            record.insert(
                "previous_summary".into(),
                AgentValue::string(previous.clone()),
            );
        }
        self.output(ctx.clone(), PORT_COMPACTION, AgentValue::object(record))
            .await?;

        let mut out: Vector<AgentValue> =
            vector![Message::user(format!("{SUMMARY_PREFIX}{summary}")).into()];
        out.extend(kept.iter().cloned().map(AgentValue::from));
        self.output(ctx, PORT_MESSAGES, AgentValue::array(out))
            .await
    }
}

impl CompactMessagesAgent {
    /// One non-streaming summarization request against the configured model:
    /// no tools, no thinking, provider defaults for sampling. Runs under the
    /// retry policy and the flow's cancellation token.
    async fn summarize(
        &mut self,
        ctx: &AgentContext,
        model_id: &ModelIdentifier,
        prompt: String,
        retry: RetryPolicy,
    ) -> Result<String, AgentError> {
        let message = match model_id.provider {
            #[cfg(feature = "openai")]
            ProviderKind::OpenAI => {
                let client = self.openai_manager.get_client(self.ma())?;
                let request = serde_json::json!({
                    "model": model_id.model_name,
                    "messages": [openai_client::message_to_chat_json(&Message::user(prompt))],
                    "stream": false,
                });
                let url = client.chat_completions_url();
                let res: openai_client::ChatCompletionResponse = request_or_cancelled(
                    ctx.cancel_token(),
                    retry.run(|| client.post_json(&url, &request)),
                )
                .await?;
                let choice = res.choices.first().ok_or_else(|| {
                    AgentError::IoError("Summarization response has no choices".to_string())
                })?;
                openai_client::message_from_chat_response(&choice.message)
            }
            #[cfg(feature = "claude")]
            ProviderKind::Claude => {
                let client = self.claude_manager.get_client(self.ma())?;
                // Same non-streaming default as ChatAgent: cap at 8192 so a
                // runaway generation cannot outlive the per-attempt timeout.
                let max_tokens = crate::capabilities::resolve_entry(model_id)
                    .max_tokens
                    .unwrap_or(crate::capabilities::DEFAULT_MAX_TOKENS)
                    .min(crate::capabilities::DEFAULT_MAX_TOKENS);
                let prompt_messages = vector![AgentValue::from(Message::user(prompt))];
                let (system, claude_messages) = claude_client::messages_to_claude(&prompt_messages);
                let request = claude_client::ClaudeRequest {
                    model: model_id.model_name.clone(),
                    max_tokens,
                    messages: claude_messages,
                    system: system.map(claude_client::ClaudeContent::Text),
                    stream: None,
                    tools: None,
                    thinking: None,
                    output_config: None,
                    temperature: None,
                    top_p: None,
                };
                let response = request_or_cancelled(
                    ctx.cancel_token(),
                    retry.run(|| client.create_message(&request)),
                )
                .await?;
                claude_client::message_from_claude_response(&response)
            }
            #[cfg(feature = "ollama")]
            ProviderKind::Ollama => {
                let client = self.ollama_manager.get_client(self.ma())?;
                let request = serde_json::json!({
                    "model": model_id.model_name,
                    "messages": [
                        serde_json::to_value(ollama_client::message_to_ollama(&Message::user(
                            prompt,
                        )))
                        .unwrap_or(serde_json::json!({}))
                    ],
                    "stream": false,
                });
                let url = client.chat_url();
                let res: ollama_client::ChatResponse = request_or_cancelled(
                    ctx.cancel_token(),
                    retry.run(|| client.post_json(&url, &request)),
                )
                .await?;
                ollama_client::message_from_ollama(&res.message)
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(AgentError::InvalidConfig(format!(
                    "Provider {:?} not enabled. Enable the corresponding feature.",
                    model_id.provider
                )));
            }
        };

        let summary = message.text().trim().to_string();
        if summary.is_empty() {
            return Err(AgentError::IoError(
                "Summarization returned an empty response".to_string(),
            ));
        }
        Ok(summary)
    }
}

/// The compaction trigger: the estimated context exceeds the window minus
/// the reserved headroom. A reserve at or above the window makes the
/// threshold zero, so any non-empty context compacts.
fn should_compact(context_tokens: u64, context_window: u64, reserve_tokens: u64) -> bool {
    context_tokens > context_window.saturating_sub(reserve_tokens)
}

/// Detects the previous summary injected by `build_context` at the head of
/// the context. Returns the bare summary text (prefix stripped) and the
/// index where compactable messages start — the summary message itself is
/// merged into the new summary, never counted as dropped.
fn split_previous_summary(messages: &[Message]) -> (Option<String>, usize) {
    let previous = messages
        .first()
        .filter(|m| m.role == "user")
        .map(|m| m.text())
        .filter(|t| t.starts_with(SUMMARY_PREFIX))
        .map(|t| t[SUMMARY_PREFIX.len()..].to_string());
    let start = usize::from(previous.is_some());
    (previous, start)
}

/// A message the kept tail may start with. Tool results are excluded so a
/// result is never orphaned from the assistant tool-call message it answers;
/// cutting *at* that assistant message keeps the pair together instead.
fn is_valid_cut(m: &Message) -> bool {
    m.role == "user" || m.role == "assistant"
}

/// The valid index nearest `candidate` matching `pred`, searching forward
/// first: dropping more than the token-based candidate keeps the tail within
/// budget, while keeping more could leave the context over the threshold.
fn nearest_matching(
    messages: &[Message],
    lo: usize,
    hi: usize,
    candidate: usize,
    pred: impl Fn(&Message) -> bool,
) -> Option<usize> {
    (candidate..=hi)
        .find(|&i| pred(&messages[i]))
        .or_else(|| (lo..candidate).rev().find(|&i| pred(&messages[i])))
}

/// Chooses the index of the first kept message. Walks from the end
/// accumulating estimated tokens until `keep_recent_tokens` is spent, then
/// adjusts to the nearest valid cut point. A user boundary is preferred,
/// but only within half the keep budget of the candidate: an unbounded
/// preference could drag the cut to a distant user message, either keeping
/// far more than the budget (leaving the emitted context still over the
/// threshold) or collapsing the kept tail to almost nothing. At least one
/// message is dropped (`> start`) and the last message is always kept.
/// `None` when no valid cut exists in that range.
fn choose_cut_index(messages: &[Message], start: usize, keep_recent_tokens: u64) -> Option<usize> {
    let len = messages.len();
    let lo = start + 1;
    let hi = len.checked_sub(1)?;
    if lo > hi {
        return None;
    }

    // suffix[i] = estimated tokens of messages[i..].
    let mut suffix = vec![0u64; len + 1];
    for i in (0..len).rev() {
        suffix[i] = suffix[i + 1] + estimate_message_tokens(&messages[i]);
    }

    let mut candidate = len;
    for i in (lo..len).rev() {
        if suffix[i] > keep_recent_tokens {
            break;
        }
        candidate = i;
    }
    let candidate = candidate.min(hi);
    let candidate_tail = suffix[candidate];

    let bound = keep_recent_tokens / 2;
    let user_cut = (lo..=hi)
        .filter(|&i| messages[i].role == "user")
        .map(|i| (i, suffix[i].abs_diff(candidate_tail)))
        .filter(|&(_, distance)| distance <= bound)
        // Ties resolve forward (larger index): dropping more keeps the
        // tail within budget, keeping more could stay over the threshold.
        .min_by_key(|&(i, distance)| (distance, std::cmp::Reverse(i)))
        .map(|(i, _)| i);

    user_cut.or_else(|| nearest_matching(messages, lo, hi, candidate, is_valid_cut))
}

/// The structured summarization prompt. With a previous summary the prompt
/// switches to UPDATE mode, merging it with the newly dropped messages; the
/// user's `instructions` config is appended verbatim when non-empty.
fn build_summary_prompt(
    previous_summary: Option<&str>,
    dropped: &[Message],
    instructions: &str,
) -> String {
    let mut prompt = String::new();
    if let Some(previous) = previous_summary {
        prompt.push_str(
            "You are updating the running summary of an ongoing conversation.\n\n\
             Current summary:\n",
        );
        prompt.push_str(previous);
        prompt.push_str("\n\nMerge the following new conversation messages into the summary:\n\n");
    } else {
        prompt.push_str("Summarize the following conversation:\n\n");
    }
    prompt.push_str(&render_transcript(dropped));
    prompt.push_str(
        "\nWrite a concise summary that captures:\n\
         - the user's goals and intent\n\
         - key decisions made and why\n\
         - constraints and requirements that must be respected\n\
         - important facts, names, and data mentioned\n\
         - unresolved questions and pending tasks\n\
         Respond with the summary text only.",
    );
    if !instructions.is_empty() {
        prompt.push_str("\n\nAdditional instructions:\n");
        prompt.push_str(instructions);
    }
    prompt
}

/// Renders dropped messages as a plain-text transcript for the
/// summarization prompt. Tool calls are listed by name and arguments so
/// decisions carried through tools survive into the summary; image blocks
/// leave an `[image: <mime_type>]` placeholder so the summary keeps a
/// record that a tool returned an image.
fn render_transcript(messages: &[Message]) -> String {
    let mut out = String::new();
    for m in messages {
        out.push('[');
        out.push_str(&m.role);
        out.push_str("]\n");
        // Not text(): that drops image blocks without a trace.
        let text = crate::content::tool_result_fallback_text(&m.content);
        if !text.is_empty() {
            out.push_str(&text);
            out.push('\n');
        }
        if let Some(tool_calls) = &m.tool_calls {
            for call in tool_calls {
                let args = serde_json::to_string(&call.function.parameters).unwrap_or_default();
                out.push_str(&format!("(tool call: {} {})\n", call.function.name, args));
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use im::vector;
    use modular_agent_core::test_utils::{ProbeReceiver, TestProbeAgent, probe_receiver};
    use modular_agent_core::{
        AgentStatus, ConnectionSpec, ContentBlock, MessageContent, ToolCall, ToolCallFunction,
    };

    fn user(text: &str) -> Message {
        Message::user(text.to_string())
    }

    #[test]
    fn render_transcript_marks_image_tool_results() {
        // A dropped image-only tool result must leave a trace in the
        // summarization prompt, not an empty [tool] entry.
        let msg = Message::tool_with_content(
            "screenshot".to_string(),
            MessageContent::Blocks(vec![ContentBlock::Image {
                data: "base64data".to_string(),
                mime_type: "image/png".to_string(),
            }]),
        );
        let out = render_transcript(&[msg]);
        assert!(out.contains("[tool]"));
        assert!(out.contains("[image: image/png]"));
    }

    fn assistant(text: &str) -> Message {
        Message::assistant(text.to_string())
    }

    /// The large arguments give the message ~100 estimated tokens, so cut
    /// candidates can be steered onto or past it via the budget.
    fn assistant_with_tool_call() -> Message {
        let mut m = Message::assistant(String::new());
        m.tool_calls = Some(
            vector![ToolCall {
                function: ToolCallFunction {
                    id: Some("t1".to_string()),
                    name: "lookup".to_string(),
                    parameters: serde_json::json!({"q": "x".repeat(400)}),
                    parse_error: None,
                },
            }]
            .into_iter()
            .collect(),
        );
        m
    }

    fn tool_result(text: &str) -> Message {
        Message::tool("lookup".to_string(), text.to_string())
    }

    // --- threshold arithmetic ---

    #[test]
    fn test_should_compact_threshold() {
        assert!(!should_compact(150, 200, 50));
        assert!(should_compact(151, 200, 50));
        // Reserve at or above the window: threshold is zero.
        assert!(should_compact(1, 100, 100));
        assert!(should_compact(1, 100, 200));
        assert!(!should_compact(0, 100, 200));
    }

    // --- previous-summary detection ---

    #[test]
    fn test_split_previous_summary() {
        let msgs = vec![
            Message::user(format!("{SUMMARY_PREFIX}earlier summary")),
            user("q"),
        ];
        let (summary, start) = split_previous_summary(&msgs);
        assert_eq!(summary.as_deref(), Some("earlier summary"));
        assert_eq!(start, 1);

        let msgs = vec![user("plain question"), assistant("a")];
        let (summary, start) = split_previous_summary(&msgs);
        assert_eq!(summary, None);
        assert_eq!(start, 0);

        // The prefix only counts on a user message.
        let msgs = vec![
            Message::assistant(format!("{SUMMARY_PREFIX}not a summary")),
            user("q"),
        ];
        let (summary, start) = split_previous_summary(&msgs);
        assert_eq!(summary, None);
        assert_eq!(start, 0);
    }

    // --- cut-point selection ---

    #[test]
    fn test_cut_never_starts_kept_tail_at_tool_result() {
        // Token walk with budget 150: user "q" (1) and the tool result
        // (~100) fit, the tool-call assistant (~100) breaks — the raw
        // candidate is the tool-result index 2 and must be adjusted.
        let msgs = vec![
            user(&"a".repeat(400)),
            assistant_with_tool_call(),
            tool_result(&"r".repeat(400)),
            user("q"),
        ];
        let cut = choose_cut_index(&msgs, 0, 150).unwrap();
        assert_eq!(cut, 3);
        assert_ne!(msgs[cut].role, "tool");
    }

    #[test]
    fn test_cut_keeps_assistant_tool_pair_together() {
        // Budget 50 puts the raw candidate on the final assistant (index 4)
        // with no user in bound: cutting there drops the assistant+tool
        // pair *together* — the pair is never split across the cut. The
        // backward user at index 1 is far outside the token bound, so it
        // must not drag the cut back (which would keep ~4x the budget).
        let msgs = vec![
            user(&"a".repeat(400)),
            user("b"),
            assistant_with_tool_call(),
            tool_result(&"r".repeat(400)),
            assistant("done"),
        ];
        let cut = choose_cut_index(&msgs, 0, 50).unwrap();
        assert_eq!(cut, 4);
        assert_ne!(msgs[cut].role, "tool");
    }

    #[test]
    fn test_cut_user_preference_bounded_backward() {
        // Agentic shape: users only near the head, then a long assistant
        // run. The backward user at index 1 would keep ~300 tokens against
        // a 150 budget; the bounded preference rejects it and the valid
        // assistant cut at the candidate wins.
        let msgs = vec![
            user(&"a".repeat(400)),
            user("b"),
            assistant(&"c".repeat(400)),
            assistant(&"d".repeat(400)),
            assistant(&"e".repeat(400)),
        ];
        let cut = choose_cut_index(&msgs, 0, 150).unwrap();
        assert_eq!(cut, 4);
    }

    #[test]
    fn test_cut_user_preference_bounded_forward() {
        // The only user at or after the mid-chain candidate is the final
        // message; jumping there would summarize away the whole kept
        // budget. The bounded preference rejects it and the assistant at
        // the candidate wins, keeping the assistant+tool pair dropped
        // together.
        let msgs = vec![
            user(&"a".repeat(400)),
            assistant_with_tool_call(),
            tool_result(&"r".repeat(400)),
            assistant(&"s".repeat(400)),
            user("q"),
        ];
        let cut = choose_cut_index(&msgs, 0, 150).unwrap();
        assert_eq!(cut, 3);
    }

    #[test]
    fn test_cut_prefers_user_boundary_over_assistant() {
        // Budget lands the candidate on the assistant at index 3; the
        // forward user at index 4 is preferred over cutting at an assistant.
        let msgs = vec![
            user("a"),
            assistant(&"b".repeat(400)),
            user(&"c".repeat(400)),
            assistant(&"d".repeat(40)),
            user("e"),
        ];
        let cut = choose_cut_index(&msgs, 0, 20).unwrap();
        assert_eq!(cut, 4);
    }

    #[test]
    fn test_cut_respects_start_after_previous_summary() {
        // start = 1: the summary message can never be part of the dropped
        // range, and at least one real message must be dropped.
        let msgs = vec![
            Message::user(format!("{SUMMARY_PREFIX}old")),
            user(&"a".repeat(400)),
            assistant("b"),
            user("c"),
        ];
        let cut = choose_cut_index(&msgs, 1, 10).unwrap();
        assert!(cut > 1);
        assert_eq!(cut, 3);
    }

    #[test]
    fn test_cut_always_keeps_last_message_and_drops_at_least_one() {
        // All-user messages so the boundary preference cannot move the cut.
        // A huge budget still drops at least one message (lo = start + 1)...
        let msgs = vec![user("a"), user("b"), user("c")];
        let cut = choose_cut_index(&msgs, 0, u64::MAX).unwrap();
        assert_eq!(cut, 1);

        // ...and a zero budget still keeps the last message.
        let cut = choose_cut_index(&msgs, 0, 0).unwrap();
        assert_eq!(cut, 2);
    }

    #[test]
    fn test_cut_none_when_no_valid_cut_exists() {
        let msgs = vec![user("a"), tool_result("r1"), tool_result("r2")];
        assert_eq!(choose_cut_index(&msgs, 0, 0), None);
    }

    // --- dropped counting ---

    #[test]
    fn test_dropped_count_excludes_previous_summary() {
        let msgs = vec![
            Message::user(format!("{SUMMARY_PREFIX}old")),
            user(&"a".repeat(400)),
            assistant(&"b".repeat(400)),
            user("c"),
        ];
        let (summary, start) = split_previous_summary(&msgs);
        assert!(summary.is_some());
        let cut = choose_cut_index(&msgs, start, 10).unwrap();
        assert_eq!(cut, 3);
        // dropped = messages in [start, cut): the two real messages only.
        assert_eq!(cut - start, 2);
    }

    #[test]
    fn test_dropped_count_without_previous_summary() {
        let msgs = vec![
            user(&"a".repeat(400)),
            assistant(&"b".repeat(400)),
            user("c"),
        ];
        let (summary, start) = split_previous_summary(&msgs);
        assert!(summary.is_none());
        let cut = choose_cut_index(&msgs, start, 10).unwrap();
        assert_eq!(cut - start, 2);
    }

    // --- prompt building ---

    #[test]
    fn test_build_summary_prompt_update_mode_and_instructions() {
        let dropped = vec![user("hello"), assistant("hi")];

        let fresh = build_summary_prompt(None, &dropped, "");
        assert!(fresh.starts_with("Summarize the following conversation:"));
        assert!(fresh.contains("hello"));
        assert!(!fresh.contains("Additional instructions:"));

        let update = build_summary_prompt(Some("previous facts"), &dropped, "Keep it short.");
        assert!(update.contains("previous facts"));
        assert!(update.contains("Merge the following new conversation messages"));
        assert!(update.ends_with("Additional instructions:\nKeep it short."));
    }

    // --- harness: pass-through paths (no LLM call) ---

    /// `start_patch` returns before the spawned agent loop has run
    /// `AsAgent::start`; wait until the status flips to `Start`.
    async fn wait_until_started(ma: &ModularAgent, agent_id: &str) {
        for _ in 0..200 {
            {
                let agent = ma.get_agent(agent_id).unwrap();
                let guard = agent.lock().await;
                if *guard.status() == AgentStatus::Start {
                    return;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("agent {agent_id} did not start in time");
    }

    /// Build a running patch with a CompactMessagesAgent whose `messages`
    /// and `compaction` ports each feed a probe.
    async fn setup_compact_agent(
        configs: Vec<(&str, AgentValue)>,
    ) -> (ModularAgent, String, ProbeReceiver, ProbeReceiver) {
        let ma = ModularAgent::init().unwrap();
        ma.ready().await.unwrap();

        let patch_id = ma.new_patch().unwrap();
        let def = ma
            .get_agent_definition(CompactMessagesAgent::DEF_NAME)
            .unwrap();
        let mut spec = def.to_spec();
        {
            let spec_configs = spec.configs.as_mut().unwrap();
            for (key, value) in configs {
                spec_configs.set(key.into(), value);
            }
        }
        let agent_id = ma.add_agent(patch_id.clone(), spec).await.unwrap();

        let probe_def = ma.get_agent_definition(TestProbeAgent::DEF_NAME).unwrap();
        let messages_probe_id = ma
            .add_agent(patch_id.clone(), probe_def.to_spec())
            .await
            .unwrap();
        ma.add_connection(
            &patch_id,
            ConnectionSpec {
                source: agent_id.clone(),
                source_handle: PORT_MESSAGES.into(),
                target: messages_probe_id.clone(),
                target_handle: "value".into(),
            },
        )
        .await
        .unwrap();
        let compaction_probe_id = ma
            .add_agent(patch_id.clone(), probe_def.to_spec())
            .await
            .unwrap();
        ma.add_connection(
            &patch_id,
            ConnectionSpec {
                source: agent_id.clone(),
                source_handle: PORT_COMPACTION.into(),
                target: compaction_probe_id.clone(),
                target_handle: "value".into(),
            },
        )
        .await
        .unwrap();

        ma.start_patch(&patch_id).await.unwrap();
        wait_until_started(&ma, &agent_id).await;
        let messages_rx = probe_receiver(&ma, &messages_probe_id).await.unwrap();
        let compaction_rx = probe_receiver(&ma, &compaction_probe_id).await.unwrap();

        (ma, agent_id, messages_rx, compaction_rx)
    }

    async fn send(ma: &ModularAgent, agent_id: &str, value: AgentValue) {
        let agent = ma.get_agent(agent_id).unwrap();
        let mut guard = agent.lock().await;
        let compact = guard.as_agent_mut::<CompactMessagesAgent>().unwrap();
        AsAgent::process(
            compact,
            AgentContext::new(),
            PORT_MESSAGES.to_string(),
            value,
        )
        .await
        .unwrap();
    }

    async fn assert_no_probe_emit(rx: &ProbeReceiver) {
        assert!(
            rx.recv_with_timeout(std::time::Duration::from_millis(100))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn compact_passes_through_under_threshold_without_llm_call() {
        // Threshold 1000 - 100 = 900; the tiny context stays far below it.
        let (ma, agent_id, messages_rx, compaction_rx) = setup_compact_agent(vec![
            (CONFIG_CONTEXT_WINDOW, AgentValue::integer(1000)),
            (CONFIG_RESERVE_TOKENS, AgentValue::integer(100)),
        ])
        .await;

        let input = AgentValue::array(vector![
            user("hello").into(),
            assistant("hi").into(),
            user("more").into(),
        ]);
        send(&ma, &agent_id, input.clone()).await;

        let (_ctx, value) = messages_rx.recv().await.unwrap();
        assert_eq!(value, input);
        assert_no_probe_emit(&compaction_rx).await;

        ma.quit();
    }

    #[tokio::test]
    async fn compact_passes_through_streaming_tail_even_over_threshold() {
        let (ma, agent_id, messages_rx, compaction_rx) = setup_compact_agent(vec![
            (CONFIG_CONTEXT_WINDOW, AgentValue::integer(100)),
            (CONFIG_RESERVE_TOKENS, AgentValue::integer(50)),
        ])
        .await;

        let mut partial = assistant(&"x".repeat(4000));
        partial.streaming = true;
        let input = AgentValue::array(vector![
            user("q1").into(),
            assistant(&"y".repeat(4000)).into(),
            partial.into(),
        ]);
        send(&ma, &agent_id, input.clone()).await;

        let (_ctx, value) = messages_rx.recv().await.unwrap();
        assert_eq!(value, input);
        assert_no_probe_emit(&compaction_rx).await;

        ma.quit();
    }

    #[tokio::test]
    async fn compact_passes_through_when_context_window_unknown() {
        // context_window 0 and a model the registry cannot know: warn and
        // pass through instead of compacting against a guessed window.
        let (ma, agent_id, messages_rx, compaction_rx) = setup_compact_agent(vec![(
            CONFIG_MODEL,
            AgentValue::string("openai/totally-unknown-model-xyz"),
        )])
        .await;

        let input = AgentValue::array(vector![
            user(&"a".repeat(4000)).into(),
            assistant(&"b".repeat(4000)).into(),
            user("q").into(),
        ]);
        send(&ma, &agent_id, input.clone()).await;

        let (_ctx, value) = messages_rx.recv().await.unwrap();
        assert_eq!(value, input);
        assert_no_probe_emit(&compaction_rx).await;

        ma.quit();
    }

    #[tokio::test]
    async fn compact_passes_through_tiny_context_even_over_threshold() {
        let (ma, agent_id, messages_rx, compaction_rx) = setup_compact_agent(vec![
            (CONFIG_CONTEXT_WINDOW, AgentValue::integer(100)),
            (CONFIG_RESERVE_TOKENS, AgentValue::integer(100)),
        ])
        .await;

        let input = AgentValue::array(vector![user(&"a".repeat(4000)).into(), user("q").into(),]);
        send(&ma, &agent_id, input.clone()).await;

        let (_ctx, value) = messages_rx.recv().await.unwrap();
        assert_eq!(value, input);
        assert_no_probe_emit(&compaction_rx).await;

        ma.quit();
    }
}
