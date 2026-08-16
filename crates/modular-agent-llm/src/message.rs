use std::sync::Arc;

use im::{Vector, vector};
use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    ContentBlock, InMemorySessionStore, JsonlSessionStore, Message, MessageContent, ModularAgent,
    SessionEntry, SessionMeta, SessionStore, async_trait, build_context, estimate_context_tokens,
    estimate_message_tokens, modular_agent,
};

const CATEGORY: &str = "LLM/Message";

const PORT_MESSAGE: &str = "message";
const PORT_MESSAGES: &str = "messages";
const PORT_RESET: &str = "reset";
const PORT_SESSION_ID: &str = "session_id";

const CONFIG_MAX_SIZE: &str = "max_size";
const CONFIG_MAX_TOKENS: &str = "max_tokens";
const CONFIG_MESSAGE: &str = "message";
const CONFIG_PREAMBLE: &str = "preamble";
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

    /// Latest partial streaming message; never appended to the store.
    partial: Option<Message>,
}

impl SessionState {
    fn session_id(&self) -> Result<&str, AgentError> {
        self.session_id
            .as_deref()
            .ok_or_else(|| AgentError::Other("Session is not initialized".to_string()))
    }

    /// The emitted context: the stored entries passed through
    /// [`build_context`], with the current partial message (if any) last.
    fn context_value(&self) -> AgentValue {
        let mut messages: Vector<AgentValue> = build_context(&self.entries)
            .into_iter()
            .map(AgentValue::from)
            .collect();
        if let Some(partial) = &self.partial {
            messages.push_back(partial.clone().into());
        }
        AgentValue::array(messages)
    }
}

/// Store and state access shared by [`process_session_input`] across the
/// Messages agents; each agent keeps its own store kind (in-memory vs JSONL
/// files).
trait SessionMessages: AsAgent {
    fn store(&self) -> Result<Arc<dyn SessionStore>, AgentError>;

