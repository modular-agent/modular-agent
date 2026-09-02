use std::sync::Arc;

use im::{Vector, vector};
use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    ContentBlock, InMemorySessionStore, JsonlSessionStore, Message, MessageContent, ModularAgent,
    SessionEntry, SessionMeta, SessionStore, async_trait, build_context, build_context_with_ids,
    estimate_context_tokens, estimate_message_tokens, modular_agent,
};

const CATEGORY: &str = "LLM/Message";

const PORT_MESSAGE: &str = "message";
const PORT_MESSAGES: &str = "messages";
const PORT_RESET: &str = "reset";
const PORT_SESSION_ID: &str = "session_id";

const CONFIG_MAX_CONTEXT_TOKENS: &str = "max_context_tokens";
const CONFIG_MAX_MESSAGES: &str = "max_messages";
const CONFIG_MAX_MESSAGE_TOKENS: &str = "max_message_tokens";
const CONFIG_MAX_SIZE: &str = "max_size";
const CONFIG_MAX_TOKENS: &str = "max_tokens";
const CONFIG_MESSAGE: &str = "message";
const CONFIG_PREAMBLE: &str = "preamble";
const CONFIG_PRUNE_FILE: &str = "prune_file";
const CONFIG_SESSION_DIR: &str = "session_dir";
const CONFIG_SESSION_ID: &str = "session_id";

/// Old patches stored the history in a hidden `messages` config;
/// `reconcile_spec()` renames it to `_messages` for lazy migration.
const STALE_CONFIG_MESSAGES: &str = "_messages";

/// `session_dir` was removed from the Messages agent when file persistence
/// moved to the File Messages agent; `reconcile_spec()` renames a leftover
/// value to `_session_dir`, which `new()` reads to warn about the change.
const STALE_CONFIG_SESSION_DIR: &str = "_session_dir";

/// Must match the prefix `build_context` (modular-agent-core) puts on the
/// injected summary message; used to recognize a genuine head message the
/// compactor mistook for an injected summary.
const SUMMARY_PREFIX: &str = "[Conversation summary]\n";

/// Marker inserted where middle-trimming removed text.
const TRIM_MARKER: &str = "\n[...]\n";

/// Minimum bytes of text kept on each side of the marker, so trimming never
/// empties a message even when the budget says it should.
const TRIM_FLOOR_BYTES: usize = 16;

// Assistant Message Agent
#[modular_agent(
    title="Assistant Message",
    category=CATEGORY,
    inputs=[PORT_MESSAGES],
    outputs=[PORT_MESSAGES],
    text_config(name=CONFIG_MESSAGE),
    hint(width = 2, height = 1),
)]
pub struct AssistantMessageAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for AssistantMessageAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let message = self.configs()?.get_string(CONFIG_MESSAGE)?;
        let message = Message::assistant(message);
        let messages = append_message(value, message);
        self.output(ctx, PORT_MESSAGES, messages).await?;
        Ok(())
    }
}

/// Add a system message to the messages.
///
/// The system message is always prepended to the messages.
#[modular_agent(
    title="System Message",
    category=CATEGORY,
    inputs=[PORT_MESSAGES],
    outputs=[PORT_MESSAGES],
    text_config(name=CONFIG_MESSAGE),
    hint(width = 2, height = 1),
)]
pub struct SystemMessageAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for SystemMessageAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let message = self.configs()?.get_string(CONFIG_MESSAGE)?;
        let message = Message::system(message);
        let messages = prepend_message(value, message);
        self.output(ctx, PORT_MESSAGES, messages).await?;
        Ok(())
    }
}

// User Message Agent
#[modular_agent(
    title="User Message",
    category=CATEGORY,
    inputs=[PORT_MESSAGES],
    outputs=[PORT_MESSAGES],
    text_config(name=CONFIG_MESSAGE),
    hint(width = 2, height = 1),
)]
pub struct UserMessageAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for UserMessageAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let message = self.configs()?.get_string(CONFIG_MESSAGE)?;
        let message = Message::user(message);
        let messages = append_message(value, message);
        self.output(ctx, PORT_MESSAGES, messages).await?;
        Ok(())
    }
}

fn append_message(value: AgentValue, message: Message) -> AgentValue {
    #[cfg(feature = "image")]
    if let AgentValue::Image(img) = &value {
        let message = message.with_image(img.clone());
        return AgentValue::array(vector![message.into()]);
    }

    let Some(value) = value.to_message_value() else {
        return message.into();
    };

    if value.is_array() {
        let mut arr = value.into_array().unwrap_or_default();
        arr.push_back(message.into());
        return AgentValue::array(arr);
    }

    AgentValue::array(vector![value, message.into()])
}

fn prepend_message(value: AgentValue, message: Message) -> AgentValue {
    let Some(value) = value.to_message_value() else {
        return message.into();
    };

    if value.is_array() {
        let mut arr = value.into_array().unwrap_or_default();
        arr.push_front(message.into());
        return AgentValue::array(arr);
    }

    AgentValue::array(vector![message.into(), value])
}

/// Prepend a preamble message to the first input message.
///
//// The preamble message is added only once.
#[modular_agent(
    title="Preamble",
    category=CATEGORY,
    inputs=[PORT_MESSAGE, PORT_RESET],
    outputs=[PORT_MESSAGES],
    object_config(name=CONFIG_PREAMBLE),
    hint(width = 2, height = 2),
)]
pub struct PreambleAgent {
    data: AgentData,
    preamble: Option<Vector<AgentValue>>,
    prepended: bool,
}

#[async_trait]
impl AsAgent for PreambleAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let preamble = spec
            .configs
            .as_ref()
            .map(|c| c.get(CONFIG_PREAMBLE))
            .transpose()?
            .and_then(|v| v.to_message_value());
        let preamble = match preamble {
            None => None,
            Some(preamble) => {
                if preamble.is_array() {
                    Some(preamble.into_array().unwrap_or_default())
                } else {
                    Some(vector![preamble])
                }
            }
        };
        let data = AgentData::new(ma, id, spec);
        Ok(Self {
            data,
            preamble,
            prepended: false,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let preamble = self.configs()?.get(CONFIG_PREAMBLE)?.to_message_value();
        self.preamble = match preamble {
            None => None,
            Some(preamble) => {
                if preamble.is_array() {
                    Some(preamble.into_array().unwrap_or_default())
                } else {
                    Some(vector![preamble])
                }
            }
        };
        Ok(())
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        self.prepended = false;
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if port == PORT_RESET {
            self.prepended = false;
            return Ok(());
        }

        let Some(message) = value.to_message() else {
            return Err(AgentError::InvalidValue(
                "Input value is not a Message".to_string(),
            ));
        };

        if self.prepended {
            return self
                .output(
                    ctx,
                    PORT_MESSAGES,
                    AgentValue::array(vector![message.into()]),
                )
                .await;
        }

        self.prepended = true;

        let Some(preamble) = &self.preamble else {
            return self
                .output(
                    ctx,
                    PORT_MESSAGES,
                    AgentValue::array(vector![message.into()]),
                )
                .await;
        };

        let mut messages = preamble.clone();
        messages.push_back(message.into());
        self.output(ctx, PORT_MESSAGES, AgentValue::array(messages))
            .await?;

        Ok(())
    }
}

/// State shared by the session-backed Messages agents: the active session
/// and the in-memory caches used to build the emitted context.
#[derive(Default)]
struct SessionState {
    /// Session the agent appends to, resolved in `start()`.
    session_id: Option<String>,

    /// In-memory cache of the session's entries, replayed in `start()`.
    entries: Vec<SessionEntry>,

    /// Whether entries have been pruned from this session. A baseline-less
    /// compaction record's `dropped` count is resolved against the entry
    /// log's head, so once the head has moved it can no longer be trusted.
    pruned: bool,
}

impl SessionState {
    fn session_id(&self) -> Result<&str, AgentError> {
        self.session_id
            .as_deref()
            .ok_or_else(|| AgentError::Other("Session is not initialized".to_string()))
    }
}

/// Store and state access shared by [`process_session_input`] across the
/// Messages agents; each agent keeps its own store kind (in-memory vs JSONL
/// files).
trait SessionMessages: AsAgent {
    fn store(&self) -> Result<Arc<dyn SessionStore>, AgentError>;

    fn session_state_mut(&mut self) -> &mut SessionState;

    /// Whether pruned entries are also removed from the store. The
    /// in-memory agent always prunes its store (that store *is* the memory
    /// being bounded); the file agent consults its `prune_file` config.
    fn prune_store(&self) -> Result<bool, AgentError> {
        Ok(true)
    }
}

/// Resolve the session to append to at start: create a fresh one when no id
/// is configured, otherwise load the configured session.
async fn resolve_session(
    store: &Arc<dyn SessionStore>,
    configured_id: String,
) -> Result<(String, Vec<SessionEntry>), AgentError> {
    if configured_id.is_empty() {
        let id = store.create(SessionMeta::new()).await?;
        return Ok((id, Vec::new()));
    }
    match store.load(&configured_id).await {
        Ok(entries) => Ok((configured_id, entries)),
        Err(load_err) => {
            // The configured id may point at a session this store has never
            // seen (e.g. an in-memory store after a process restart).
            // Recreate it as an empty session; if even that fails, the load
            // error was real and wins.
            let meta = SessionMeta {
                id: configured_id.clone(),
                ..SessionMeta::new()
            };
            if store.create(meta).await.is_err() {
                return Err(load_err);
            }
            Ok((configured_id, Vec::new()))
        }
    }
}

/// Write an issued session id back to the config and push it to the UI;
/// `set_config` alone emits no event.
fn publish_session_id<A: SessionMessages>(agent: &mut A, id: &str) -> Result<(), AgentError> {
    agent.set_config(CONFIG_SESSION_ID.to_string(), AgentValue::string(id))?;
    agent.emit_config_updated(CONFIG_SESSION_ID, AgentValue::string(id));
    Ok(())
}

/// Convert an input value into a batch of message values, or `None` when the
/// input is empty and there is nothing to append.
fn to_message_batch(value: AgentValue) -> Result<Option<Vector<AgentValue>>, AgentError> {
    let message = value
        .to_message_value()
        .ok_or_else(|| AgentError::InvalidValue("Input contains non-Message values".to_string()))?;
    let messages = if message.is_array() {
        message.into_array().unwrap_or_default()
    } else {
        vector![message]
    };
    Ok((!messages.is_empty()).then_some(messages))
}

/// Append a batch of messages to the session. Only finalized messages reach
/// the store; streaming partials are skipped entirely. With
/// `max_message_tokens` > 0, each non-system message is middle-trimmed to
/// that budget before it is stored.
async fn append_messages(
    store: &Arc<dyn SessionStore>,
    state: &mut SessionState,
    messages: &[Message],
    max_message_tokens: i64,
) -> Result<(), AgentError> {
    let session_id = state.session_id()?.to_string();
    for message in messages {
        if message.streaming {
            continue;
        }
        let message = if max_message_tokens > 0 && message.role != "system" {
            middle_trim(message, max_message_tokens as u64).unwrap_or_else(|| message.clone())
        } else {
            message.clone()
        };
        let entry = SessionEntry::message(message);
        store.append(&session_id, entry.clone()).await?;
        state.entries.push(entry);
    }
    Ok(())
}