    fn session_state_mut(&mut self) -> &mut SessionState;
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
/// the store; a partial streaming message replaces the previous partial in
/// the single in-memory slot, and the slot is cleared when the final message
/// with the same id arrives.
async fn append_messages(
    store: &Arc<dyn SessionStore>,
    state: &mut SessionState,
    in_messages: &Vector<AgentValue>,
) -> Result<(), AgentError> {
    let session_id = state.session_id()?.to_string();
    for value in in_messages {
        let message = value.as_message().ok_or_else(|| {
            AgentError::InvalidValue("Input contains non-Message values".to_string())
        })?;

        if message.streaming {
            state.partial = Some(message.clone());
            continue;
        }

        if let Some(partial) = &state.partial
            && partial.id.is_some()
            && partial.id == message.id
        {
            state.partial = None;
        }

        let entry = SessionEntry::message(message.clone());
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
/// session, a unit input re-emits the current context, a compaction record
/// appends a marker, and anything else is appended as messages.
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
        state.partial = None;
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
        let messages = agent.session_state_mut().context_value();
        agent.output(ctx, PORT_MESSAGES, messages).await?;
        return Ok(());
    }

    // Dispatch by shape, before message conversion: Message objects carry
    // role/content and never a "type" key, so this cannot collide.
    if value.get_str("type") == Some("compaction") {
        let store = agent.store()?;
        return record_compaction(&store, agent.session_state_mut(), &value).await;
    }

    let Some(in_messages) = to_message_batch(value)? else {
        return Ok(());
    };

    let store = agent.store()?;
    append_messages(&store, agent.session_state_mut(), &in_messages).await?;

    let messages = agent.session_state_mut().context_value();
    agent.output(ctx, PORT_MESSAGES, messages).await?;
    Ok(())
}

/// Accumulate messages in an in-memory session store.
///
/// Received messages are appended to a session (an append-only conversation
/// log) and the full conversation context is emitted after every input. The
/// history lives in memory only: it is retained across agent stop/start
/// within the same process and lost when the process exits. To persist
/// sessions as files that survive restarts, use the File Messages agent
/// instead. The history is never trimmed here — limiting the context size
/// is the job of downstream agents such as `MessagesForPromptAgent`.
///
/// Only finalized messages (`streaming == false`) reach the store. Partial
/// streaming messages are held in a single in-memory slot — each partial
/// replaces the previous one, and the slot is cleared when the final message
/// with the same id arrives — and appear only at the end of the emitted
/// context, never in the store.
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
/// baseline to compare) — is discarded with a warning instead of being
/// recorded: the summarization call behind a record takes seconds, and a
/// `reset` or a second compaction in that window makes the record stale.
///
/// Patches saved before session support carried the history in a hidden
/// `messages` config. On the first start that history is imported once into
/// the session store (only if the session has no messages yet); the stale
/// config key is dropped afterwards.
///
/// # Ports
/// - Input `message`: Message or array of messages to append. A unit value
///   emits the current context without appending. A compaction record
///   (object with `type` `"compaction"`, a non-empty `summary`, a
///   non-negative integer `dropped`, an optional `tokens_before`, and an
///   optional `previous_summary` baseline) appends a compaction marker to
///   the session and emits nothing; a record with a stale baseline is
///   discarded
/// - Input `reset`: Start a new session and emit an empty array
/// - Output `messages`: Conversation context as an array of messages
/// - Output `session_id`: The freshly issued session id, emitted when
///   `reset` switches to a new session
///
/// # Configuration
/// - `session_id`: Session to resume on start. Empty: a new session is
///   created and its id is written back to this config (default: "")
#[modular_agent(
    title="Messages",
    category=CATEGORY,
    inputs=[PORT_MESSAGE, PORT_RESET],
    outputs=[PORT_MESSAGES, PORT_SESSION_ID],
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
        self.state.partial = None;

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
/// Received messages are appended to a session (an append-only conversation
/// log) persisted as `<session_dir>/<session_id>.jsonl`, and the full
/// conversation context is emitted after every input. Sessions survive
/// restarts; to keep the history in memory only, use the Messages agent
/// instead. The history is never trimmed here — limiting the context size
/// is the job of downstream agents such as `MessagesForPromptAgent`.
///
/// Only finalized messages (`streaming == false`) reach the store. Partial
/// streaming messages are held in a single in-memory slot — each partial
/// replaces the previous one, and the slot is cleared when the final message
/// with the same id arrives — and appear only at the end of the emitted
/// context, never in the store.
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
/// baseline to compare) — is discarded with a warning instead of being
/// recorded: the summarization call behind a record takes seconds, and a
/// `reset` or a second compaction in that window makes the record stale.
///
/// `session_dir` is applied when the agent starts; the agent fails to start
/// while it is empty. Changing it while the agent is running makes further
/// inputs fail with a config error until the agent is restarted.
///
/// # Ports
/// - Input `message`: Message or array of messages to append. A unit value
///   emits the current context without appending. A compaction record
///   (object with `type` `"compaction"`, a non-empty `summary`, a
///   non-negative integer `dropped`, an optional `tokens_before`, and an
///   optional `previous_summary` baseline) appends a compaction marker to
///   the session and emits nothing; a record with a stale baseline is
///   discarded
/// - Input `reset`: Start a new session and emit an empty array
/// - Output `messages`: Conversation context as an array of messages
/// - Output `session_id`: The freshly issued session id, emitted when
///   `reset` switches to a new session
///
/// # Configuration
/// - `session_dir`: Directory for the JSONL session files. Required; the
///   agent fails to start while it is empty (default: "")
/// - `session_id`: Session to resume on start. Empty: a new session is
///   created and its id is written back to this config (default: "")
#[modular_agent(
    title="File Messages",
    category=CATEGORY,
    inputs=[PORT_MESSAGE, PORT_RESET],
    outputs=[PORT_MESSAGES, PORT_SESSION_ID],
    string_config(name=CONFIG_SESSION_DIR, default=""),
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
        self.state.partial = None;
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