/// Record a compaction marker received on the `message` port (emitted by
/// `CompactMessagesAgent`'s `compaction` output).
///
/// The record's `dropped` count refers to the context the compactor
/// received, whose head is the previous compaction's first kept entry
/// (or the log's head when none exists). Skipping `dropped` Message
/// entries from there yields the new `first_kept_id`. The entry is
/// appended to the store and the cache, but nothing is emitted:
/// re-emitting the compacted context would re-trigger the downstream
/// ChatAgent and issue a duplicate request. The next turn picks the
/// compaction up naturally via [`build_context`]. The partial slot is
/// left untouched.
///
/// The record's `previous_summary` identifies the baseline it was
/// computed against; a record whose baseline no longer matches the
/// session's latest compaction is *discarded* with a warning. The
/// summarization call takes seconds, so a stale record is realistic:
/// a `reset` can swap in a fresh session while the call is in flight
/// (applying the old conversation's summary would leak it into the new
/// session), and a second same-baseline compaction can race the first
/// (resolving it against the newer compaction would silently drop
/// messages its summary does not cover). A first compaction carries no
/// baseline, so it is sanity-checked by size instead: a record whose
/// `tokens_before` is more than twice the session's current estimate
/// cannot describe this session and is discarded.
async fn record_compaction(
    store: &Arc<dyn SessionStore>,
    state: &mut SessionState,
    value: &AgentValue,
) -> Result<(), AgentError> {
    let summary = value
        .get_str("summary")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AgentError::InvalidValue("Compaction record must have a non-empty summary".to_string())
        })?
        .to_string();
    let dropped = value
        .get("dropped")
        .and_then(|v| v.as_i64())
        .filter(|d| *d >= 0)
        .ok_or_else(|| {
            AgentError::InvalidValue(
                "Compaction record must have a non-negative integer dropped count".to_string(),
            )
        })? as usize;
    let tokens_before = value
        .get("tokens_before")
        .and_then(|v| v.as_i64())
        .and_then(|v| u64::try_from(v).ok());
    let previous_summary = value.get_str("previous_summary").map(str::to_string);

    let session_id = state.session_id()?.to_string();

    let last_compaction = state
        .entries
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, e)| match e {
            SessionEntry::Compaction {
                summary,
                first_kept_id,
                ..
            } => Some((i, summary.clone(), first_kept_id.clone())),
            _ => None,
        });

    // Where the compactor's received context started. The record's
    // baseline (`previous_summary`) must agree with the session's state;
    // otherwise the record is stale and applying it would corrupt the
    // session.
    let start = match (&previous_summary, &last_compaction) {
        // Baseline is the latest compaction: its first kept Message
        // entry, or the entry right after it when the id is unknown.
        (Some(previous), Some((compaction_index, last_summary, first_kept_id)))
            if previous == last_summary =>
        {
            state
                .entries
                .iter()
                .position(|e| matches!(e, SessionEntry::Message { id, .. } if id == first_kept_id))
                .unwrap_or(compaction_index + 1)
        }
        // No compaction on either side: the record claims to cover this
        // session's head. With no baseline to compare, cross-check the
        // record's size instead: the compactor computed `tokens_before`
        // from a context that must be a prefix of this session's
        // entries, so the session's own estimate can only be larger
        // (the tail may have grown since), never substantially smaller.
        // A much smaller session means the record came from a different
        // session — a reset raced the in-flight summarization call. The
        // factor of 2 absorbs usage-anchor drift (a fresh reply
        // re-anchoring a heuristically estimated tail).
        (None, None) => {
            // The record's `dropped` count is anchored to the log's head; a
            // prune has moved the head, so the walk below would skip past
            // messages the summary does not cover.
            if state.pruned {
                log::warn!(
                    "Discarding a baseline-less compaction record: the session's \
                     history has been pruned since it was computed"
                );
                return Ok(());
            }
            if let Some(tokens_before) = tokens_before {
                let current = estimate_context_tokens(&build_context(&state.entries));
                if tokens_before > current.saturating_mul(2) {
                    log::warn!(
                        "Discarding a compaction record sized for another session \
                         (record covers ~{tokens_before} tokens, session holds \
                         ~{current}); the session has likely been reset since"
                    );
                    return Ok(());
                }
            }
            0
        }
        // A baseline with no compaction on file: the compactor mistook a
        // genuine head message for an injected summary. It excluded that
        // head from `dropped`, so the walk must skip the matching entry
        // too.
        (Some(previous), None) => {
            let head = state.entries.iter().position(|e| {
                matches!(e, SessionEntry::Message { message, .. }
                    if message.role == "user"
                        && message.text() == format!("{SUMMARY_PREFIX}{previous}"))
            });
            let Some(head_index) = head else {
                log::warn!(
                    "Discarding a compaction record computed against an unknown \
                     previous summary; the session has likely been reset since"
                );
                return Ok(());
            };
            head_index + 1
        }
        _ => {
            log::warn!(
                "Discarding a stale compaction record: its baseline does not match \
                 the session's latest compaction"
            );
            return Ok(());
        }
    };

    let first_kept_id = state.entries[start..]
        .iter()
        .filter_map(|e| match e {
            SessionEntry::Message { id, .. } => Some(id),
            _ => None,
        })
        .nth(dropped)
        .cloned();
    let Some(first_kept_id) = first_kept_id else {
        // A consistent record always resolves (the compactor keeps at
        // least one message), so exhaustion means the record belongs to
        // another state of the world — e.g. a session reset during the
        // summarization call. Recording it anyway would inject a foreign
        // summary into this session.
        log::warn!(
            "Discarding a compaction record: its dropped count {dropped} exceeds \
             the session's messages"
        );
        return Ok(());
    };

    let entry = SessionEntry::compaction(summary, first_kept_id, tokens_before);
    store.append(&session_id, entry.clone()).await?;
    state.entries.push(entry);
    Ok(())
}

/// Shared `process()` body for the Messages agents: `reset` swaps in a new
/// session, a unit input re-emits the current window, a compaction record
/// appends a marker, and anything else is appended as messages — emitting
/// the context window only when the arriving message is a user message or
/// tool result.
async fn process_session_input<A: SessionMessages>(
    agent: &mut A,
    ctx: AgentContext,
    port: String,
    value: AgentValue,
) -> Result<(), AgentError> {
    if port == PORT_RESET {
        let store = agent.store()?;
        let id = store.create(SessionMeta::new()).await?;
        publish_session_id(agent, &id)?;
        let state = agent.session_state_mut();
        state.session_id = Some(id.clone());
        state.entries.clear();
        state.pruned = false;
        // Publish the switch before the (ambiguous) empty context so
        // downstream agents can tell a reset from an empty session.
        agent
            .output(ctx.clone(), PORT_SESSION_ID, AgentValue::string(id))
            .await?;
        agent
            .output(ctx, PORT_MESSAGES, AgentValue::array_default())
            .await?;
        return Ok(());
    }

    if value.is_unit() {
        // Re-emit without appending; read-only, so nothing is pruned.
        return emit_window(agent, ctx, false).await;
    }

    // Dispatch by shape, before message conversion: Message objects carry
    // role/content and never a "type" key, so this cannot collide.
    if value.get_str("type") == Some("compaction") {
        let store = agent.store()?;
        return record_compaction(&store, agent.session_state_mut(), &value).await;
    }

    let Some(in_values) = to_message_batch(value)? else {
        return Ok(());
    };
    let mut in_messages: Vec<Message> = Vec::with_capacity(in_values.len());
    for value in &in_values {
        let message = value.as_message().ok_or_else(|| {
            AgentError::InvalidValue("Input contains non-Message values".to_string())
        })?;
        in_messages.push(message.clone());
    }

    let configs = agent.configs()?;
    let max_message_tokens = configs.get_integer_or_default(CONFIG_MAX_MESSAGE_TOKENS);
    let max_context_tokens = configs.get_integer_or_default(CONFIG_MAX_CONTEXT_TOKENS);

    // The arriving message — the batch's last non-streaming one — decides
    // whether the window is emitted: only a user message or tool result
    // gives a downstream Chat agent something to respond to.
    let trigger = in_messages
        .iter()
        .rposition(|m| !m.streaming)
        .map(|i| (i, in_messages[i].role.clone()));

    let store = agent.store()?;
    if let Some((anchor_index, role)) = &trigger
        && role == "user"
    {
        // Append everything before the anchor first, so the anchor's budget
        // trim sees the system message even when it arrived in this batch.
        let (head, tail) = in_messages.split_at(*anchor_index);
        append_messages(&store, agent.session_state_mut(), head, max_message_tokens).await?;
        let mut anchor = tail[0].clone();
        if max_message_tokens > 0
            && let Some(trimmed) = middle_trim(&anchor, max_message_tokens as u64)
        {
            anchor = trimmed;
        }
        if max_context_tokens > 0 {
            // The minimal window is the pinned system message plus this
            // user message; trim the user so at least that pair fits.
            let context = build_context(&agent.session_state_mut().entries);
            let system_tokens = context
                .iter()
                .rev()
                .find(|m| m.role == "system")
                .map(estimate_message_tokens)
                .unwrap_or(0);
            let budget = (max_context_tokens as u64).saturating_sub(system_tokens);
            if let Some(trimmed) = middle_trim(&anchor, budget) {
                anchor = trimmed;
            }
        }
        append_messages(&store, agent.session_state_mut(), &[anchor], 0).await?;
    } else {
        append_messages(
            &store,
            agent.session_state_mut(),
            &in_messages,
            max_message_tokens,
        )
        .await?;
    }

    match trigger.as_ref().map(|(_, role)| role.as_str()) {
        Some("user") | Some("tool") => emit_window(agent, ctx, true).await,
        _ => Ok(()),
    }
}

/// The context window selected by [`select_window`]: indices into the built
/// context of the first kept non-system message and of the pinned system
/// message.
struct Window {
    cut: usize,
    pinned_system: Option<usize>,
}

/// Selects the emitted window over a built context.
///
/// Returns `None` when nothing should be emitted: the context is empty,
/// does not end with a user message or tool result, or ends with a tool
/// result but holds no user message to head the window.
///
/// The last system message is pinned (hoisted to the front of the output);
/// other system messages are excluded. The minimal window — the latest user
/// message through the tail — is selected unconditionally; earlier
/// user-headed groups are then added newest-first while the budgets hold.
/// A group runs from a user message up to the next user message, so an
/// in-flight assistant/tool exchange is never split. Budgets of 0 or less
/// are inactive.
fn select_window(
    context: &[Message],
    max_context_tokens: i64,
    max_messages: i64,
) -> Option<Window> {
    let last = context.last()?;
    if last.role != "user" && last.role != "tool" {
        return None;
    }
    let anchor = context.iter().rposition(|m| m.role == "user")?;
    let pinned_system = context.iter().rposition(|m| m.role == "system");

    let use_tokens = max_context_tokens > 0;
    let use_count = max_messages > 0;

    let mut total_tokens: u64 = pinned_system
        .map(|i| estimate_message_tokens(&context[i]))
        .unwrap_or(0);
    let mut total_count = pinned_system.is_some() as u64;
    for message in &context[anchor..] {
        if message.role == "system" {
            continue;
        }
        total_tokens += estimate_message_tokens(message);
        total_count += 1;
    }

    let mut cut = anchor;
    let mut pending_tokens: u64 = 0;
    let mut pending_count: u64 = 0;
    for i in (0..anchor).rev() {
        let message = &context[i];
        if message.role == "system" {
            continue;
        }
        pending_tokens += estimate_message_tokens(message);
        pending_count += 1;
        if message.role == "user" {
            let fits_tokens =
                !use_tokens || total_tokens + pending_tokens <= max_context_tokens as u64;
            let fits_count = !use_count || total_count + pending_count <= max_messages as u64;
            if !(fits_tokens && fits_count) {
                break;
            }
            total_tokens += pending_tokens;
            total_count += pending_count;
            pending_tokens = 0;
            pending_count = 0;
            cut = i;
        }
    }

    Some(Window { cut, pinned_system })
}

/// Entry ids that fell out of the window and can be removed from the
/// session: Message entries before the first kept one (except the pinned
/// system message's), plus compaction markers — all of them when the
/// injected summary fell out of the window, all but the latest when it is
/// kept. Everything a kept marker still hides lies before its first kept
/// entry, so removal never changes what [`build_context`] returns.
fn prunable_entry_ids(
    entries: &[SessionEntry],
    context_ids: &[Option<String>],
    window: &Window,
) -> Vec<String> {
    // Entry index of the first kept context message that has a backing
    // entry. A window holding only the injected summary keeps everything
    // (conservative, and it costs nothing: the window is recomputed at
    // every emit).
    let cut_index = context_ids[window.cut..]
        .iter()
        .flatten()
        .next()
        .and_then(|id| entries.iter().position(|e| e.id() == id))
        .unwrap_or(0);
    let summary_kept = window.cut == 0 && context_ids.first().is_some_and(|id| id.is_none());
    let pinned_entry_id = window.pinned_system.and_then(|i| context_ids[i].as_deref());
    let latest_marker = entries
        .iter()
        .rposition(|e| matches!(e, SessionEntry::Compaction { .. }));

    entries
        .iter()
        .enumerate()
        .filter_map(|(i, entry)| match entry {
            SessionEntry::Compaction { id, .. } => {
                (!(summary_kept && Some(i) == latest_marker)).then(|| id.clone())
            }
            SessionEntry::Message { id, .. } => {
                (i < cut_index && pinned_entry_id != Some(id.as_str())).then(|| id.clone())
            }
        })
        .collect()
}