    #[tokio::test]
    async fn file_messages_agent_persists_only_finalized_messages() {
        let dir = tempfile::tempdir().unwrap();
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) = setup_file_messages_agent(vec![(
            CONFIG_SESSION_DIR,
            AgentValue::string(dir.path().to_string_lossy()),
        )])
        .await;

        let mut partial = Message::assistant("Hel".to_string());
        partial.id = Some("m1".to_string());
        partial.streaming = true;
        send_file(&ma, &agent_id, PORT_MESSAGE, partial.into()).await;

        // The partial appears in the emitted context...
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 1);
        assert!(messages[0].streaming);
        assert_eq!(messages[0].text(), "Hel");

        let mut fin = Message::assistant("Hello".to_string());
        fin.id = Some("m1".to_string());
        send_file(&ma, &agent_id, PORT_MESSAGE, fin.into()).await;

        // ...and the final with the same id replaces it: exactly one copy.
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 1);
        assert!(!messages[0].streaming);
        assert_eq!(messages[0].text(), "Hello");

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

        // start() replayed the history: a unit input emits it as-is.
        send_file(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text(), "a");
        assert_eq!(messages[1].text(), "b");

        // The next appended message extends the same session.
        send_file(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("c".to_string()).into(),
        )
        .await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 3);
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

        // The old history landed in the (in-memory) store.
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text(), "old user");
        assert_eq!(messages[1].text(), "old assistant");

        // A stop()/start() cycle must not import again; the in-memory store
        // is retained across the cycle.
        ma.stop_patch(&patch_id).await.unwrap();
        ma.start_patch(&patch_id).await.unwrap();
        wait_until_started(&ma, &agent_id).await;

        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text(), "old user");
        assert_eq!(messages[1].text(), "old assistant");
    }

    #[tokio::test]
    async fn messages_agent_unit_outputs_context_and_array_appends_in_order() {
        let (ma, _patch_id, agent_id, probe_rx, _session_rx) = setup_messages_agent(vec![]).await;

        // Unit input on an empty session emits an empty array.
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 0);

        // An array input appends multiple messages in order.
        let batch = AgentValue::array(vector![
            Message::user("a".to_string()).into(),
            Message::assistant("b".to_string()).into(),
        ]);
        send(&ma, &agent_id, PORT_MESSAGE, batch).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text(), "a");
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].text(), "b");
        assert_eq!(messages[1].role, "assistant");

        // Unit input re-emits the current context unchanged.
        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text(), "a");
        assert_eq!(messages[1].text(), "b");
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
        assert_eq!(recv_messages(&probe_rx).await.len(), 4);

        // The record appends a marker and emits nothing (an emit here would
        // re-trigger a downstream ChatAgent with the compacted context).
        send_file(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            compaction_record(Some("S"), Some(2), Some(3)),
        )
        .await;
        assert!(
            probe_rx
                .recv_with_timeout(std::time::Duration::from_millis(100))
                .await
                .is_err()
        );

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

        // The next input emits the compacted context via build_context.
        send_file(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].text(), "[Conversation summary]\nS");
        assert_eq!(messages[1].text(), "c");
        assert_eq!(messages[2].text(), "d");

        // A second compaction counts dropped from the first one's kept head:
        // context was [summary, c, d, e], dropped=1 skips "c", keeping "d".
        // Its record carries the first summary as its baseline.
        send_file(
            &ma,
            &agent_id,
            PORT_MESSAGE,
            Message::user("e".to_string()).into(),
        )
        .await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 4);
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
        assert_eq!(recv_messages(&probe_rx).await.len(), 2);

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

        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        let messages = recv_messages(&probe_rx).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text(), "a");
        assert_eq!(messages[1].text(), "b");
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

        send(&ma, &agent_id, PORT_MESSAGE, AgentValue::unit()).await;
        assert_eq!(recv_messages(&probe_rx).await.len(), 0);
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