/// Emits the current context window on `messages`, or nothing when the
/// context does not end with a user message or tool result. With `prune`
/// set and a window budget active, entries outside the window are removed —
/// from the store first (per [`SessionMessages::prune_store`]), then from
/// the cache, so a cancellation mid-way leaves a superset that the next
/// emit re-trims.
async fn emit_window<A: SessionMessages>(
    agent: &mut A,
    ctx: AgentContext,
    prune: bool,
) -> Result<(), AgentError> {
    let configs = agent.configs()?;
    let max_context_tokens = configs.get_integer_or_default(CONFIG_MAX_CONTEXT_TOKENS);
    let max_messages = configs.get_integer_or_default(CONFIG_MAX_MESSAGES);

    let (context_ids, context): (Vec<Option<String>>, Vec<Message>) =
        build_context_with_ids(&agent.session_state_mut().entries)
            .into_iter()
            .unzip();
    let Some(window) = select_window(&context, max_context_tokens, max_messages) else {
        return Ok(());
    };

    let mut out: Vector<AgentValue> = Vector::new();
    if let Some(i) = window.pinned_system {
        out.push_back(context[i].clone().into());
    }
    for message in &context[window.cut..] {
        if message.role != "system" {
            out.push_back(message.clone().into());
        }
    }

    if prune && (max_context_tokens > 0 || max_messages > 0) {
        let prunable =
            prunable_entry_ids(&agent.session_state_mut().entries, &context_ids, &window);
        if !prunable.is_empty() {
            if agent.prune_store()? {
                let store = agent.store()?;
                let session_id = agent.session_state_mut().session_id()?.to_string();
                store.remove_entries(&session_id, &prunable).await?;
            }
            let state = agent.session_state_mut();
            state
                .entries
                .retain(|e| !prunable.iter().any(|id| id == e.id()));
            state.pruned = true;
        }
    }

    agent
        .output(ctx, PORT_MESSAGES, AgentValue::array(out))
        .await
}

/// Middle-trims a message's text so its estimated tokens fit
/// `budget_tokens`: the head and tail survive and the removed middle is
/// replaced with [`TRIM_MARKER`]. Only plain text is trimmed — thinking
/// blocks and images stay intact, so a message dominated by them can still
/// exceed the budget after trimming. Returns `None` when the message
/// already fits or nothing more can be trimmed.
fn middle_trim(message: &Message, budget_tokens: u64) -> Option<Message> {
    if estimate_message_tokens(message) <= budget_tokens {
        return None;
    }

    let text_len: usize = match &message.content {
        MessageContent::Text(text) => text.len(),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .map(|b| match b {
                ContentBlock::Text { text } => text.len(),
                _ => 0,
            })
            .sum(),
    };
    if text_len == 0 {
        return None;
    }

    // Whatever the untrimmable parts cost, the remaining budget goes to
    // text; the floor keeps a stub of the message even when they already
    // blow the budget on their own.
    let text_tokens = (text_len as u64).div_ceil(4);
    let other_tokens = estimate_message_tokens(message).saturating_sub(text_tokens);
    let budget_bytes = (budget_tokens.saturating_sub(other_tokens) as usize)
        .saturating_mul(4)
        .saturating_sub(TRIM_MARKER.len())
        .max(2 * TRIM_FLOOR_BYTES);
    if budget_bytes >= text_len {
        return None;
    }
    let head_budget = budget_bytes / 2;
    let tail_budget = budget_bytes - head_budget;

    let mut trimmed = message.clone();
    trimmed.content = match &message.content {
        MessageContent::Text(text) => {
            let head_end = floor_char_boundary(text, head_budget);
            let tail_start = ceil_char_boundary(text, text.len() - tail_budget);
            MessageContent::Text(format!(
                "{}{TRIM_MARKER}{}",
                &text[..head_end],
                &text[tail_start..]
            ))
        }
        MessageContent::Blocks(blocks) => MessageContent::Blocks(trim_blocks_middle(
            blocks,
            text_len,
            head_budget,
            tail_budget,
        )),
    };
    Some(trimmed)
}

/// Middle-trims the concatenated text of a block list: text before
/// `head_budget` and after `total_text_len - tail_budget` survives, the
/// marker lands at the cut, and non-text blocks pass through in place.
fn trim_blocks_middle(
    blocks: &[ContentBlock],
    total_text_len: usize,
    head_budget: usize,
    tail_budget: usize,
) -> Vec<ContentBlock> {
    let cut_start = head_budget;
    let cut_end = total_text_len - tail_budget;
    let mut out = Vec::with_capacity(blocks.len());
    let mut pos = 0usize;
    let mut marker_inserted = false;
    for block in blocks {
        let ContentBlock::Text { text } = block else {
            out.push(block.clone());
            continue;
        };
        let start = pos;
        let end = pos + text.len();
        pos = end;
        let mut kept = String::new();
        if start < cut_start {
            let head_end = floor_char_boundary(text, (cut_start - start).min(text.len()));
            kept.push_str(&text[..head_end]);
        }
        if !marker_inserted && end > cut_start {
            kept.push_str(TRIM_MARKER);
            marker_inserted = true;
        }
        if end > cut_end {
            let tail_start =
                ceil_char_boundary(text, cut_end.saturating_sub(start).min(text.len()));
            kept.push_str(&text[tail_start..]);
        }
        if !kept.is_empty() {
            out.push(ContentBlock::Text { text: kept });
        }
    }
    out
}

fn floor_char_boundary(s: &str, mut index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn ceil_char_boundary(s: &str, mut index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(index) {
        index += 1;
    }
    index
}

/// Accumulate messages in an in-memory session store.
///
/// Received messages are appended to a session (a conversation log). The
/// history lives in memory only: it is retained across agent stop/start
/// within the same process and lost when the process exits. To persist
/// sessions as files that survive restarts, use the File Messages agent
/// instead.
///
/// The conversation context is emitted on `messages` only when the arriving
/// message — the last non-streaming message of the input — is a user
/// message or a tool result, i.e. exactly when a downstream Chat agent has
/// something to respond to. Assistant messages are stored without emitting;
/// streaming partials (`streaming == true`) are ignored entirely, neither
/// stored nor emitted — emit intermediate output from the Chat agent's own
/// message port instead. The emitted array is prompt-ready: an optional
/// system message first (when the history holds several, only the last one
/// is kept), then messages starting with a user message and ending with the
/// user message or tool result that triggered the emit.
///
/// With `max_context_tokens` and/or `max_messages` set (> 0), the emitted
/// window is limited: starting from the latest user message, earlier
/// user-headed exchanges are added newest-first until a limit would be
/// exceeded, and history that fell out of the window is deleted from the
/// store. The limits are soft: the minimal window — the system message plus
/// the latest user message (through the tool result, when a tool exchange
/// is in flight) — is always emitted even when it exceeds them, the system
/// message is never trimmed and consumes budget, and tokens are estimated
/// (about 4 characters per token). Set the limits comfortably below the
/// model's context window.
///
/// `max_message_tokens` (> 0) caps each arriving non-system message: an
/// oversized message has the middle of its text replaced with a `[...]`
/// marker before it is stored. When the latest user message plus the system
/// message exceed `max_context_tokens`, the user message is middle-trimmed
/// the same way. Trimmed text replaces the stored message — the original is
/// not kept — and messages already in the session (e.g. resumed history)
/// are not re-trimmed.
///
/// An input on `reset` starts a new session: a fresh `session_id` is issued,
/// written back to the config, and emitted on the `session_id` port, then an
/// empty array is emitted on `messages`. The previous session is left
/// untouched; to resume a past conversation, set `session_id` to its id and
/// restart the agent.
///
/// The `message` port also accepts a compaction record — an object whose
/// `type` key is `"compaction"` — as emitted by the `compaction` output of
/// the Compact Messages agent. The record is stored as a non-destructive
/// compaction marker (its `dropped` count resolved to the first kept entry
/// id) and **nothing is emitted**: emitting would re-send the compacted
/// context and re-trigger a downstream Chat agent. From the next input on,
/// the emitted context starts with the summary followed by the kept
/// messages. A record whose `previous_summary` baseline no longer matches
/// the session's latest compaction — or whose `dropped` count exceeds the
/// session's messages, or whose `tokens_before` is more than twice the
/// session's current estimate (first compactions only, which carry no
/// baseline to compare), or which carries no baseline after this agent has
/// deleted history — is discarded with a warning instead of being recorded:
/// the summarization call behind a record takes seconds, and a `reset`, a
/// second compaction, or a window deletion in that time makes the record
/// stale.
///
/// Patches saved before session support carried the history in a hidden
/// `messages` config. On the first start that history is imported once into
/// the session store (only if the session has no messages yet); the stale
/// config key is dropped afterwards.
///
/// # Ports
/// - Input `message`: Message or array of messages to append. The context
///   window is emitted only when the last non-streaming message is a user
///   message or tool result. A unit value re-emits the current window
///   without appending — and emits nothing when the context does not end
///   with a user message or tool result. A compaction record (object with
///   `type` `"compaction"`, a non-empty `summary`, a non-negative integer
///   `dropped`, an optional `tokens_before`, and an optional
///   `previous_summary` baseline) appends a compaction marker to the
///   session and emits nothing; a stale record is discarded
/// - Input `reset`: Start a new session and emit an empty array
/// - Output `messages`: Prompt-ready context window as an array of messages
/// - Output `session_id`: The freshly issued session id, emitted when
///   `reset` switches to a new session
///
/// # Configuration
/// - `max_context_tokens`: Estimated-token budget for the emitted window;
///   history outside the window is deleted. 0 disables the limit
///   (default: 0)
/// - `max_messages`: Maximum number of emitted messages, counting the
///   system message and an injected compaction summary; history outside the
///   window is deleted. 0 disables the limit (default: 0)
/// - `max_message_tokens`: Estimated-token cap for each arriving non-system
///   message; the middle of an oversized message is cut before it is
///   stored. 0 disables the cap (default: 0)
/// - `session_id`: Session to resume on start. Empty: a new session is
///   created and its id is written back to this config (default: "")
#[modular_agent(
    title="Messages",
    category=CATEGORY,
    inputs=[PORT_MESSAGE, PORT_RESET],
    outputs=[PORT_MESSAGES, PORT_SESSION_ID],
    integer_config(name=CONFIG_MAX_CONTEXT_TOKENS),
    integer_config(name=CONFIG_MAX_MESSAGES),
    integer_config(name=CONFIG_MAX_MESSAGE_TOKENS),
    string_config(name=CONFIG_SESSION_ID, default="", detail),
    hint(width = 2, height = 1),
)]
pub struct MessagesAgent {
    data: AgentData,

    /// The agent instance owns its store, and keeping it here across
    /// stop()/start() is what preserves the history for the lifetime of
    /// the process.
    store: Option<Arc<dyn SessionStore>>,

    state: SessionState,

    /// History read from the stale `_messages` config, imported into the
    /// store once on the first `start()`.
    pending_import: Option<Vec<Message>>,
}

impl MessagesAgent {
    fn resolve_store(&mut self) -> Arc<dyn SessionStore> {
        if let Some(store) = &self.store {
            return store.clone();
        }
        let store: Arc<dyn SessionStore> = Arc::new(InMemorySessionStore::new());
        self.store = Some(store.clone());
        store
    }
}

impl SessionMessages for MessagesAgent {
    fn store(&self) -> Result<Arc<dyn SessionStore>, AgentError> {
        self.store
            .clone()
            .ok_or_else(|| AgentError::Other("Session store is not initialized".to_string()))
    }

    fn session_state_mut(&mut self) -> &mut SessionState {
        &mut self.state
    }
}

#[async_trait]
impl AsAgent for MessagesAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        // Read the stale keys here: AgentData::new() strips `_`-prefixed
        // config keys preserved by reconcile_spec().
        let pending_import: Option<Vec<Message>> = spec
            .configs
            .as_ref()
            .and_then(|c| c.get(STALE_CONFIG_MESSAGES).ok())
            .and_then(|v| v.to_message_value())
            .map(|v| {
                let arr = if v.is_array() {
                    v.into_array().unwrap_or_default()
                } else {
                    vector![v]
                };
                arr.iter().filter_map(|m| m.as_message().cloned()).collect()
            })
            .filter(|messages: &Vec<Message>| !messages.is_empty());

        // A leftover `session_dir` means this node persisted its sessions
        // before the in-memory/file split; that is now the File Messages
        // agent's job.
        let stale_dir = spec
            .configs
            .as_ref()
            .and_then(|c| c.get(STALE_CONFIG_SESSION_DIR).ok());
        if stale_dir
            .as_ref()
            .and_then(|v| v.as_str())
            .is_some_and(|d| !d.is_empty())
        {
            log::warn!(
                "Messages agent {id} no longer saves sessions to disk; \
                 replace it with a File Messages agent to keep them in files"
            );
        }

        Ok(Self {
            data: AgentData::new(ma, id, spec),
            store: None,
            state: SessionState::default(),
            pending_import,
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        let store = self.resolve_store();

        let configured_id = self.configs()?.get_string_or_default(CONFIG_SESSION_ID);
        let issued_new = configured_id.is_empty();
        let (session_id, entries) = resolve_session(&store, configured_id).await?;
        if issued_new {
            publish_session_id(self, &session_id)?;
        }
        self.state.session_id = Some(session_id.clone());
        self.state.entries = entries;
        self.state.pruned = false;

        // One-way migration of the pre-session `messages` config. The
        // pending history is cleared only once the import is resolved: a
        // mid-import append failure below keeps it, so a retried start()
        // lands in the warn branch and reports the partial state instead of
        // dropping the tail silently.
        if let Some(imported) = self.pending_import.clone() {
            let has_messages = self
                .state
                .entries
                .iter()
                .any(|e| matches!(e, SessionEntry::Message { .. }));
            if has_messages {
                log::warn!(
                    "Skipping legacy `messages` history import ({} messages): \
                     session {session_id} already has messages",
                    imported.len()
                );
            } else {
                for message in imported {
                    if message.streaming {
                        continue;
                    }
                    let entry = SessionEntry::message(message);
                    store.append(&session_id, entry.clone()).await?;
                    self.state.entries.push(entry);
                }
            }
            self.pending_import = None;
        }

        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        process_session_input(self, ctx, port, value).await
    }
}

/// Accumulate messages in JSONL session files.
///
/// Received messages are appended to a session (a conversation log)
/// persisted as `<session_dir>/<session_id>.jsonl`. Sessions survive
/// restarts; to keep the history in memory only, use the Messages agent
/// instead.
///
/// The conversation context is emitted on `messages` only when the arriving
/// message — the last non-streaming message of the input — is a user
/// message or a tool result, i.e. exactly when a downstream Chat agent has
/// something to respond to. Assistant messages are stored without emitting;
/// streaming partials (`streaming == true`) are ignored entirely, neither
/// stored nor emitted — emit intermediate output from the Chat agent's own
/// message port instead. The emitted array is prompt-ready: an optional
/// system message first (when the history holds several, only the last one
/// is kept), then messages starting with a user message and ending with the
/// user message or tool result that triggered the emit.
///
/// With `max_context_tokens` and/or `max_messages` set (> 0), the emitted
/// window is limited: starting from the latest user message, earlier
/// user-headed exchanges are added newest-first until a limit would be
/// exceeded. History that fell out of the window is dropped from memory,
/// and — while `prune_file` is on — deleted from the session file as well.
/// The limits are soft: the minimal window — the system message plus the
/// latest user message (through the tool result, when a tool exchange is in
/// flight) — is always emitted even when it exceeds them, the system
/// message is never trimmed and consumes budget, and tokens are estimated
/// (about 4 characters per token). Set the limits comfortably below the
/// model's context window.
///
/// `max_message_tokens` (> 0) caps each arriving non-system message: an
/// oversized message has the middle of its text replaced with a `[...]`
/// marker before it is stored. When the latest user message plus the system
/// message exceed `max_context_tokens`, the user message is middle-trimmed
/// the same way. Trimmed text replaces the stored message — the original is
/// not kept — and messages already in the session (e.g. resumed history)
/// are not re-trimmed.
///
/// An input on `reset` starts a new session: a fresh `session_id` is issued,
/// written back to the config, and emitted on the `session_id` port, then an
/// empty array is emitted on `messages`. The previous session is left
/// untouched; to resume a past conversation, set `session_id` to its id and
/// restart the agent.
///
/// The `message` port also accepts a compaction record — an object whose
/// `type` key is `"compaction"` — as emitted by the `compaction` output of
/// the Compact Messages agent. The record is stored as a non-destructive
/// compaction marker (its `dropped` count resolved to the first kept entry
/// id) and **nothing is emitted**: emitting would re-send the compacted
/// context and re-trigger a downstream Chat agent. From the next input on,
/// the emitted context starts with the summary followed by the kept
/// messages. A record whose `previous_summary` baseline no longer matches
/// the session's latest compaction — or whose `dropped` count exceeds the
/// session's messages, or whose `tokens_before` is more than twice the
/// session's current estimate (first compactions only, which carry no
/// baseline to compare), or which carries no baseline after this agent has
/// deleted history — is discarded with a warning instead of being recorded:
/// the summarization call behind a record takes seconds, and a `reset`, a
/// second compaction, or a window deletion in that time makes the record
/// stale.
///
/// `session_dir` is applied when the agent starts; the agent fails to start
/// while it is empty. Changing it while the agent is running makes further
/// inputs fail with a config error until the agent is restarted.
///
/// # Ports
/// - Input `message`: Message or array of messages to append. The context
///   window is emitted only when the last non-streaming message is a user
///   message or tool result. A unit value re-emits the current window
///   without appending — and emits nothing when the context does not end
///   with a user message or tool result. A compaction record (object with
///   `type` `"compaction"`, a non-empty `summary`, a non-negative integer
///   `dropped`, an optional `tokens_before`, and an optional
///   `previous_summary` baseline) appends a compaction marker to the
///   session and emits nothing; a stale record is discarded
/// - Input `reset`: Start a new session and emit an empty array
/// - Output `messages`: Prompt-ready context window as an array of messages
/// - Output `session_id`: The freshly issued session id, emitted when
///   `reset` switches to a new session
///
/// # Configuration
/// - `session_dir`: Directory for the JSONL session files. Required; the
///   agent fails to start while it is empty (default: "")
/// - `max_context_tokens`: Estimated-token budget for the emitted window;
///   history outside the window is dropped (see `prune_file`). 0 disables
///   the limit (default: 0)
/// - `max_messages`: Maximum number of emitted messages, counting the
///   system message and an injected compaction summary; history outside the
///   window is dropped (see `prune_file`). 0 disables the limit (default: 0)
/// - `max_message_tokens`: Estimated-token cap for each arriving non-system
///   message; the middle of an oversized message is cut before it is
///   stored. 0 disables the cap (default: 0)
/// - `prune_file`: Also delete history that fell out of the window from the
///   session file; memory always drops it. Beware: resuming a long session
///   with a window limit set irreversibly deletes everything outside the
///   window on the first emit, and when two agents share one session file,
///   a rewrite by one can lose an append the other made meanwhile. Turn off
///   to keep the full history on disk (default: true)
/// - `session_id`: Session to resume on start. Empty: a new session is
///   created and its id is written back to this config (default: "")
#[modular_agent(
    title="File Messages",
    category=CATEGORY,
    inputs=[PORT_MESSAGE, PORT_RESET],
    outputs=[PORT_MESSAGES, PORT_SESSION_ID],
    string_config(name=CONFIG_SESSION_DIR, default=""),
    integer_config(name=CONFIG_MAX_CONTEXT_TOKENS),
    integer_config(name=CONFIG_MAX_MESSAGES),
    integer_config(name=CONFIG_MAX_MESSAGE_TOKENS),
    boolean_config(name=CONFIG_PRUNE_FILE, default=true, detail),
    string_config(name=CONFIG_SESSION_ID, default="", detail),
    hint(width = 2, height = 1),
)]
pub struct FileMessagesAgent {
    data: AgentData,

    /// Active store tagged with the `session_dir` it was created for.
    store: Option<(String, Arc<dyn SessionStore>)>,

    state: SessionState,
}

impl FileMessagesAgent {
    fn resolve_store(&mut self) -> Result<Arc<dyn SessionStore>, AgentError> {
        let dir = self.configs()?.get_string_or_default(CONFIG_SESSION_DIR);
        if dir.is_empty() {
            return Err(AgentError::InvalidConfig(
                "session_dir is required".to_string(),
            ));
        }
        if let Some((store_dir, store)) = &self.store
            && *store_dir == dir
        {
            return Ok(store.clone());
        }
        let store: Arc<dyn SessionStore> = Arc::new(JsonlSessionStore::new(&dir));
        self.store = Some((dir, store.clone()));
        Ok(store)
    }
}

impl SessionMessages for FileMessagesAgent {
    fn store(&self) -> Result<Arc<dyn SessionStore>, AgentError> {
        let (store_dir, store) = self
            .store
            .as_ref()
            .ok_or_else(|| AgentError::Other("Session store is not initialized".to_string()))?;
        // The store is bound to session_dir in start(). Failing loudly on a
        // runtime mismatch beats silently writing into the old directory
        // while the config points at the new one.
        let dir = self.configs()?.get_string_or_default(CONFIG_SESSION_DIR);
        if *store_dir != dir {
            return Err(AgentError::InvalidConfig(
                "session_dir changed while the agent is running; restart the agent to apply it"
                    .to_string(),
            ));
        }
        Ok(store.clone())
    }

    fn session_state_mut(&mut self) -> &mut SessionState {
        &mut self.state
    }

    fn prune_store(&self) -> Result<bool, AgentError> {
        Ok(self.configs()?.get_bool_or(CONFIG_PRUNE_FILE, true))
    }
}

#[async_trait]
impl AsAgent for FileMessagesAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            store: None,
            state: SessionState::default(),
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        let store = self.resolve_store()?;

        let configured_id = self.configs()?.get_string_or_default(CONFIG_SESSION_ID);
        let issued_new = configured_id.is_empty();
        let (session_id, entries) = resolve_session(&store, configured_id).await?;
        if issued_new {
            publish_session_id(self, &session_id)?;
        }
        self.state.session_id = Some(session_id);
        self.state.entries = entries;
        self.state.pruned = false;
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        process_session_input(self, ctx, port, value).await
    }
}

/// Trim a message history to fit a prompt budget.
///
/// For message streams assembled without the session-backed agents; the
/// Messages and File Messages agents apply their own `max_context_tokens`
/// window before emitting and do not need this agent downstream.
///
/// Selects messages from newest to oldest until the budget is exhausted,
/// so the most recent conversation always survives. A leading system
/// message is always kept and counts against the budget. After selection,
/// non-user messages are dropped from the front until the first
/// non-system message is a user message, yielding the (system,) user,
/// (assistant, user)* order providers expect. Unsigned thinking blocks
/// are stripped from the selected messages; signed and redacted blocks
/// are preserved for provider replay. When neither budget is set (both
/// <= 0), the input passes through unchanged; an empty input emits
/// nothing.
///
/// # Ports
/// - Input `messages`: Message or array of messages (full history)
/// - Output `messages`: The trimmed message array
///
/// # Configuration
/// - `max_tokens`: Token budget, using the core `estimate_message_tokens`
///   heuristic (chars/4, flat cost per image). Takes precedence over
///   `max_size` when > 0. In this mode each message is measured after
///   unsigned thinking is stripped — the form actually sent (default: 0)
/// - `max_size`: Legacy byte budget: text bytes, tool-call name and
///   serialized parameter bytes, and estimated image file sizes. Used
///   only when `max_tokens` <= 0 (default: 0)
#[modular_agent(
    title="Messages for Prompt",
    category=CATEGORY,
    inputs=[PORT_MESSAGES],
    outputs=[PORT_MESSAGES],
    integer_config(name=CONFIG_MAX_TOKENS),
    integer_config(name=CONFIG_MAX_SIZE),
    hint(width = 2, height = 1),
)]
pub struct MessagesForPromptAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for MessagesForPromptAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let configs = self.configs()?;
        let max_size = configs.get_integer_or_default(CONFIG_MAX_SIZE);
        let max_tokens = configs.get_integer_or_default(CONFIG_MAX_TOKENS);
        if max_size <= 0 && max_tokens <= 0 {
            // Just output the input messages
            self.output(ctx, PORT_MESSAGES, value).await?;
            return Ok(());
        }

        let messages_value = value.to_message_value().ok_or_else(|| {
            AgentError::InvalidValue("Input contains non-Message values".to_string())
        })?;
        let mut messages = if messages_value.is_array() {
            messages_value.as_array().unwrap().clone()
        } else {
            vector![messages_value]
        };
        if messages.is_empty() {
            return Ok(());
        }

        // The estimated-token budget is more faithful to what providers
        // actually charge, so it takes precedence over the byte budget.
        let use_tokens = max_tokens > 0;
        let budget = if use_tokens {
            max_tokens as u64
        } else {
            max_size as u64
        };
        let mut total: u64 = 0;

        // Extract system message if exists
        let mut system_message: Option<AgentValue> = None;
        if messages.front().unwrap().as_message().unwrap().role == "system" {
            let msg = messages.pop_front().unwrap();
            let m = msg.as_message().unwrap();
            total += if use_tokens {
                estimate_message_tokens(m)
            } else {
                m.text().len() as u64
            };
            system_message = Some(msg);
        }

        // Collect messages in reverse order
        let mut selected_messages: Vec<AgentValue> = Vec::with_capacity(messages.len());
        while !messages.is_empty() {
            let value = messages.pop_back().unwrap();
            let msg = value.as_message().unwrap();

            // Stripping happens before measuring: the stripped form is what
            // actually gets sent, so it is what must fit the budget.
            let stripped = strip_unsigned_thinking(msg);

            let msg_size: u64 = if use_tokens {
                estimate_message_tokens(stripped.as_ref().unwrap_or(msg))
            } else {
                let mut size = msg.text().len() as u64;

                // text() skips image blocks, but their base64 payloads are
                // sent to the provider, so they must count toward the budget
                // — otherwise image tool results measure as ~0 bytes and
                // trimming never triggers.
                if let MessageContent::Blocks(blocks) = &msg.content {
                    for block in blocks {
                        if let ContentBlock::Image { data, .. } = block {
                            size += data.len() as u64;
                        }
                    }
                }

                #[cfg(feature = "image")]
                if let Some(img) = &msg.image {
                    size += img.get_estimated_filesize();
                }

                if let Some(tool_calls) = &msg.tool_calls {
                    for call in tool_calls {
                        size += call.function.name.len() as u64;
                        size += serde_json::to_string(&call.function.parameters)
                            .map_or(0, |s| s.len() as u64);
                    }
                }

                size
            };

            if total + msg_size > budget {
                break;
            }
            total += msg_size;

            if let Some(m) = stripped {
                selected_messages.push(AgentValue::message(m));
            } else {
                selected_messages.push(value);
            }
        }

        // Ensure the first message is user
        while let Some(last_msg) = selected_messages.last() {
            let role = last_msg.as_message().unwrap().role.as_str();
            if role != "user" {
                selected_messages.pop();
            } else {
                break;
            }
        }

        if let Some(system_message) = system_message {
            selected_messages.push(system_message);
        }

        selected_messages.reverse();
        self.output(ctx, PORT_MESSAGES, selected_messages.into())
            .await?;

        Ok(())
    }
}

/// Drop unsigned thinking blocks to save prompt budget when trimming
/// history, flattening back to the legacy string form when only text
/// remains. Signed and redacted blocks survive untouched: Claude requires
/// them verbatim when an extended-thinking + tool-use turn is replayed,
/// so removing them would fail the continuation request. Returns `None`
/// when the message has nothing to strip.
fn strip_unsigned_thinking(msg: &Message) -> Option<Message> {
    fn is_unsigned(block: &ContentBlock) -> bool {
        matches!(
            block,
            ContentBlock::Thinking {
                signature: None,
                redacted: false,
                ..
            }
        )
    }

    let MessageContent::Blocks(blocks) = &msg.content else {
        return None;
    };
    if !blocks.iter().any(is_unsigned) {
        return None;
    }
    let kept: Vec<ContentBlock> = blocks.iter().filter(|b| !is_unsigned(b)).cloned().collect();
    let mut m = msg.clone();
    m.content = if kept.iter().all(|b| matches!(b, ContentBlock::Text { .. })) {
        kept.iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>()
            .into()
    } else {
        MessageContent::Blocks(kept)
    };
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use im::hashmap;
    use modular_agent_core::test_utils::{ProbeReceiver, TestProbeAgent, probe_receiver};
    use modular_agent_core::{AgentStatus, ConnectionSpec};

    /// `start_patch` returns before the spawned agent loop has run
    /// `AsAgent::start`; wait until the status flips to `Start` (set under
    /// the same lock as `start()`, so seeing it means `start()` finished).
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

    /// Build a running patch with a session-backed agent of `def_name`
    /// (configured via `configs`) whose `messages` and `session_id` ports
    /// each feed a probe.
    async fn setup_session_agent(
        def_name: &str,
        configs: Vec<(&str, AgentValue)>,
    ) -> (ModularAgent, String, String, ProbeReceiver, ProbeReceiver) {
        let ma = ModularAgent::init().unwrap();
        ma.ready().await.unwrap();

        let patch_id = ma.new_patch().unwrap();
        let def = ma.get_agent_definition(def_name).unwrap();
        let mut spec = def.to_spec();
        {
            let spec_configs = spec.configs.as_mut().unwrap();
            for (key, value) in configs {
                spec_configs.set(key.into(), value);
            }
        }
        let agent_id = ma.add_agent(patch_id.clone(), spec).await.unwrap();

        let probe_def = ma.get_agent_definition(TestProbeAgent::DEF_NAME).unwrap();
        let probe_id = ma
            .add_agent(patch_id.clone(), probe_def.to_spec())
            .await
            .unwrap();
        ma.add_connection(
            &patch_id,
            ConnectionSpec {
                source: agent_id.clone(),
                source_handle: PORT_MESSAGES.into(),
                target: probe_id.clone(),
                target_handle: "value".into(),
            },
        )
        .await
        .unwrap();

        let session_probe_id = ma
            .add_agent(patch_id.clone(), probe_def.to_spec())
            .await
            .unwrap();
        ma.add_connection(
            &patch_id,
            ConnectionSpec {
                source: agent_id.clone(),
                source_handle: PORT_SESSION_ID.into(),
                target: session_probe_id.clone(),
                target_handle: "value".into(),
            },
        )
        .await
        .unwrap();

        ma.start_patch(&patch_id).await.unwrap();
        wait_until_started(&ma, &agent_id).await;
        let probe_rx = probe_receiver(&ma, &probe_id).await.unwrap();
        let session_rx = probe_receiver(&ma, &session_probe_id).await.unwrap();

        (ma, patch_id, agent_id, probe_rx, session_rx)
    }

    async fn setup_messages_agent(
        configs: Vec<(&str, AgentValue)>,
    ) -> (ModularAgent, String, String, ProbeReceiver, ProbeReceiver) {
        setup_session_agent(MessagesAgent::DEF_NAME, configs).await
    }

    async fn setup_file_messages_agent(
        configs: Vec<(&str, AgentValue)>,
    ) -> (ModularAgent, String, String, ProbeReceiver, ProbeReceiver) {
        setup_session_agent(FileMessagesAgent::DEF_NAME, configs).await
    }

    async fn send_as<T: AsAgent>(ma: &ModularAgent, agent_id: &str, port: &str, value: AgentValue) {
        let agent = ma.get_agent(agent_id).unwrap();
        let mut guard = agent.lock().await;
        let target = guard.as_agent_mut::<T>().unwrap();
        AsAgent::process(target, AgentContext::new(), port.to_string(), value)
            .await
            .unwrap();
    }

    async fn send(ma: &ModularAgent, agent_id: &str, port: &str, value: AgentValue) {
        send_as::<MessagesAgent>(ma, agent_id, port, value).await
    }

    async fn send_file(ma: &ModularAgent, agent_id: &str, port: &str, value: AgentValue) {
        send_as::<FileMessagesAgent>(ma, agent_id, port, value).await
    }

    async fn session_id_config(ma: &ModularAgent, agent_id: &str) -> String {
        let agent = ma.get_agent(agent_id).unwrap();
        let guard = agent.lock().await;
        guard
            .configs()
            .unwrap()
            .get_string_or_default(CONFIG_SESSION_ID)
    }

    async fn recv_messages(probe_rx: &ProbeReceiver) -> Vec<Message> {
        let (_ctx, value) = probe_rx.recv().await.unwrap();
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_message().unwrap().clone())
            .collect()
    }

    async fn assert_no_emit(probe_rx: &ProbeReceiver) {
        assert!(
            probe_rx
                .recv_with_timeout(std::time::Duration::from_millis(100))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn file_messages_agent_persists_only_finalized_messages() {
        let dir = tempfile::tempdir().unwrap();
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) = setup_file_messages_agent(vec![(
            CONFIG_SESSION_DIR,
            AgentValue::string(dir.path().to_string_lossy()),
        )])
        .await;

        // A streaming partial is ignored entirely: nothing emitted, nothing
        // stored.
        let mut partial = Message::assistant("Hel".to_string());
        partial.id = Some("m1".to_string());
        partial.streaming = true;
        send_file(&ma, &agent_id, PORT_MESSAGE, partial.into()).await;
        assert_no_emit(&probe_rx).await;

        // A final assistant message is stored but not emitted either.
        let mut fin = Message::assistant("Hello".to_string());
        fin.id = Some("m1".to_string());
        send_file(&ma, &agent_id, PORT_MESSAGE, fin.into()).await;
        assert_no_emit(&probe_rx).await;

        // A fresh replay of the same session contains only the final message.
        let session_id = session_id_config(&ma, &agent_id).await;
        let store = JsonlSessionStore::new(dir.path());
        let entries = store.load(&session_id).await.unwrap();
        assert_eq!(entries.len(), 1);
        let SessionEntry::Message { message, .. } = &entries[0] else {
            panic!("expected a Message entry");
        };
        assert!(!message.streaming);
        assert_eq!(message.text(), "Hello");

        // A user arrival emits; the leading assistant cannot head the
        // window, so only the user message appears — but with no limits set
        // nothing is deleted from the store.
        send_file(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("q".to_string()).into(),
        )
        .await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text(), "q");
        assert_eq!(store.load(&session_id).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn file_messages_agent_reset_starts_new_session_and_keeps_old_one() {
        let dir = tempfile::tempdir().unwrap();
        let (ma, _patch_id, agent_id, probe_rx, session_rx) = setup_file_messages_agent(vec![(
            CONFIG_SESSION_DIR,
            AgentValue::string(dir.path().to_string_lossy()),
        )])
        .await;

        send_file(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("hello".to_string()).into(),
        )
        .await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 1);
        let old_session_id = session_id_config(&ma, &agent_id).await;

        send_file(&ma, &agent_id, PORT_RESET, AgentValue::unit()).await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 0);

        // A new session id was issued and written back to the config.
        let new_session_id = session_id_config(&ma, &agent_id).await;
        assert_ne!(new_session_id, old_session_id);
        assert!(!new_session_id.is_empty());

        // The switch was also published on the session_id output port.
        let (_ctx, value) = session_rx.recv().await.unwrap();
        assert_eq!(value.as_str(), Some(new_session_id.as_str()));

        // The old session file still exists with its entries.
        assert!(dir.path().join(format!("{old_session_id}.jsonl")).exists());
        let store = JsonlSessionStore::new(dir.path());
        let old_entries = store.load(&old_session_id).await.unwrap();
        assert_eq!(old_entries.len(), 1);

        // New inputs land in the new session only.
        send_file(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("next".to_string()).into(),
        )
        .await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text(), "next");
    }

    #[tokio::test]
    async fn file_messages_agent_resumes_existing_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonlSessionStore::new(dir.path());
        let session_id = store.create(SessionMeta::new()).await.unwrap();
        store
            .append(
                &session_id,
                SessionEntry::message(Message::user("a".to_string())),
            )
            .await
            .unwrap();
        store
            .append(
                &session_id,
                SessionEntry::message(Message::assistant("b".to_string())),
            )
            .await
            .unwrap();

        let (ma, _patch_id, agent_id, probe_rx, _session_rx) = setup_file_messages_agent(vec![
            (
                CONFIG_SESSION_DIR,
                AgentValue::string(dir.path().to_string_lossy()),
            ),
            (CONFIG_SESSION_ID, AgentValue::string(session_id.clone())),
        ])
        .await;

        // The replayed history ends with an assistant message, so a unit
        // input emits nothing.
        send_file(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        assert_no_emit(&probe_rx).await;

        // The next user message extends the same session and emits the
        // whole replayed history.
        send_file(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("c".to_string()).into(),
        )
        .await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text(), "a");
        assert_eq!(messages[1].text(), "b");
        assert_eq!(messages[2].text(), "c");
        assert_eq!(store.load(&session_id).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn file_messages_agent_requires_session_dir() {
        let ma = ModularAgent::init().unwrap();
        ma.ready().await.unwrap();

        let patch_id = ma.new_patch().unwrap();
        let def = ma
            .get_agent_definition(FileMessagesAgent::DEF_NAME)
            .unwrap();
        let agent_id = ma.add_agent(patch_id, def.to_spec()).await.unwrap();

        let agent = ma.get_agent(&agent_id).unwrap();
        let mut guard = agent.lock().await;
        let file_agent = guard.as_agent_mut::<FileMessagesAgent>().unwrap();
        let result = AsAgent::start(file_agent).await;
        assert!(matches!(result, Err(AgentError::InvalidConfig(_))));
    }

    #[tokio::test]
    async fn messages_agent_migrates_stale_messages_config_once() {
        let old_messages = AgentValue::array(vector![
            AgentValue::object(hashmap! {
                "role".into() => AgentValue::string("user"),
                "content".into() => AgentValue::string("old user"),
            }),
            AgentValue::object(hashmap! {
                "role".into() => AgentValue::string("assistant"),
                "content".into() => AgentValue::string("old assistant"),
            }),
        ]);
        let (ma, patch_id, agent_id, probe_rx, _session_rx) =
            setup_messages_agent(vec![(STALE_CONFIG_MESSAGES, old_messages)]).await;

        // The old history landed in the (in-memory) store: a user arrival
        // emits it in full.
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("q".to_string()).into(),
        )
        .await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text(), "old user");
        assert_eq!(messages[1].text(), "old assistant");
        assert_eq!(messages[2].text(), "q");

        // A stop()/start() cycle must not import again; the in-memory store
        // is retained across the cycle.
        ma.stop_patch(&patch_id).await.unwrap();
        ma.start_patch(&patch_id).await.unwrap();
        wait_until_started(&ma, &agent_id).await;

        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text(), "old user");
        assert_eq!(messages[1].text(), "old assistant");
        assert_eq!(messages[2].text(), "q");
    }

    #[tokio::test]
    async fn messages_agent_emits_only_on_user_or_tool_arrivals() {
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) = setup_messages_agent(vec![]).await;

        // Unit input on an empty session emits nothing.
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        assert_no_emit(&probe_rx).await;

        // A batch ending with an assistant message appends without emitting;
        // so does a unit input while the context ends with an assistant.
        let batch = AgentValue::array(vector![
            Message::user("a".to_string()).into(),
            Message::assistant("b".to_string()).into(),
        ]);
        send(&ma, &agent_id, PORT_MESSAGE, batch).await;
        assert_no_emit(&probe_rx).await;
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        assert_no_emit(&probe_rx).await;

        // A user arrival emits the accumulated context in order.
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("c".to_string()).into(),
        )
        .await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text(), "a");
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].text(), "b");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[2].text(), "c");

        // Unit input now re-emits the same window.
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 3);
    }

    #[tokio::test]
    async fn messages_agent_tool_result_emits_whole_exchange() {
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) = setup_messages_agent(vec![]).await;

        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("q".to_string()).into(),
        )
        .await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 1);

        // The assistant's tool call appends silently; the tool result
        // re-emits the context so a downstream Chat agent can continue.
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::assistant("calling".to_string()).into(),
        )
        .await;
        assert_no_emit(&probe_rx).await;
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::tool("my_tool".to_string(), "result".to_string()).into(),
        )
        .await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text(), "q");
        assert_eq!(messages[1].text(), "calling");
        assert_eq!(messages[2].role, "tool");
    }

    #[tokio::test]
    async fn messages_agent_tool_tail_without_user_emits_nothing() {
        // A tool exchange with no user message anywhere (e.g. right after a
        // reset raced a tool loop) has no window head: nothing is emitted
        // and nothing is deleted, even with limits active.
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) =
            setup_messages_agent(vec![(CONFIG_MAX_CONTEXT_TOKENS, AgentValue::integer(5))]).await;

        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::assistant("calling".to_string()).into(),
        )
        .await;
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::tool("my_tool".to_string(), "result".to_string()).into(),
        )
        .await;
        assert_no_emit(&probe_rx).await;

        // The exchange is still in the session: the next user arrival
        // emits, with the orphan exchange excluded from the window.
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("q".to_string()).into(),
        )
        .await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text(), "q");
    }

    fn compaction_record(
        summary: Option<&str>,
        dropped: Option<i64>,
        tokens_before: Option<i64>,
    ) -> AgentValue {
        compaction_record_with_previous(summary, dropped, tokens_before, None)
    }

    fn compaction_record_with_previous(
        summary: Option<&str>,
        dropped: Option<i64>,
        tokens_before: Option<i64>,
        previous_summary: Option<&str>,
    ) -> AgentValue {
        let mut map = hashmap! {
            "type".to_string() => AgentValue::string("compaction"),
        };
        if let Some(s) = summary {
            map.insert("summary".to_string(), AgentValue::string(s));
        }
        if let Some(d) = dropped {
            map.insert("dropped".to_string(), AgentValue::integer(d));
        }
        if let Some(t) = tokens_before {
            map.insert("tokens_before".to_string(), AgentValue::integer(t));
        }
        if let Some(p) = previous_summary {
            map.insert("previous_summary".to_string(), AgentValue::string(p));
        }
        AgentValue::object(map)
    }

    fn message_entry_ids(entries: &[SessionEntry]) -> Vec<String> {
        entries
            .iter()
            .filter_map(|e| match e {
                SessionEntry::Message { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn file_messages_agent_compaction_record_appends_marker_and_emits_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) = setup_file_messages_agent(vec![(
            CONFIG_SESSION_DIR,
            AgentValue::string(dir.path().to_string_lossy()),
        )])
        .await;

        let batch = AgentValue::array(vector![
            Message::user("a".to_string()).into(),
            Message::assistant("b".to_string()).into(),
            Message::user("c".to_string()).into(),
            Message::assistant("d".to_string()).into(),
        ]);
        send_file(&ma, &agent_id, PORT_MESSAGE, batch).await;
        assert_no_emit(&probe_rx).await;

        // The record appends a marker and emits nothing (an emit here would
        // re-trigger a downstream ChatAgent with the compacted context).
        send_file(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            compaction_record(Some("S"), Some(2), Some(3)),
        )
        .await;
        assert_no_emit(&probe_rx).await;

        // The marker landed in the store, dropped=2 resolved to the third
        // message's entry id.
        let session_id = session_id_config(&ma, &agent_id).await;
        let store = JsonlSessionStore::new(dir.path());
        let entries = store.load(&session_id).await.unwrap();
        assert_eq!(entries.len(), 5);
        let msg_ids = message_entry_ids(&entries);
        let SessionEntry::Compaction {
            summary,
            first_kept_id,
            tokens_before,
            ..
        } = &entries[4]
        else {
            panic!("expected a Compaction entry");
        };
        assert_eq!(summary, "S");
        assert_eq!(*first_kept_id, msg_ids[2]);
        assert_eq!(*tokens_before, Some(3));

        // The next user arrival emits the compacted context via
        // build_context.
        send_file(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("e".to_string()).into(),
        )
        .await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].text(), "[Conversation summary]\nS");
        assert_eq!(messages[1].text(), "c");
        assert_eq!(messages[2].text(), "d");
        assert_eq!(messages[3].text(), "e");

        // A second compaction counts dropped from the first one's kept head:
        // context was [summary, c, d, e], dropped=1 skips "c", keeping "d".
        // Its record carries the first summary as its baseline.
        send_file(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            compaction_record_with_previous(Some("S2"), Some(1), None, Some("S")),
        )
        .await;

        let entries = store.load(&session_id).await.unwrap();
        let msg_ids = message_entry_ids(&entries);
        let Some(SessionEntry::Compaction {
            summary,
            first_kept_id,
            ..
        }) = entries.last()
        else {
            panic!("expected a Compaction entry");
        };
        assert_eq!(summary, "S2");
        assert_eq!(*first_kept_id, msg_ids[3]);

        send_file(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text(), "[Conversation summary]\nS2");
        assert_eq!(messages[1].text(), "d");
        assert_eq!(messages[2].text(), "e");
    }

    #[tokio::test]
    async fn messages_agent_compaction_dropped_beyond_history_discards_record() {
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) = setup_messages_agent(vec![]).await;

        let batch = AgentValue::array(vector![
            Message::user("a".to_string()).into(),
            Message::assistant("b".to_string()).into(),
        ]);
        send(&ma, &agent_id, PORT_MESSAGE, batch).await;
        assert_no_emit(&probe_rx).await;

        // dropped exceeds the history: the record cannot describe this
        // session (e.g. it raced a reset), so it is discarded — recording
        // it would inject a foreign summary.
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            compaction_record(Some("S"), Some(10), None),
        )
        .await;

        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("c".to_string()).into(),
        )
        .await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text(), "a");
        assert_eq!(messages[1].text(), "b");
        assert_eq!(messages[2].text(), "c");
    }

    #[tokio::test]
    async fn messages_agent_discards_oversized_first_compaction_record() {
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) = setup_messages_agent(vec![]).await;

        let batch = AgentValue::array(vector![
            Message::user("a".to_string()).into(),
            Message::assistant("b".to_string()).into(),
            Message::user("c".to_string()).into(),
        ]);
        send(&ma, &agent_id, PORT_MESSAGE, batch).await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 3);

        // A first compaction record (no baseline) sized for a ~200k-token
        // context cannot describe this tiny session: it was computed from
        // another session across a reset and must be discarded.
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            compaction_record(Some("S"), Some(1), Some(200_000)),
        )
        .await;

        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text(), "a");
        assert_eq!(messages[1].text(), "b");
        assert_eq!(messages[2].text(), "c");
    }

    #[tokio::test]
    async fn messages_agent_discards_stale_compaction_records() {
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) = setup_messages_agent(vec![]).await;

        let batch = AgentValue::array(vector![
            Message::user("a".to_string()).into(),
            Message::assistant("b".to_string()).into(),
            Message::user("c".to_string()).into(),
        ]);
        send(&ma, &agent_id, PORT_MESSAGE, batch).await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 3);

        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            compaction_record(Some("S"), Some(1), None),
        )
        .await;

        // A record computed before the compaction above (no baseline) and
        // one computed against a different summary are both stale.
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            compaction_record(Some("stale"), Some(1), None),
        )
        .await;
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            compaction_record_with_previous(Some("stale"), Some(1), None, Some("other")),
        )
        .await;

        // Only the first compaction took effect: [summary S, b, c].
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text(), "[Conversation summary]\nS");
        assert_eq!(messages[1].text(), "b");
        assert_eq!(messages[2].text(), "c");

        // A reset swaps in a fresh session; a record from the old
        // conversation (baseline = old summary) must not leak into it.
        send(&ma, &agent_id, PORT_RESET, AgentValue::unit()).await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 0);
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            compaction_record_with_previous(Some("S2"), Some(1), None, Some("S")),
        )
        .await;

        // The fresh session is empty, so a unit input emits nothing.
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        assert_no_emit(&probe_rx).await;
    }

    #[tokio::test]
    async fn messages_agent_compaction_resolves_pseudo_summary_head() {
        // A genuine first user message that happens to carry the summary
        // prefix: the compactor excludes it from `dropped`, and the walk
        // must skip the matching head entry to stay aligned.
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) = setup_messages_agent(vec![]).await;

        let batch = AgentValue::array(vector![
            Message::user("[Conversation summary]\nold".to_string()).into(),
            Message::user("m1".to_string()).into(),
            Message::assistant("m2".to_string()).into(),
            Message::user("m3".to_string()).into(),
        ]);
        send(&ma, &agent_id, PORT_MESSAGE, batch).await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 4);

        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            compaction_record_with_previous(Some("S"), Some(2), None, Some("old")),
        )
        .await;

        // dropped=2 skips m1 and m2 counted from *after* the pseudo-summary
        // head, so the kept tail starts at m3 — matching the compactor.
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text(), "[Conversation summary]\nS");
        assert_eq!(messages[1].text(), "m3");
    }

    #[tokio::test]
    async fn messages_agent_invalid_compaction_record_errors() {
        let (ma, _patch_id, agent_id, _probe_rx, _session_rx) = setup_messages_agent(vec![]).await;

        let agent = ma.get_agent(&agent_id).unwrap();
        let mut guard = agent.lock().await;
        let messages_agent = guard.as_agent_mut::<MessagesAgent>().unwrap();

        // Missing summary, missing dropped, and negative dropped all fail.
        for record in [
            compaction_record(None, Some(1), None),
            compaction_record(Some(""), Some(1), None),
            compaction_record(Some("S"), None, None),
            compaction_record(Some("S"), Some(-1), None),
        ] {
            let result = AsAgent::process(
                messages_agent,
                AgentContext::new(),
                PORT_MESSAGE.to_string(),
                record,
            )
            .await;
            assert!(matches!(result, Err(AgentError::InvalidValue(_))));
        }
    }

    #[tokio::test]
    async fn messages_agent_window_prunes_history_from_memory_store() {
        let (ma, patch_id, agent_id, probe_rx, _session_rx) =
            setup_messages_agent(vec![(CONFIG_MAX_CONTEXT_TOKENS, AgentValue::integer(15))]).await;

        // 10 tokens each: the pair plus the 2-token anchor breaks the budget.
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("x".repeat(40)).into(),
        )
        .await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 1);
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::assistant("y".repeat(40)).into(),
        )
        .await;
        assert_no_emit(&probe_rx).await;
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("zzzzzzzz".to_string()).into(),
        )
        .await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text(), "zzzzzzzz");

        // The old pair was deleted from the store too: a restart replays
        // only the surviving window.
        ma.stop_patch(&patch_id).await.unwrap();
        ma.start_patch(&patch_id).await.unwrap();
        wait_until_started(&ma, &agent_id).await;
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text(), "zzzzzzzz");
    }

    #[tokio::test]
    async fn messages_agent_max_messages_counts_system_message() {
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) =
            setup_messages_agent(vec![(CONFIG_MAX_MESSAGES, AgentValue::integer(3))]).await;

        let batch = AgentValue::array(vector![
            Message::system("sys".to_string()).into(),
            Message::user("u1".to_string()).into(),
            Message::assistant("a1".to_string()).into(),
            Message::user("u2".to_string()).into(),
        ]);
        send(&ma, &agent_id, PORT_MESSAGE, batch).await;

        // system + u2 = 2; adding the (u1, a1) pair would make 4 > 3.
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].text(), "u2");

        // The pair was pruned; the system message survives.
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
    }

    #[tokio::test]
    async fn messages_agent_trims_oversized_latest_user_and_stores_it() {
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) =
            setup_messages_agent(vec![(CONFIG_MAX_CONTEXT_TOKENS, AgentValue::integer(12))]).await;

        // The system message (10 tokens) leaves 2 tokens for the user: the
        // user text is middle-trimmed down to the floor, the system message
        // is left whole.
        let batch = AgentValue::array(vector![
            Message::system("s".repeat(40)).into(),
            Message::user(format!("{}{}", "h".repeat(200), "t".repeat(200))).into(),
        ]);
        send(&ma, &agent_id, PORT_MESSAGE, batch).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text().len(), 40);
        let user_text = messages[1].text();
        assert!(user_text.contains(TRIM_MARKER));
        assert!(user_text.starts_with('h'));
        assert!(user_text.ends_with('t'));
        assert!(user_text.len() < 100);

        // The trimmed form is what was stored.
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages[1].text(), user_text);
    }

    #[tokio::test]
    async fn messages_agent_caps_each_arriving_message_except_system() {
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) =
            setup_messages_agent(vec![(CONFIG_MAX_MESSAGE_TOKENS, AgentValue::integer(10))]).await;

        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("q".to_string()).into(),
        )
        .await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 1);
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::assistant("a".repeat(400)).into(),
        )
        .await;
        assert_no_emit(&probe_rx).await;

        // The oversized assistant message was stored middle-trimmed; a
        // system message is never capped.
        let batch = AgentValue::array(vector![
            Message::system("s".repeat(400)).into(),
            Message::user("u2".to_string()).into(),
        ]);
        send(&ma, &agent_id, PORT_MESSAGE, batch).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].text().len(), 400);
        assert_eq!(messages[1].text(), "q");
        let assistant_text = messages[2].text();
        assert!(assistant_text.contains(TRIM_MARKER));
        assert!(assistant_text.len() < 100);
        assert_eq!(messages[3].text(), "u2");
    }

    #[tokio::test]
    async fn messages_agent_keeps_only_last_system_and_prunes_older_ones() {
        let (ma, patch_id, agent_id, probe_rx, _session_rx) =
            setup_messages_agent(vec![(CONFIG_MAX_CONTEXT_TOKENS, AgentValue::integer(1000))])
                .await;

        let batch = AgentValue::array(vector![
            Message::system("one".to_string()).into(),
            Message::user("u1".to_string()).into(),
        ]);
        send(&ma, &agent_id, PORT_MESSAGE, batch).await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 2);

        let batch = AgentValue::array(vector![
            Message::system("two".to_string()).into(),
            Message::user("u2".to_string()).into(),
        ]);
        send(&ma, &agent_id, PORT_MESSAGE, batch).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text(), "two");
        assert_eq!(messages[1].text(), "u1");
        assert_eq!(messages[2].text(), "u2");

        // The superseded system message was pruned from the store.
        ma.stop_patch(&patch_id).await.unwrap();
        ma.start_patch(&patch_id).await.unwrap();
        wait_until_started(&ma, &agent_id).await;
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text(), "two");
    }

    #[tokio::test]
    async fn messages_agent_discards_baseline_less_record_after_prune() {
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) =
            setup_messages_agent(vec![(CONFIG_MAX_CONTEXT_TOKENS, AgentValue::integer(15))]).await;

        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("x".repeat(40)).into(),
        )
        .await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 1);
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::assistant("y".repeat(40)).into(),
        )
        .await;
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("u2".to_string()).into(),
        )
        .await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 1);

        // The prune moved the log's head; a baseline-less record anchored
        // to the old head can no longer be resolved and is discarded.
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            compaction_record(Some("S"), Some(0), None),
        )
        .await;

        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("u3".to_string()).into(),
        )
        .await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text(), "u2");
        assert_eq!(messages[1].text(), "u3");
    }

    #[tokio::test]
    async fn messages_agent_prunes_hidden_history_but_keeps_live_marker() {
        let (ma, patch_id, agent_id, probe_rx, _session_rx) =
            setup_messages_agent(vec![(CONFIG_MAX_CONTEXT_TOKENS, AgentValue::integer(1000))])
                .await;

        let batch = AgentValue::array(vector![
            Message::user("a".to_string()).into(),
            Message::assistant("b".to_string()).into(),
            Message::user("c".to_string()).into(),
        ]);
        send(&ma, &agent_id, PORT_MESSAGE, batch).await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 3);
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            compaction_record(Some("S"), Some(1), None),
        )
        .await;

        // The summary stays in the window, so the marker survives while
        // the history it hides ("a") is pruned.
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("d".to_string()).into(),
        )
        .await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].text(), "[Conversation summary]\nS");

        ma.stop_patch(&patch_id).await.unwrap();
        ma.start_patch(&patch_id).await.unwrap();
        wait_until_started(&ma, &agent_id).await;
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].text(), "[Conversation summary]\nS");
        assert_eq!(messages[1].text(), "b");
        assert_eq!(messages[2].text(), "c");
        assert_eq!(messages[3].text(), "d");
    }

    #[tokio::test]
    async fn messages_agent_drops_marker_when_summary_leaves_window() {
        let (ma, patch_id, agent_id, probe_rx, _session_rx) =
            setup_messages_agent(vec![(CONFIG_MAX_CONTEXT_TOKENS, AgentValue::integer(15))]).await;

        let batch = AgentValue::array(vector![
            Message::user("a".to_string()).into(),
            Message::assistant("b".to_string()).into(),
            Message::user("c".to_string()).into(),
        ]);
        send(&ma, &agent_id, PORT_MESSAGE, batch).await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 3);
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            compaction_record(Some("S"), Some(1), None),
        )
        .await;

        // The 10-token user pushes the summary (and "b") out of the window;
        // the marker is deleted along with the history it covered.
        send(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("d".repeat(40)).into(),
        )
        .await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text(), "c");
        assert_eq!(messages[1].text(), "d".repeat(40));

        ma.stop_patch(&patch_id).await.unwrap();
        ma.start_patch(&patch_id).await.unwrap();
        wait_until_started(&ma, &agent_id).await;
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text(), "c");
    }

    #[tokio::test]
    async fn file_messages_agent_prune_file_controls_file_deletion() {
        for (prune_file, expected_file_entries) in [(true, 1usize), (false, 3usize)] {
            let dir = tempfile::tempdir().unwrap();
            let (ma, _patch_id, agent_id, probe_rx, _session_rx) = setup_file_messages_agent(vec![
                (
                    CONFIG_SESSION_DIR,
                    AgentValue::string(dir.path().to_string_lossy()),
                ),
                (CONFIG_MAX_CONTEXT_TOKENS, AgentValue::integer(15)),
                (CONFIG_PRUNE_FILE, AgentValue::boolean(prune_file)),
            ])
            .await;

            send_file(
                &ma,
                &agent_id,
                PORT_MESSAGE,
                Message::user("x".repeat(40)).into(),
            )
            .await;
            assert_eq!(recv_messages(&probe_rx).await.len(), 1);
            send_file(
                &ma,
                &agent_id,
                PORT_MESSAGE,
                Message::assistant("y".repeat(40)).into(),
            )
            .await;
            send_file(
                &ma,
                &agent_id,
                PORT_MESSAGE,
                Message::user("u2".to_string()).into(),
            )
            .await;
            assert_eq!(recv_messages(&probe_rx).await.len(), 1);

            let session_id = session_id_config(&ma, &agent_id).await;
            let store = JsonlSessionStore::new(dir.path());
            assert_eq!(
                store.load(&session_id).await.unwrap().len(),
                expected_file_entries,
                "prune_file = {prune_file}"
            );
        }
    }

    fn plain(role: &str, text: &str) -> Message {
        match role {
            "user" => Message::user(text.to_string()),
            "assistant" => Message::assistant(text.to_string()),
            "system" => Message::system(text.to_string()),
            "tool" => Message::tool("t".to_string(), text.to_string()),
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_select_window_requires_user_or_tool_tail() {
        assert!(select_window(&[], 0, 0).is_none());
        assert!(select_window(&[plain("user", "u"), plain("assistant", "a")], 0, 0).is_none());
        // A tool tail with no user anywhere has no window head.
        assert!(select_window(&[plain("assistant", "a"), plain("tool", "t")], 0, 0).is_none());
    }

    #[test]
    fn test_select_window_adds_pairs_until_budget() {
        let context = vec![
            plain("user", &"1".repeat(40)),
            plain("assistant", &"2".repeat(40)),
            plain("user", &"3".repeat(40)),
            plain("assistant", &"4".repeat(40)),
            plain("user", "55555555"),
        ];
        // anchor 2 + newest pair 20 = 22 fits 25; the next pair would not.
        let window = select_window(&context, 25, 0).unwrap();
        assert_eq!(window.cut, 2);
        assert_eq!(window.pinned_system, None);

        // No limits: everything is kept.
        assert_eq!(select_window(&context, 0, 0).unwrap().cut, 0);
    }

    #[test]
    fn test_select_window_minimal_window_ignores_budgets() {
        let context = vec![
            plain("system", &"s".repeat(40)),
            plain("user", &"u".repeat(40)),
        ];
        let window = select_window(&context, 5, 1).unwrap();
        assert_eq!(window.cut, 1);
        assert_eq!(window.pinned_system, Some(0));
    }

    #[test]
    fn test_select_window_counts_messages_including_system() {
        let context = vec![
            plain("system", "s"),
            plain("user", "u1"),
            plain("assistant", "a1"),
            plain("user", "u2"),
        ];
        assert_eq!(select_window(&context, 0, 4).unwrap().cut, 1);
        assert_eq!(select_window(&context, 0, 3).unwrap().cut, 3);
    }

    #[test]
    fn test_select_window_keeps_tool_exchange_atomic() {
        let context = vec![
            plain("user", &"1".repeat(40)),
            plain("assistant", &"2".repeat(40)),
            plain("user", &"3".repeat(40)),
            plain("assistant", &"4".repeat(40)),
            plain("tool", &"5".repeat(40)),
            plain("user", "66666666"),
        ];
        // The (user, assistant, tool) group is added as one unit: 2 + 30
        // fits 35, the next pair would not.
        let window = select_window(&context, 35, 0).unwrap();
        assert_eq!(window.cut, 2);

        // A tool tail's minimal window runs from the latest user.
        let window = select_window(&context[..5], 1, 0).unwrap();
        assert_eq!(window.cut, 2);
    }

    #[test]
    fn test_select_window_pins_last_system_and_excludes_orphans() {
        let context = vec![
            plain("system", "s1"),
            plain("user", "u1"),
            plain("system", "s2"),
            plain("user", "u2"),
        ];
        let window = select_window(&context, 0, 0).unwrap();
        assert_eq!(window.cut, 1);
        assert_eq!(window.pinned_system, Some(2));

        // A leading assistant can never head the window.
        let context = vec![
            plain("assistant", "orphan"),
            plain("user", "u1"),
            plain("assistant", "a1"),
            plain("user", "u2"),
        ];
        assert_eq!(select_window(&context, 0, 0).unwrap().cut, 1);

        // Consecutive users each form their own group.
        let context = vec![
            plain("user", "u1"),
            plain("user", "u2"),
            plain("user", "u3"),
        ];
        assert_eq!(select_window(&context, 0, 0).unwrap().cut, 0);
    }

    #[test]
    fn test_middle_trim_replaces_middle_with_marker() {
        let message = Message::user(format!("{}{}", "h".repeat(200), "t".repeat(200)));
        let trimmed = middle_trim(&message, 20).expect("should trim");
        let text = trimmed.text();
        assert!(text.contains(TRIM_MARKER));
        assert!(text.starts_with('h'));
        assert!(text.ends_with('t'));
        assert!(text.len() <= 80);
        assert!(estimate_message_tokens(&trimmed) <= 20);

        // A message within budget is left alone.
        assert!(middle_trim(&Message::user("short".to_string()), 20).is_none());
    }

    #[test]
    fn test_middle_trim_floor_keeps_a_stub() {
        let message = Message::user("a".repeat(100));
        let trimmed = middle_trim(&message, 0).expect("should trim");
        let text = trimmed.text();
        assert_eq!(text.len(), 2 * TRIM_FLOOR_BYTES + TRIM_MARKER.len());
        assert!(text.contains(TRIM_MARKER));
    }

    #[test]
    fn test_middle_trim_cuts_on_char_boundaries() {
        let message = Message::user("あ".repeat(200));
        let trimmed = middle_trim(&message, 20).expect("should trim");
        let text = trimmed.text();
        assert!(text.contains(TRIM_MARKER));
        assert!(text.starts_with('あ'));
        assert!(text.ends_with('あ'));
    }

    #[test]
    fn test_middle_trim_blocks_keeps_non_text_blocks() {
        let mut message = Message::user(String::new());
        message.content = MessageContent::Blocks(vec![
            ContentBlock::Image {
                data: "A".repeat(100),
                mime_type: "image/png".to_string(),
            },
            ContentBlock::Text {
                text: "b".repeat(400),
            },
        ]);
        let trimmed = middle_trim(&message, 10).expect("should trim");
        let MessageContent::Blocks(blocks) = &trimmed.content else {
            panic!("expected blocks");
        };
        assert_eq!(blocks.len(), 2);
        assert!(matches!(&blocks[0], ContentBlock::Image { data, .. } if data.len() == 100));
        let ContentBlock::Text { text } = &blocks[1] else {
            panic!("expected a text block");
        };
        assert!(text.contains(TRIM_MARKER));
        assert!(text.len() < 100);
    }

    #[test]
    fn test_add_message() {
        // () + user
        // result should be the user message
        let value = AgentValue::unit();
        let msg = Message::user("Hello".to_string());
        let result = append_message(value, msg);
        assert!(result.is_message());
        let result_msg = result.as_message().unwrap();
        assert_eq!(result_msg.role, "user");
        assert_eq!(result_msg.text(), "Hello");

        // string + assistant
        // result should be an array with user and assistant messages
        let value = AgentValue::string("How are you?");
        let msg = Message::assistant("Hello".to_string());
        let result = append_message(value, msg);
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let msg0 = &arr[0].as_message().unwrap();
        assert_eq!(msg0.role, "user");
        assert_eq!(msg0.text(), "How are you?");
        let msg1 = &arr[1].as_message().unwrap();
        assert_eq!(msg1.role, "assistant");
        assert_eq!(msg1.text(), "Hello");

        // object + user
        // result should be an array with the original object and the new user message
        let value = AgentValue::object(hashmap! {
            "role".into() => AgentValue::string("system"),
            "content".into() => AgentValue::string("I am fine."),
        });
        let msg = Message::user("Hello".to_string());
        let result = append_message(value, msg);
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let msg0 = &arr[0].as_message().unwrap();
        assert_eq!(msg0.role, "system");
        assert_eq!(msg0.text(), "I am fine.");
        let msg1 = &arr[1].as_message().unwrap();
        assert_eq!(msg1.role, "user");
        assert_eq!(msg1.text(), "Hello");

        // array + user
        // result should be the original array with the new user message appended
        let value = AgentValue::array(vector![
            AgentValue::object(hashmap! {
                "role".into() => AgentValue::string("system"),
                "content".into() => AgentValue::string("Welcome!"),
            }),
            AgentValue::object(hashmap! {
                "role".into() => AgentValue::string("assistant"),
                "content".into() => AgentValue::string("Hello!"),
            }),
        ]);
        let msg = Message::user("How are you?".to_string());
        let result = append_message(value, msg);
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        let msg0 = &arr[0].as_message().unwrap();
        assert_eq!(msg0.role, "system");
        assert_eq!(msg0.text(), "Welcome!");
        let msg1 = &arr[1].as_message().unwrap();
        assert_eq!(msg1.role, "assistant");
        assert_eq!(msg1.text(), "Hello!");
        let msg2 = &arr[2].as_message().unwrap();
        assert_eq!(msg2.role, "user");
        assert_eq!(msg2.text(), "How are you?");

        // image + user
        #[cfg(feature = "image")]
        let img = AgentValue::image(modular_agent_core::PhotonImage::new(vec![0u8; 4], 1, 1));
        {
            let msg = Message::user("Check this image".to_string());
            let result = append_message(img, msg);
            assert!(result.is_array());
            let arr = result.as_array().unwrap();
            assert_eq!(arr.len(), 1);
            let msg0 = &arr[0].as_message().unwrap();
            assert_eq!(msg0.role, "user");
            assert_eq!(msg0.text(), "Check this image");
            assert!(msg0.image.is_some());
        }
    }

    #[test]
    fn test_strip_unsigned_thinking_flattens_to_text() {
        let mut msg = Message::assistant(String::new());
        msg.content = MessageContent::Blocks(vec![
            ContentBlock::Thinking {
                thinking: "unsigned trace".to_string(),
                signature: None,
                redacted: false,
            },
            ContentBlock::Text {
                text: "answer".to_string(),
            },
        ]);
        let stripped = strip_unsigned_thinking(&msg).expect("should strip");
        assert_eq!(stripped.content, MessageContent::Text("answer".to_string()));
    }

    #[test]
    fn test_strip_unsigned_thinking_keeps_signed_and_redacted_blocks() {
        // Signed / redacted thinking must survive trimming: Claude replays
        // them on extended-thinking + tool-use continuations.
        let signed = ContentBlock::Thinking {
            thinking: "signed trace".to_string(),
            signature: Some("sig123".to_string()),
            redacted: false,
        };
        let redacted = ContentBlock::Thinking {
            thinking: "ciphertext".to_string(),
            signature: None,
            redacted: true,
        };
        let text = ContentBlock::Text {
            text: "answer".to_string(),
        };

        let mut msg = Message::assistant(String::new());
        msg.content = MessageContent::Blocks(vec![
            signed.clone(),
            ContentBlock::Thinking {
                thinking: "unsigned trace".to_string(),
                signature: None,
                redacted: false,
            },
            redacted.clone(),
            text.clone(),
        ]);
        let stripped = strip_unsigned_thinking(&msg).expect("should strip");
        assert_eq!(
            stripped.content,
            MessageContent::Blocks(vec![signed, redacted, text])
        );
    }

    #[test]
    fn test_strip_unsigned_thinking_none_when_nothing_to_strip() {
        // Plain text and fully signed content pass through untouched so the
        // caller keeps the original AgentValue.
        let msg = Message::assistant("plain".to_string());
        assert!(strip_unsigned_thinking(&msg).is_none());

        let mut msg = Message::assistant(String::new());
        msg.content = MessageContent::Blocks(vec![
            ContentBlock::Thinking {
                thinking: "signed".to_string(),
                signature: Some("sig".to_string()),
                redacted: false,
            },
            ContentBlock::Text {
                text: "answer".to_string(),
            },
        ]);
        assert!(strip_unsigned_thinking(&msg).is_none());
    }

    /// Build a running patch with a MessagesForPromptAgent (configured via
    /// `configs`) whose `messages` port feeds a probe.
    async fn setup_prompt_agent(
        configs: Vec<(&str, AgentValue)>,
    ) -> (ModularAgent, String, ProbeReceiver) {
        let ma = ModularAgent::init().unwrap();
        ma.ready().await.unwrap();

        let patch_id = ma.new_patch().unwrap();
        let def = ma
            .get_agent_definition(MessagesForPromptAgent::DEF_NAME)
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
        let probe_id = ma
            .add_agent(patch_id.clone(), probe_def.to_spec())
            .await
            .unwrap();
        ma.add_connection(
            &patch_id,
            ConnectionSpec {
                source: agent_id.clone(),
                source_handle: PORT_MESSAGES.into(),
                target: probe_id.clone(),
                target_handle: "value".into(),
            },
        )
        .await
        .unwrap();

        ma.start_patch(&patch_id).await.unwrap();
        wait_until_started(&ma, &agent_id).await;
        let probe_rx = probe_receiver(&ma, &probe_id).await.unwrap();

        (ma, agent_id, probe_rx)
    }

    async fn send_to_prompt_agent(ma: &ModularAgent, agent_id: &str, value: AgentValue) {
        let agent = ma.get_agent(agent_id).unwrap();
        let mut guard = agent.lock().await;
        let prompt_agent = guard.as_agent_mut::<MessagesForPromptAgent>().unwrap();
        AsAgent::process(
            prompt_agent,
            AgentContext::new(),
            PORT_MESSAGES.to_string(),
            value,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn messages_for_prompt_max_tokens_keeps_newest_and_system() {
        let (ma, agent_id, probe_rx) =
            setup_prompt_agent(vec![(CONFIG_MAX_TOKENS, AgentValue::integer(10))]).await;

        // system: 4 chars = 1 token; old pair: 20 tokens each; recent
        // user: 2 tokens. Budget 10 fits system + recent user only.
        let input = AgentValue::array(vector![
            Message::system("sys.".to_string()).into(),
            Message::user("x".repeat(80)).into(),
            Message::assistant("y".repeat(80)).into(),
            Message::user("recent q".to_string()).into(),
        ]);
        send_to_prompt_agent(&ma, &agent_id, input).await;

        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].text(), "sys.");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[1].text(), "recent q");
    }

    #[tokio::test]
    async fn messages_for_prompt_max_tokens_first_message_is_user() {
        let (ma, agent_id, probe_rx) =
            setup_prompt_agent(vec![(CONFIG_MAX_TOKENS, AgentValue::integer(5))]).await;

        // The oldest user message (10 tokens) breaks the budget, leaving
        // the selection headed by an assistant message that must be popped.
        let input = AgentValue::array(vector![
            Message::user("a".repeat(40)).into(),
            Message::assistant("bbbb".to_string()).into(),
            Message::user("cccc".to_string()).into(),
        ]);
        send_to_prompt_agent(&ma, &agent_id, input).await;

        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].text(), "cccc");
    }

    #[tokio::test]
    async fn messages_for_prompt_max_tokens_takes_precedence_over_max_size() {
        // max_size alone would trim everything; the generous token budget
        // must win and keep the whole history.
        let (ma, agent_id, probe_rx) = setup_prompt_agent(vec![
            (CONFIG_MAX_SIZE, AgentValue::integer(1)),
            (CONFIG_MAX_TOKENS, AgentValue::integer(1000)),
        ])
        .await;

        let input = AgentValue::array(vector![
            Message::user("hello".to_string()).into(),
            Message::assistant("world".to_string()).into(),
            Message::user("again".to_string()).into(),
        ]);
        send_to_prompt_agent(&ma, &agent_id, input).await;

        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text(), "hello");
        assert_eq!(messages[1].text(), "world");
        assert_eq!(messages[2].text(), "again");
    }

    #[tokio::test]
    async fn messages_for_prompt_max_size_counts_image_blocks() {
        // An image tool result has no text, but its base64 payload is sent
        // to the provider, so it must count toward the byte budget.
        let (ma, agent_id, probe_rx) =
            setup_prompt_agent(vec![(CONFIG_MAX_SIZE, AgentValue::integer(100))]).await;

        let image_result = Message::tool_with_content(
            "screenshot".to_string(),
            MessageContent::Blocks(vec![ContentBlock::Image {
                data: "A".repeat(500),
                mime_type: "image/png".to_string(),
            }]),
        );
        let input = AgentValue::array(vector![
            Message::user("old question".to_string()).into(),
            image_result.into(),
            Message::user("recent q".to_string()).into(),
        ]);
        send_to_prompt_agent(&ma, &agent_id, input).await;

        // The 500-byte image breaks the 100-byte budget, so only the newest
        // user message survives.
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].text(), "recent q");
    }
}
