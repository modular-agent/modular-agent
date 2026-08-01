//! Built-in MCP server exposing flow-editing tools over streamable HTTP.
//!
//! External agents (e.g. Claude Code) connect to `http://127.0.0.1:<port>/mcp`
//! and use fine-grained tools (`add_agent`, `add_connection`, `start_preset`,
//! ...) to build and run workflows. All tools are thin wrappers over
//! [`ModularAgent`] methods plus pre-validation that turns mistakes (wrong
//! definition names, wrong port names) into self-correctable error messages.
//!
//! The server binds to 127.0.0.1 only. When [`McpServerConfig::token`] is
//! set, every request must carry an `Authorization: Bearer <token>` header;
//! without a token the server is unauthenticated and should stay opt-in on
//! the host side.
//!
//! # Example
//!
//! ```rust,no_run
//! use modular_agent_core::ModularAgent;
//! use modular_agent_core::mcp_server::{McpServerConfig, start_mcp_server};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let ma = ModularAgent::init()?;
//! ma.ready().await?;
//! let handle = start_mcp_server(
//!     ma,
//!     McpServerConfig {
//!         port: 8765,
//!         presets_dir: Some("/path/to/presets".into()),
//!         token: Some("secret".into()),
//!     },
//! )
//! .await?;
//! // ... later ...
//! handle.stop().await;
//! # Ok(())
//! # }
//! ```

use std::collections::VecDeque;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use axum::{
    extract::Request,
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::config::AgentConfigs;
use crate::definition::AgentDefinition;
use crate::error::AgentError;
use crate::modular_agent::{ModularAgent, ModularAgentEvent};
use crate::spec::{AgentSpec, ConnectionSpec, PresetSpec};
use crate::value::AgentValue;

const SERVER_INSTRUCTIONS: &str = r#"Modular Agent flow editor.

FLOW MODEL
- A preset is a workflow graph: agents (nodes) connected by directed edges.
- Each agent is an instance of an agent definition, identified by def_name
  (a fully-qualified Rust path, e.g. "modular_agent_llm::chat::ChatAgent").
- A connection routes values from a source agent's output port to a target
  agent's input port. Port names must exactly match the definition's ports.
- Special ports:
  - Every agent has an implicit "err" output port emitting error messages.
  - A target_handle of the form "config:<key>" writes incoming values into
    the target agent's config <key> instead of a regular input port.

LAYOUT CONVENTION
- Node position and size are stored as x, y, width, height fields on the
  agent spec (pixels). The editor grid unit is 240: place nodes at multiples
  of 240 and lay flows out left to right (x grows in the data-flow
  direction). When width/height are omitted, the editor sizes the node from
  the agent definition's hints (grid-unit multipliers, default 1x1 grid
  unit), so omitting them is usually the right choice. Definitions with a
  free_size hint do not follow the 240 grid; their hint sizes are pixels.
- add_agent accepts x/y/width/height directly; update_agent_spec changes
  them later, e.g. patch {"x": 480, "y": 240}.

DYNAMIC CONFIGS AND PORTS
- Some agents generate additional configs and ports from a config value
  (e.g. Switch's "n" controls the conditions c0..c(n-1) and the numbered
  output ports). The definition only lists the initial ones; the keys valid
  right now are the "configs" of the live agent spec, as returned by
  add_agent and get_preset_spec.

SECRETS AND GLOBAL CONFIGS
- API keys and tokens (Slack bot/app tokens, LLM API keys, ...) are GLOBAL
  configurations attached to an agent definition. The user sets them in the
  application settings. They are not settable through this server and must
  NEVER be written into an agent instance's configs.

TYPICAL WORKFLOW
1. list_agent_definitions to discover agents (get_agent_definition for
   details on one).
2. create_preset, then add_agent per node and add_connection per edge.
3. save_preset to persist, start_preset / stop_preset to run.

VERIFYING A FLOW
1. start_preset to run the workflow.
2. write_external_input to feed a test value into an external input channel
   (an ExternalInputAgent's configured name), or wait for a real event
   source (e.g. a Slack message).
3. Poll get_external_outputs and get_agent_errors. Both return latest_seq;
   pass it back as since_seq on the next call to receive only new records.
   dropped > 0 means the collector fell behind the event stream and some
   records were lost before capture.
4. stop_preset when done.

WORKED EXAMPLE - "listen to a Slack channel, send each message to a chat
LLM, post the reply back to Slack":
  A: modular_agent_slack::agents::SlackListenerAgent   at x=0,   y=0
  B: modular_agent_slack::agents::SlackToMessageAgent  at x=240, y=0
  C: modular_agent_llm::chat::ChatAgent                at x=480, y=0
  D: modular_agent_slack::agents::SlackPostAgent       at x=720, y=0
Connections:
  A.value   -> B.value
  B.message -> C.message
  C.message -> D.message
Slack tokens and the LLM API key are global configs set by the user."#;

/// Configuration for [`start_mcp_server`].
#[derive(Clone)]
pub struct McpServerConfig {
    /// TCP port to bind on 127.0.0.1 (endpoint path is `/mcp`).
    pub port: u16,
    /// Root directory where the `save_preset` tool writes preset JSON files
    /// (`<presets_dir>/<name>.json`). When `None`, saving is unavailable.
    pub presets_dir: Option<PathBuf>,
    /// Bearer token required on every request. `None` disables
    /// authentication.
    pub token: Option<String>,
}

/// Handle to a running MCP server. Dropping it does NOT stop the server;
/// call [`stop`](Self::stop).
pub struct McpServerHandle {
    cancel: CancellationToken,
    server: tokio::task::JoinHandle<()>,
    collector: tokio::task::JoinHandle<()>,
}

impl McpServerHandle {
    /// Stop the server: terminates active sessions and waits for the
    /// listener and event-collector tasks to finish.
    pub async fn stop(self) {
        self.cancel.cancel();
        for task in [self.server, self.collector] {
            if let Err(e) = task.await {
                log::warn!("MCP server task join error: {}", e);
            }
        }
    }
}

/// Start the built-in MCP server on `127.0.0.1:<port>` (endpoint `/mcp`).
pub async fn start_mcp_server(
    ma: ModularAgent,
    config: McpServerConfig,
) -> Result<McpServerHandle, AgentError> {
    // Stamp here so every change made through MCP tools carries the "mcp"
    // origin even if the host passes a plain handle.
    let ma = ma.with_origin("mcp");

    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, config.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| AgentError::IoError(format!("Failed to bind MCP server to {addr}: {e}")))?;

    let cancel = CancellationToken::new();

    // One ring shared by all sessions: runtime events keep accumulating
    // while no session is polling, so a session connecting later still sees
    // recent errors/outputs.
    let ring = Arc::new(EventRing::new());
    let collector = tokio::spawn(run_event_collector(
        ma.clone(),
        ring.clone(),
        cancel.child_token(),
    ));

    let presets_dir = config.presets_dir;
    let service = StreamableHttpService::new(
        {
            let ma = ma.clone();
            let ring = ring.clone();
            move || {
                Ok(McpServer::new(
                    ma.clone(),
                    presets_dir.clone(),
                    ring.clone(),
                ))
            }
        },
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default().with_cancellation_token(cancel.child_token()),
    );
    let mut router = axum::Router::new().nest_service("/mcp", service);
    if let Some(token) = config.token {
        let expected: Arc<str> = token.into();
        router = router.route_layer(middleware::from_fn(move |req: Request, next: Next| {
            let expected = expected.clone();
            async move { require_bearer(expected, req, next).await }
        }));
    }

    let shutdown = cancel.clone();
    let server = tokio::spawn(async move {
        let result = axum::serve(listener, router)
            .with_graceful_shutdown(async move { shutdown.cancelled().await })
            .await;
        if let Err(e) = result {
            log::error!("MCP server error: {}", e);
        }
        log::info!("MCP server stopped");
    });
    log::info!("MCP server listening on http://{addr}/mcp");

    Ok(McpServerHandle {
        cancel,
        server,
        collector,
    })
}

// --- Bearer authentication ---

/// Constant-time byte comparison: XOR-folds all byte pairs so response
/// timing does not reveal the position of the first mismatch. The length
/// check leaks only the token length.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn require_bearer(expected: Arc<str>, req: Request, next: Next) -> Response {
    let authorized = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|t| constant_time_eq(t.as_bytes(), expected.as_bytes()));
    if authorized {
        next.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
        )
            .into_response()
    }
}

// --- Runtime event ring buffer ---

const RING_CAPACITY: usize = 200;
const DEFAULT_POLL_LIMIT: usize = 50;

/// Upper bound on waiting for an agent's mutex when resolving the preset id
/// of a captured error; see [`run_event_collector`].
const PRESET_ID_RESOLVE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100);

#[derive(Clone, Serialize)]
struct ErrorRecord {
    seq: u64,
    time_ms: u64,
    preset_id: Option<String>,
    agent_id: String,
    message: String,
}

#[derive(Clone, Serialize)]
struct OutputRecord {
    seq: u64,
    time_ms: u64,
    channel: String,
    value: AgentValue,
}

/// Bounded capture of runtime events for polling tools. `seq` is a single
/// monotonic counter across both buffers; each buffer stores records in
/// seq order, so paging with the seq of the last returned record as
/// since_seq never loses a record and never returns one twice.
struct EventRing {
    seq: AtomicU64,
    dropped: AtomicU64,
    errors: Mutex<VecDeque<ErrorRecord>>,
    outputs: Mutex<VecDeque<OutputRecord>>,
}

impl EventRing {
    fn new() -> Self {
        Self {
            seq: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            errors: Mutex::new(VecDeque::new()),
            outputs: Mutex::new(VecDeque::new()),
        }
    }

    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    // Seq allocation happens while the buffer lock is held so the deque is
    // always sorted by seq — a record can never appear "behind" one a
    // reader has already seen, which the paging cursor depends on.

    fn push_error(
        &self,
        time_ms: u64,
        preset_id: Option<String>,
        agent_id: String,
        message: String,
    ) {
        let mut errors = self.errors.lock().unwrap();
        if errors.len() >= RING_CAPACITY {
            errors.pop_front();
        }
        errors.push_back(ErrorRecord {
            seq: self.next_seq(),
            time_ms,
            preset_id,
            agent_id,
            message,
        });
    }

    fn push_output(&self, time_ms: u64, channel: String, value: AgentValue) {
        let mut outputs = self.outputs.lock().unwrap();
        if outputs.len() >= RING_CAPACITY {
            outputs.pop_front();
        }
        outputs.push_back(OutputRecord {
            seq: self.next_seq(),
            time_ms,
            channel,
            value,
        });
    }

    /// Oldest-first page of errors after `since_seq`; truncated to `limit`
    /// so the caller can continue from the last returned seq.
    fn collect_errors(
        &self,
        preset_id: Option<&str>,
        since_seq: u64,
        limit: usize,
    ) -> Vec<ErrorRecord> {
        let errors = self.errors.lock().unwrap();
        let mut result: Vec<ErrorRecord> = errors
            .iter()
            .filter(|r| r.seq > since_seq)
            .filter(|r| preset_id.is_none_or(|p| r.preset_id.as_deref() == Some(p)))
            .cloned()
            .collect();
        result.truncate(limit);
        result
    }

    /// Oldest-first page of external outputs after `since_seq`.
    fn collect_outputs(
        &self,
        channel: Option<&str>,
        since_seq: u64,
        limit: usize,
    ) -> Vec<OutputRecord> {
        let outputs = self.outputs.lock().unwrap();
        let mut result: Vec<OutputRecord> = outputs
            .iter()
            .filter(|r| r.seq > since_seq)
            .filter(|r| channel.is_none_or(|c| r.channel == c))
            .cloned()
            .collect();
        result.truncate(limit);
        result
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

async fn run_event_collector(ma: ModularAgent, ring: Arc<EventRing>, cancel: CancellationToken) {
    let mut rx = ma.subscribe();
    loop {
        let envelope = tokio::select! {
            _ = cancel.cancelled() => break,
            recv = rx.recv() => match recv {
                Ok(envelope) => envelope,
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    ring.dropped.fetch_add(n, Ordering::Relaxed);
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
        };
        match envelope.event {
            ModularAgentEvent::AgentError(agent_id, message) => {
                // Resolve the preset at capture time: once the agent is
                // gone the mapping is unrecoverable. The agent mutex is
                // held for the whole duration of process() and errors are
                // emitted while it is still held, so an unbounded lock
                // here could stall capture behind a long-running agent
                // until broadcast events are dropped as Lagged. A short
                // bounded wait resolves the common case (the agent loop
                // releases the lock right after emitting) and otherwise
                // records the error without a preset id.
                let preset_id = match ma.get_agent(&agent_id) {
                    Some(agent) => {
                        match tokio::time::timeout(PRESET_ID_RESOLVE_TIMEOUT, agent.lock()).await {
                            Ok(agent) => Some(agent.preset_id().to_string()),
                            Err(_) => None,
                        }
                    }
                    None => None,
                };
                ring.push_error(now_ms(), preset_id, agent_id, message);
            }
            ModularAgentEvent::ExternalOutput(channel, value) => {
                ring.push_output(now_ms(), channel, value);
            }
            _ => {}
        }
    }
}

struct McpServer {
    ma: ModularAgent,
    presets_dir: Option<PathBuf>,
    ring: Arc<EventRing>,
}

impl McpServer {
    fn new(ma: ModularAgent, presets_dir: Option<PathBuf>, ring: Arc<EventRing>) -> Self {
        Self {
            ma,
            presets_dir,
            ring,
        }
    }
}

// Tool methods return Result<CallToolResult, McpError> because that is what
// rmcp's IntoCallToolResult accepts; failures an external agent can fix by
// itself (wrong names, invalid values) are reported as is_error results
// (err_text), not protocol errors.
fn ok_text(text: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

fn err_text(text: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::error(vec![ContentBlock::text(text)]))
}

fn ok_json(value: &impl serde::Serialize) -> Result<CallToolResult, McpError> {
    match serde_json::to_string_pretty(value) {
        Ok(json) => ok_text(json),
        Err(e) => err_text(format!("Failed to serialize result: {e}")),
    }
}

/// Error text for an agent id that has no live instance.
///
/// A spec-only agent (its definition is not registered in this build) is
/// still listed by get_preset_spec, so pointing the caller there would send
/// it in circles; name the unregistered definition instead. Only a truly
/// absent id gets the get_preset_spec hint. `role` is the label used for the
/// agent in the message ("Agent", "Source agent", ...).
async fn missing_agent_text(ma: &ModularAgent, role: &str, agent_id: &str) -> String {
    match ma.find_stored_agent_spec(agent_id).await {
        Some(spec) => format!(
            "{role} \"{agent_id}\" exists in the preset, but its definition \"{}\" is not \
             registered in this build, so this tool cannot operate on it.",
            spec.def_name
        ),
        None => format!("{role} \"{agent_id}\" not found. Use get_preset_spec to list agent ids."),
    }
}

/// Maps a preset-name error to guidance the calling agent can act on.
fn preset_error_text(e: AgentError) -> String {
    match e {
        AgentError::PresetNameExists(name) => format!(
            "Preset name \"{name}\" already exists. Use list_presets to see open presets \
             (and their ids), then pick a different name or work with the existing preset."
        ),
        e => e.to_string(),
    }
}

/// Strips `config_specs` (per-config UI metadata) from serialized agent
/// specs: it is bulky and irrelevant for external editing agents.
fn strip_config_specs(value: &mut Value) {
    if let Some(obj) = value.as_object_mut() {
        obj.remove("config_specs");
    }
}

fn preset_spec_result(spec: &PresetSpec) -> Result<CallToolResult, McpError> {
    let mut value = match serde_json::to_value(spec) {
        Ok(v) => v,
        Err(e) => return err_text(format!("Failed to serialize preset spec: {e}")),
    };
    if let Some(agents) = value.get_mut("agents").and_then(|a| a.as_array_mut()) {
        for agent in agents {
            strip_config_specs(agent);
        }
    }
    ok_json(&value)
}

/// Truncates long default values so the definition listing stays compact.
fn compact_default(value: &AgentValue) -> String {
    let mut s = serde_json::to_string(value).unwrap_or_else(|_| "?".into());
    if s.chars().count() > 40 {
        s = format!("{}\u{2026}", s.chars().take(40).collect::<String>());
    }
    s
}

fn definition_line(def: &AgentDefinition) -> String {
    let title = def.title.as_deref().unwrap_or("-");
    let category = def.category.as_deref().unwrap_or("-");
    let description = def
        .description
        .as_deref()
        .and_then(|d| d.lines().next())
        .unwrap_or("")
        .trim();
    let inputs = def
        .inputs
        .as_deref()
        .map(|p| p.join(","))
        .unwrap_or_default();
    let outputs = def
        .outputs
        .as_deref()
        .map(|p| p.join(","))
        .unwrap_or_default();
    let configs = def
        .configs
        .iter()
        .flatten()
        .filter(|(_, spec)| !spec.hidden && !spec.readonly)
        .map(|(key, spec)| {
            format!(
                "{key}:{}={}",
                spec.type_.as_deref().unwrap_or("any"),
                compact_default(&spec.value)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} | {title} ({category}) | {description} | in: {inputs} | out: {outputs} | configs: {configs}",
        def.name
    )
}

/// Validates a preset name used as a relative path under the presets
/// directory. Names come from an external agent, so this is a boundary check.
fn validate_preset_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Preset name must not be empty".into());
    }
    if name.contains('\\') {
        return Err("Preset name must use '/' as the folder separator".into());
    }
    // Path::components() normalizes "." away, so check the raw segments.
    let segments_ok = name
        .split('/')
        .all(|s| !s.is_empty() && s != "." && s != ".." && !s.ends_with(':'));
    if !segments_ok {
        return Err(format!(
            "Invalid preset name \"{name}\": must be a relative path without empty, \".\" or \"..\" components"
        ));
    }
    Ok(())
}

/// Rejects global config keys: secrets live in the application settings and
/// must never be written onto an agent instance. Global configs belong to the
/// definition, so this check never needs a live spec.
fn reject_global_config_keys<'a>(
    def: &AgentDefinition,
    keys: impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    for key in keys {
        if def
            .global_configs
            .as_ref()
            .is_some_and(|g| g.contains_key(key))
        {
            return Err(format!(
                "\"{key}\" is a GLOBAL configuration of {}. Global configs (API keys, tokens, secrets) \
                 are set by the user in the application settings and must never be set on an agent instance.",
                def.name
            ));
        }
    }
    Ok(())
}

/// Checks candidate config keys against an agent definition and, when the
/// instance already exists, against its live spec.
///
/// Agents such as Switch generate `c0`..`c(n-1)` from their `n` config in
/// `new()` / `configs_changed()`, so the definition alone does not know which
/// keys an instance accepts; `live` is the authority whenever it is
/// available. Keys starting with `_` are the stale keys parked by
/// [`AgentDefinition::reconcile_spec`] and are never settable.
fn validate_config_keys<'a>(
    def: &AgentDefinition,
    live: Option<&AgentSpec>,
    keys: impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    let keys: Vec<&String> = keys.collect();
    reject_global_config_keys(def, keys.iter().copied())?;

    let live_keys: Option<Vec<&str>> = live.and_then(|spec| {
        let config_specs = spec.config_specs.as_ref();
        let configs = spec.configs.as_ref();
        if config_specs.is_none() && configs.is_none() {
            return None;
        }
        let mut merged: Vec<&str> = Vec::new();
        for key in config_specs
            .into_iter()
            .flat_map(|cs| cs.keys())
            .chain(configs.into_iter().flat_map(|c| c.keys()))
        {
            if !merged.contains(&key.as_str()) {
                merged.push(key);
            }
        }
        Some(merged)
    });

    let valid_list = || {
        let valid: Vec<&str> = match &live_keys {
            Some(live_keys) => live_keys.clone(),
            None => def
                .configs
                .iter()
                .flatten()
                .map(|(k, _)| k.as_str())
                .collect(),
        };
        valid
            .into_iter()
            .filter(|k| !k.starts_with('_'))
            .collect::<Vec<_>>()
            .join(", ")
    };

    for key in keys {
        if key.starts_with('_') {
            return Err(format!(
                "Config \"{key}\" of {} is reserved: keys starting with \"_\" hold values parked by a \
                 definition change and cannot be set. Valid configs: [{}]",
                def.name,
                valid_list()
            ));
        }
        let known = match &live_keys {
            Some(live_keys) => live_keys.contains(&key.as_str()),
            None => def.configs.as_ref().is_some_and(|c| c.contains_key(key)),
        };
        if !known {
            return Err(format!(
                "Unknown config \"{key}\" for {}. Valid configs: [{}]",
                def.name,
                valid_list()
            ));
        }
    }
    Ok(())
}

fn json_to_agent_value(key: &str, value: Value) -> Result<AgentValue, String> {
    serde_json::from_value(value).map_err(|e| format!("Invalid value for config \"{key}\": {e}"))
}

// --- Tool parameter types ---

#[derive(Deserialize, JsonSchema)]
struct GetAgentDefinitionParams {
    /// Fully-qualified agent definition name (as listed by list_agent_definitions).
    def_name: String,
}

#[derive(Deserialize, JsonSchema)]
struct CreatePresetParams {
    /// Preset name; use '/' for folders (e.g. "MyFolder/MyPreset").
    name: String,
}

#[derive(Deserialize, JsonSchema)]
struct PresetIdParams {
    /// Preset id (as returned by create_preset / list_presets).
    preset_id: String,
}

#[derive(Deserialize, JsonSchema)]
struct AddAgentParams {
    /// Preset id to add the agent to.
    preset_id: String,
    /// Fully-qualified agent definition name.
    def_name: String,
    /// Initial config values (key -> value). Only per-instance configs
    /// declared by the definition are allowed; never pass secrets here.
    /// Some agents derive further configs and ports from a config such as
    /// "n" (e.g. Switch); pass that config here and the keys it generates
    /// become settable - the returned spec lists them.
    configs: Option<serde_json::Map<String, Value>>,
    /// Node x position in pixels (grid unit 240).
    x: Option<f64>,
    /// Node y position in pixels (grid unit 240).
    y: Option<f64>,
    /// Node width in pixels. If omitted, the editor sizes the node from the
    /// definition's hints (grid-unit multipliers; usually leave unset).
    /// Definitions with a free_size hint do not follow the 240 grid.
    width: Option<f64>,
    /// Node height in pixels. If omitted, the editor sizes the node from the
    /// definition's hints (grid-unit multipliers; usually leave unset).
    /// Definitions with a free_size hint do not follow the 240 grid.
    height: Option<f64>,
}

#[derive(Deserialize, JsonSchema)]
struct UpdateAgentSpecParams {
    /// Agent id (as shown in get_preset_spec).
    agent_id: String,
    /// Partial spec patch. Recognized keys: "configs" (object, merged into
    /// current configs), "disabled" (bool), and layout/extension fields such
    /// as "x", "y", "width", "height", "title". A null extension value
    /// removes that field. "id" and "def_name" cannot be changed.
    patch: serde_json::Map<String, Value>,
}

#[derive(Deserialize, JsonSchema)]
struct SetAgentConfigsParams {
    /// Agent id (as shown in get_preset_spec).
    agent_id: String,
    /// Config values to set (key -> value); merged into current configs.
    configs: serde_json::Map<String, Value>,
}

#[derive(Deserialize, JsonSchema)]
struct RemoveAgentParams {
    /// Preset id containing the agent.
    preset_id: String,
    /// Agent id to remove.
    agent_id: String,
}

#[derive(Deserialize, JsonSchema)]
struct ConnectionParams {
    /// Preset id containing both agents.
    preset_id: String,
    /// Source agent id.
    source: String,
    /// Output port name on the source agent (or "err").
    source_handle: String,
    /// Target agent id.
    target: String,
    /// Input port name on the target agent, or "config:<key>".
    target_handle: String,
}

#[derive(Deserialize, JsonSchema)]
struct SavePresetParams {
    /// Preset id to save.
    preset_id: String,
    /// Preset name to save as (relative path without extension, '/' for
    /// folders). Defaults to the preset's current name.
    name: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct WriteExternalInputParams {
    /// External input channel name (the name configured on an
    /// ExternalInputAgent in a running preset).
    channel: String,
    /// JSON value to send into the channel.
    value: Value,
}

#[derive(Deserialize, JsonSchema)]
struct GetAgentErrorsParams {
    /// Only return errors from this preset.
    preset_id: Option<String>,
    /// Poll cursor: only return records with seq greater than this. Use the
    /// latest_seq from a previous call; it equals the seq of the last
    /// record that call returned.
    since_seq: Option<u64>,
    /// Maximum number of records to return, oldest first (default 50).
    limit: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
struct GetExternalOutputsParams {
    /// Only return outputs on this channel.
    channel: Option<String>,
    /// Poll cursor: only return records with seq greater than this. Use the
    /// latest_seq from a previous call; it equals the seq of the last
    /// record that call returned.
    since_seq: Option<u64>,
    /// Maximum number of records to return, oldest first (default 50).
    limit: Option<usize>,
}

// --- Tools ---

#[tool_router]
impl McpServer {
    /// List all available agent definitions in a compact one-line-per-agent
    /// format. Hidden/readonly configs and global configs are omitted.
    #[tool]
    async fn list_agent_definitions(&self) -> Result<CallToolResult, McpError> {
        let defs = self.ma.get_agent_definitions();
        let mut lines = vec![
            "Format: def_name | Title (Category) | description | in: input_ports | out: output_ports | configs: key:type=default, ...".to_string(),
            String::new(),
        ];
        lines.extend(defs.values().map(definition_line));
        ok_text(lines.join("\n"))
    }

    /// Get the full JSON definition of a single agent (ports, configs with
    /// types/defaults/descriptions, UI hints).
    #[tool]
    async fn get_agent_definition(
        &self,
        Parameters(p): Parameters<GetAgentDefinitionParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.ma.get_agent_definition(&p.def_name) {
            Some(def) => ok_json(&def),
            None => err_text(format!(
                "Unknown agent definition \"{}\". Use list_agent_definitions to see available definitions.",
                p.def_name
            )),
        }
    }

    /// List all currently open presets (id, name, running state).
    #[tool]
    async fn list_presets(&self) -> Result<CallToolResult, McpError> {
        ok_json(&self.ma.get_preset_infos().await)
    }

    /// Create a new empty preset with the given name. Returns the preset id
    /// used by all other preset tools.
    #[tool]
    async fn create_preset(
        &self,
        Parameters(p): Parameters<CreatePresetParams>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(e) = validate_preset_name(&p.name) {
            return err_text(e);
        }
        match self.ma.new_preset_with_name(p.name) {
            Ok(id) => ok_json(&serde_json::json!({ "preset_id": id })),
            Err(e) => err_text(preset_error_text(e)),
        }
    }

    /// Get the live spec of a preset: all agents (with ids, configs, layout)
    /// and connections. This is the current editor canvas state.
    #[tool]
    async fn get_preset_spec(
        &self,
        Parameters(p): Parameters<PresetIdParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.ma.get_preset_spec(&p.preset_id).await {
            Some(spec) => preset_spec_result(&spec),
            None => err_text(format!(
                "Preset \"{}\" not found. Use list_presets to see open presets.",
                p.preset_id
            )),
        }
    }

    /// Add an agent to a preset. Returns the created agent spec including
    /// its assigned id (needed for add_connection).
    #[tool]
    async fn add_agent(
        &self,
        Parameters(p): Parameters<AddAgentParams>,
    ) -> Result<CallToolResult, McpError> {
        let Some(def) = self.ma.get_agent_definition(&p.def_name) else {
            return err_text(format!(
                "Unknown agent definition \"{}\". Use list_agent_definitions to see available definitions.",
                p.def_name
            ));
        };
        if let Some(configs) = &p.configs
            && let Err(e) = reject_global_config_keys(&def, configs.keys())
        {
            return err_text(e);
        }

        // Configs the definition declares go into the spec the agent is built
        // from. The rest may still be valid - agents like Switch generate
        // configs from `n` in new() - so they are applied and validated once
        // the instance exists. Values are converted up front either way, so a
        // type error fails before anything is created.
        let mut spec = def.to_spec();
        let mut deferred: Vec<(String, AgentValue)> = Vec::new();
        if let Some(configs) = p.configs {
            for (key, value) in configs {
                let agent_value = match json_to_agent_value(&key, value) {
                    Ok(v) => v,
                    Err(e) => return err_text(e),
                };
                match spec.configs.as_mut() {
                    Some(spec_configs) if spec_configs.contains_key(&key) => {
                        spec_configs.set(key, agent_value);
                    }
                    _ => deferred.push((key, agent_value)),
                }
            }
        }
        for (key, value) in [
            ("x", p.x),
            ("y", p.y),
            ("width", p.width),
            ("height", p.height),
        ] {
            if let Some(value) = value {
                spec.extensions.insert(key.into(), serde_json::json!(value));
            }
        }

        let preset_id = p.preset_id.clone();
        let agent_id = match self.ma.add_agent(p.preset_id, spec).await {
            Ok(id) => id,
            Err(e) => return err_text(e.to_string()),
        };

        let mut warning = None;
        if !deferred.is_empty() {
            let constructed = self.ma.get_agent_spec(&agent_id).await;
            let validated = match &constructed {
                Some(constructed) => validate_config_keys(
                    &def,
                    Some(constructed),
                    deferred.iter().map(|(key, _)| key),
                ),
                None => Err(format!(
                    "Agent \"{agent_id}\" vanished right after it was added"
                )),
            };
            if let Err(e) = validated {
                let mut message = format!(
                    "{e}\nSome agents derive extra configs and ports from another config (e.g. \"n\"). \
                     Set that config when adding the agent, then read the returned spec for the keys \
                     it generated."
                );
                if let Err(rollback) = self.ma.remove_agent(&preset_id, &agent_id).await {
                    log::warn!("Failed to roll back agent {agent_id}: {rollback}");
                    message.push_str(&format!(
                        "\nRolling the agent back failed, so it remains in the preset as \"{agent_id}\": {rollback}"
                    ));
                }
                return err_text(message);
            }

            let mut merged: AgentConfigs = constructed
                .and_then(|spec| spec.configs)
                .unwrap_or_default();
            for (key, value) in deferred {
                merged.set(key, value);
            }
            // A config error here is reported by an agent that has already
            // committed the new values (an unparsable Switch condition is kept
            // as never-matching), so the agent stays and the error is a
            // warning on the created spec.
            if let Err(e) = self.ma.set_agent_configs(agent_id.clone(), merged).await {
                warning = Some(format!("Agent created, but applying configs reported: {e}"));
            }
        }

        match self.ma.get_agent_spec(&agent_id).await {
            Some(created) => {
                let mut value = match serde_json::to_value(&created) {
                    Ok(v) => v,
                    Err(e) => return err_text(format!("Failed to serialize agent spec: {e}")),
                };
                strip_config_specs(&mut value);
                if let Some(warning) = warning
                    && let Some(object) = value.as_object_mut()
                {
                    object.insert("warning".into(), Value::String(warning));
                }
                ok_json(&value)
            }
            None => ok_json(&serde_json::json!({ "agent_id": agent_id })),
        }
    }

    /// Update an agent's spec: configs (merged), disabled flag, or layout /
    /// extension fields (x, y, width, height, title, ...).
    #[tool]
    async fn update_agent_spec(
        &self,
        Parameters(p): Parameters<UpdateAgentSpecParams>,
    ) -> Result<CallToolResult, McpError> {
        let patch = p.patch;
        if patch.contains_key("id") || patch.contains_key("def_name") {
            return err_text("\"id\" and \"def_name\" cannot be changed");
        }

        let Some(current) = self.ma.get_agent_spec(&p.agent_id).await else {
            return err_text(missing_agent_text(&self.ma, "Agent", &p.agent_id).await);
        };
        if let Some(configs_patch) = patch.get("configs") {
            let Value::Object(configs_patch) = configs_patch else {
                return err_text("\"configs\" must be a JSON object");
            };
            if let Some(def) = self.ma.get_agent_definition(&current.def_name)
                && let Err(e) = validate_config_keys(&def, Some(&current), configs_patch.keys())
            {
                return err_text(e);
            }
        }

        match self
            .ma
            .update_agent_spec(&p.agent_id, &Value::Object(patch))
            .await
        {
            Ok(()) => ok_text("Agent spec updated"),
            Err(e) => err_text(e.to_string()),
        }
    }

    /// Set config values on an agent (merged into its current configs).
    #[tool]
    async fn set_agent_configs(
        &self,
        Parameters(p): Parameters<SetAgentConfigsParams>,
    ) -> Result<CallToolResult, McpError> {
        let Some(current) = self.ma.get_agent_spec(&p.agent_id).await else {
            return err_text(missing_agent_text(&self.ma, "Agent", &p.agent_id).await);
        };
        if let Some(def) = self.ma.get_agent_definition(&current.def_name)
            && let Err(e) = validate_config_keys(&def, Some(&current), p.configs.keys())
        {
            return err_text(e);
        }

        let mut merged: AgentConfigs = current.configs.unwrap_or_default();
        for (key, value) in p.configs {
            let agent_value = match json_to_agent_value(&key, value) {
                Ok(v) => v,
                Err(e) => return err_text(e),
            };
            merged.set(key, agent_value);
        }

        match self.ma.set_agent_configs(p.agent_id, merged).await {
            Ok(()) => ok_text("Agent configs updated"),
            Err(e) => err_text(e.to_string()),
        }
    }

    /// Remove an agent (and all its connections) from a preset.
    #[tool]
    async fn remove_agent(
        &self,
        Parameters(p): Parameters<RemoveAgentParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.ma.remove_agent(&p.preset_id, &p.agent_id).await {
            Ok(()) => ok_text("Agent removed"),
            Err(e) => err_text(e.to_string()),
        }
    }

    /// Connect a source agent's output port to a target agent's input port
    /// (or "config:<key>" to drive a config value).
    #[tool]
    async fn add_connection(
        &self,
        Parameters(p): Parameters<ConnectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let Some(source_spec) = self.ma.get_agent_spec(&p.source).await else {
            return err_text(missing_agent_text(&self.ma, "Source agent", &p.source).await);
        };
        let Some(target_spec) = self.ma.get_agent_spec(&p.target).await else {
            return err_text(missing_agent_text(&self.ma, "Target agent", &p.target).await);
        };

        let outputs = source_spec.outputs.unwrap_or_default();
        if p.source_handle != "err" && !outputs.contains(&p.source_handle) {
            return err_text(format!(
                "Invalid source_handle \"{}\" for agent {} ({}). Valid source handles: [{}] or \"err\".",
                p.source_handle,
                p.source,
                source_spec.def_name,
                outputs.join(", ")
            ));
        }

        let inputs = target_spec.inputs.unwrap_or_default();
        let config_keys: Vec<String> = target_spec
            .configs
            .as_ref()
            .map(|c| c.keys().cloned().collect())
            .unwrap_or_default();
        let target_valid = match p.target_handle.strip_prefix("config:") {
            Some(key) => config_keys.iter().any(|k| k == key),
            None => inputs.contains(&p.target_handle),
        };
        if !target_valid {
            let config_handles = config_keys
                .iter()
                .map(|k| format!("config:{k}"))
                .collect::<Vec<_>>()
                .join(", ");
            return err_text(format!(
                "Invalid target_handle \"{}\" for agent {} ({}). Valid target handles: [{}] or [{}].",
                p.target_handle,
                p.target,
                target_spec.def_name,
                inputs.join(", "),
                config_handles
            ));
        }

        let connection = ConnectionSpec {
            source: p.source,
            source_handle: p.source_handle,
            target: p.target,
            target_handle: p.target_handle,
        };
        match self.ma.add_connection(&p.preset_id, connection).await {
            Ok(()) => ok_text("Connection added"),
            Err(e) => err_text(e.to_string()),
        }
    }

    /// Remove a connection from a preset. All four fields must match an
    /// existing connection exactly (see get_preset_spec).
    #[tool]
    async fn remove_connection(
        &self,
        Parameters(p): Parameters<ConnectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let connection = ConnectionSpec {
            source: p.source,
            source_handle: p.source_handle,
            target: p.target,
            target_handle: p.target_handle,
        };
        match self.ma.remove_connection(&p.preset_id, &connection).await {
            Ok(()) => ok_text("Connection removed"),
            Err(e) => err_text(e.to_string()),
        }
    }

    /// Save a preset to disk as <presets_dir>/<name>.json.
    #[tool]
    async fn save_preset(
        &self,
        Parameters(p): Parameters<SavePresetParams>,
    ) -> Result<CallToolResult, McpError> {
        let Some(presets_dir) = &self.presets_dir else {
            return err_text(
                "Saving is unavailable: this MCP server was started without a presets directory.",
            );
        };

        let current_name = self
            .ma
            .get_preset_info(&p.preset_id)
            .await
            .and_then(|info| info.name);
        let Some(name) = p.name.or(current_name.clone()) else {
            return err_text("Preset has no name; pass the \"name\" parameter to save it.");
        };
        if let Err(e) = validate_preset_name(&name) {
            return err_text(e);
        }
        let renamed_from = match &current_name {
            Some(old) if old != &name => Some(old.clone()),
            _ => None,
        };
        if renamed_from.is_some()
            && let Err(e) = self.ma.rename_preset(&p.preset_id, name.clone()).await
        {
            return err_text(preset_error_text(e));
        }

        let path = presets_dir.join(format!("{name}.json"));
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            return err_text(format!("Failed to create preset directory: {e}"));
        }
        let path_str = path.to_string_lossy().to_string();
        match self.ma.save_preset(&p.preset_id, &path_str).await {
            Ok(()) => {
                // A rename-on-save is a move: drop the old file so the two
                // names cannot diverge.
                if let Some(old_name) = renamed_from {
                    let old_path = presets_dir.join(format!("{old_name}.json"));
                    if old_path.exists()
                        && let Err(e) = std::fs::remove_file(&old_path)
                    {
                        log::warn!(
                            "Failed to remove old preset file {}: {e}",
                            old_path.display()
                        );
                    }
                }
                ok_text(format!("Saved preset \"{name}\""))
            }
            Err(e) => err_text(e.to_string()),
        }
    }

    /// Start all agents in a preset (run the workflow).
    #[tool]
    async fn start_preset(
        &self,
        Parameters(p): Parameters<PresetIdParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.ma.start_preset(&p.preset_id).await {
            Ok(()) => ok_text("Preset started"),
            Err(e) => err_text(e.to_string()),
        }
    }

    /// Stop all agents in a preset.
    #[tool]
    async fn stop_preset(
        &self,
        Parameters(p): Parameters<PresetIdParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.ma.stop_preset(&p.preset_id).await {
            Ok(()) => ok_text("Preset stopped"),
            Err(e) => err_text(e.to_string()),
        }
    }

    /// Write a test value into an external input channel of a running
    /// preset. Use get_external_outputs / get_agent_errors afterwards to
    /// observe the result.
    #[tool]
    async fn write_external_input(
        &self,
        Parameters(p): Parameters<WriteExternalInputParams>,
    ) -> Result<CallToolResult, McpError> {
        let value: AgentValue = match serde_json::from_value(p.value) {
            Ok(v) => v,
            Err(e) => return err_text(format!("Invalid value: {e}")),
        };
        match self.ma.write_external_input(p.channel, value).await {
            Ok(()) => ok_text("Input written"),
            Err(e) => err_text(e.to_string()),
        }
    }

    /// Get recent agent errors (from running presets). Poll with since_seq
    /// set to the previous latest_seq to receive only new records; dropped
    /// counts events lost when the collector fell behind the event stream.
    #[tool]
    async fn get_agent_errors(
        &self,
        Parameters(p): Parameters<GetAgentErrorsParams>,
    ) -> Result<CallToolResult, McpError> {
        let since_seq = p.since_seq.unwrap_or(0);
        let errors = self.ring.collect_errors(
            p.preset_id.as_deref(),
            since_seq,
            p.limit.unwrap_or(DEFAULT_POLL_LIMIT),
        );
        // The cursor is the seq of the last record actually returned, not
        // a global counter read: a record whose seq is already allocated
        // but not yet pushed would otherwise be skipped forever by the
        // next poll.
        let latest_seq = errors.last().map_or(since_seq, |r| r.seq);
        ok_json(&serde_json::json!({
            "latest_seq": latest_seq,
            "dropped": self.ring.dropped(),
            "errors": errors,
        }))
    }

    /// Get recent external output values (from running presets). Poll with
    /// since_seq set to the previous latest_seq to receive only new records;
    /// dropped counts events lost when the collector fell behind the event
    /// stream.
    #[tool]
    async fn get_external_outputs(
        &self,
        Parameters(p): Parameters<GetExternalOutputsParams>,
    ) -> Result<CallToolResult, McpError> {
        let since_seq = p.since_seq.unwrap_or(0);
        let outputs = self.ring.collect_outputs(
            p.channel.as_deref(),
            since_seq,
            p.limit.unwrap_or(DEFAULT_POLL_LIMIT),
        );
        let latest_seq = outputs.last().map_or(since_seq, |r| r.seq);
        ok_json(&serde_json::json!({
            "latest_seq": latest_seq,
            "dropped": self.ring.dropped(),
            "outputs": outputs,
        }))
    }
}

#[tool_handler]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("modular-agent", env!("CARGO_PKG_VERSION"))
                    .with_title("Modular Agent"),
            )
            .with_instructions(SERVER_INSTRUCTIONS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_name_validation() {
        assert!(validate_preset_name("MyPreset").is_ok());
        assert!(validate_preset_name("Folder/MyPreset").is_ok());
        assert!(validate_preset_name("").is_err());
        assert!(validate_preset_name("../escape").is_err());
        assert!(validate_preset_name("/absolute").is_err());
        assert!(validate_preset_name("a\\b").is_err());
        assert!(validate_preset_name("a/./b").is_err());
    }

    #[test]
    fn config_key_validation_rejects_global_and_unknown_keys() {
        let def = AgentDefinition::new("test", "t", None)
            .string_config("channel", "")
            .string_global_config("slack_bot_token", "");

        assert!(validate_config_keys(&def, None, ["channel".to_string()].iter()).is_ok());

        let err = validate_config_keys(&def, None, ["slack_bot_token".to_string()].iter())
            .expect_err("global key must be rejected");
        assert!(err.contains("GLOBAL"));

        let err = validate_config_keys(&def, None, ["nope".to_string()].iter())
            .expect_err("unknown key must be rejected");
        assert!(err.contains("channel"));
    }

    /// A definition of a Switch-like agent plus the spec of an instance that
    /// generated `c2` (value only), `c3` (spec only) and parked a stale
    /// `_c9`.
    fn numbered_def_and_live_spec() -> (AgentDefinition, AgentSpec) {
        use crate::definition::AgentConfigSpec;

        let def = AgentDefinition::new("test", "t", None)
            .integer_config("n", 2)
            .string_config("c0", "")
            .string_config("c1", "")
            .string_global_config("api_key", "");

        let mut live = def.to_spec();
        let mut configs = live.configs.take().unwrap_or_default();
        configs.set("c2".into(), AgentValue::string(""));
        configs.set("_c9".into(), AgentValue::string("stale"));
        live.configs = Some(configs);
        let mut config_specs = live.config_specs.take().unwrap_or_default();
        config_specs.insert("c3".into(), AgentConfigSpec::default());
        live.config_specs = Some(config_specs);

        (def, live)
    }

    #[test]
    fn config_key_validation_accepts_live_generated_keys() {
        let (def, live) = numbered_def_and_live_spec();

        // Generated on the instance only: known through the live spec, from
        // its configs (c2) as well as its config_specs (c3).
        assert!(validate_config_keys(&def, Some(&live), ["c2".to_string()].iter()).is_ok());
        assert!(validate_config_keys(&def, Some(&live), ["c3".to_string()].iter()).is_ok());
        assert!(validate_config_keys(&def, Some(&live), ["n".to_string()].iter()).is_ok());
    }

    #[test]
    fn config_key_validation_lists_live_keys_for_unknown_key() {
        let (def, live) = numbered_def_and_live_spec();

        let err = validate_config_keys(&def, Some(&live), ["c7".to_string()].iter())
            .expect_err("unknown key must be rejected");
        assert!(err.contains("c2"), "live keys must be listed: {err}");
        assert!(err.contains("c3"), "live keys must be listed: {err}");
        assert!(!err.contains("_c9"), "stale keys must not be listed: {err}");
    }

    #[test]
    fn config_key_validation_rejects_global_key_with_live_spec() {
        let (def, live) = numbered_def_and_live_spec();

        let err = validate_config_keys(&def, Some(&live), ["api_key".to_string()].iter())
            .expect_err("global key must be rejected");
        assert!(err.contains("GLOBAL"));
    }

    #[test]
    fn config_key_validation_rejects_stale_underscore_key() {
        let (def, live) = numbered_def_and_live_spec();

        let err = validate_config_keys(&def, Some(&live), ["_c9".to_string()].iter())
            .expect_err("stale key must be rejected even though it is present");
        assert!(err.contains("reserved"), "{err}");
    }

    #[test]
    fn config_key_validation_falls_back_to_definition_without_live_spec() {
        let (def, live) = numbered_def_and_live_spec();

        let err = validate_config_keys(&def, None, ["c2".to_string()].iter())
            .expect_err("without a live spec only declared keys are valid");
        assert!(err.contains("c0, c1"), "{err}");

        // A spec carrying neither configs nor config_specs falls back too.
        let empty = AgentSpec {
            def_name: live.def_name.clone(),
            ..Default::default()
        };
        assert!(validate_config_keys(&def, Some(&empty), ["c2".to_string()].iter()).is_err());
        assert!(validate_config_keys(&def, Some(&empty), ["c0".to_string()].iter()).is_ok());
    }

    #[test]
    fn constant_time_eq_matches_exactly() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreT"));
        assert!(!constant_time_eq(b"secret", b"secret2"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }

    struct RawResponse {
        status: u16,
        head: String,
    }

    /// Sends a raw HTTP/1.1 initialize POST to /mcp and returns the status
    /// line and headers. Raw TCP keeps the test free of HTTP client deps.
    async fn initialize_request(port: u16, auth: Option<&str>) -> RawResponse {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut stream = tokio::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"0.0.0"}}}"#;
        let auth_line = auth
            .map(|a| format!("Authorization: {a}\r\n"))
            .unwrap_or_default();
        let request = format!(
            "POST /mcp HTTP/1.1\r\n\
             Host: 127.0.0.1:{port}\r\n\
             Content-Type: application/json\r\n\
             Accept: application/json, text/event-stream\r\n\
             {auth_line}Content-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).await.unwrap();

        // Read until the header block is complete; the body is irrelevant.
        let mut buf = Vec::new();
        let mut chunk = [0u8; 1024];
        while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
            let n =
                tokio::time::timeout(std::time::Duration::from_secs(5), stream.read(&mut chunk))
                    .await
                    .expect("timed out reading MCP server response")
                    .unwrap();
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        let head = String::from_utf8_lossy(&buf).to_lowercase();
        let status = head
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse().ok())
            .unwrap_or_else(|| panic!("malformed response: {head:?}"));
        RawResponse { status, head }
    }

    #[tokio::test]
    async fn bearer_middleware_guards_the_endpoint() {
        let ma = ModularAgent::init().unwrap();
        ma.ready().await.unwrap();

        // Reserve a free port; the window between drop and rebind is
        // acceptable for a test.
        let port = {
            let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
                .await
                .unwrap();
            listener.local_addr().unwrap().port()
        };

        let handle = start_mcp_server(
            ma.clone(),
            McpServerConfig {
                port,
                presets_dir: None,
                token: Some("test-token".into()),
            },
        )
        .await
        .unwrap();

        let response = initialize_request(port, None).await;
        assert_eq!(response.status, 401, "missing token must be rejected");
        assert!(
            response.head.contains("www-authenticate: bearer"),
            "401 must carry a WWW-Authenticate: Bearer challenge: {}",
            response.head
        );

        let response = initialize_request(port, Some("Bearer wrong-token")).await;
        assert_eq!(response.status, 401, "wrong token must be rejected");

        let response = initialize_request(port, Some("Bearer test-token")).await;
        assert_eq!(
            response.status, 200,
            "correct token must pass: {}",
            response.head
        );

        handle.stop().await;
        ma.quit();
    }

    #[test]
    fn event_ring_caps_and_pages() {
        let ring = EventRing::new();
        for i in 0..(RING_CAPACITY + 10) {
            ring.push_error(0, Some("p1".into()), format!("a{i}"), "boom".into());
        }
        assert_eq!(ring.errors.lock().unwrap().len(), RING_CAPACITY);

        // Oldest surviving record has seq 11; paging resumes after a cursor.
        let page = ring.collect_errors(None, 0, 5);
        assert_eq!(page.len(), 5);
        assert_eq!(page[0].seq, 11);
        assert_eq!(page[4].seq, 15);
        let next = ring.collect_errors(None, page[4].seq, 5);
        assert_eq!(next[0].seq, page[4].seq + 1);

        // Preset filter drops non-matching records.
        assert!(ring.collect_errors(Some("other"), 0, 10).is_empty());
    }

    /// Drives the tool handlers directly against a Switch-like agent whose
    /// live spec - not its definition - knows the generated config keys.
    mod tool_tests {
        use async_trait::async_trait;
        use modular_agent_macros::modular_agent;

        use super::*;
        use crate::agent::{AgentData, AsAgent};
        use crate::definition::{AgentConfigSpec, AgentConfigSpecs};
        use crate::output::AgentOutput;

        /// A condition value the double rejects AFTER committing it, the way
        /// Switch keeps an unparsable condition as never-matching.
        const INVALID_CONDITION: &str = "#err";

        /// Switch-like double: rebuilds `c0`..`c(n-1)` and the numbered
        /// output ports from `n` in new() and configs_changed().
        #[modular_agent(
            title = "MCP Numbered",
            category = "Test",
            inputs = ["in"],
            outputs = ["0", "1"],
            integer_config(name = "n", default = 2),
            string_config(name = "c0"),
            string_config(name = "c1"),
        )]
        struct McpNumberedAgent {
            data: AgentData,
        }

        fn rebuild_numbered(spec: &mut AgentSpec) -> Result<(), AgentError> {
            let n = spec
                .configs
                .as_ref()
                .map(|configs| configs.get_integer_or("n", 2))
                .unwrap_or(2)
                .clamp(1, 16) as usize;

            let mut configs = AgentConfigs::new();
            let mut config_specs = AgentConfigSpecs::default();
            configs.set("n".to_string(), AgentValue::integer(n as i64));
            if let Some(n_spec) = spec
                .config_specs
                .as_ref()
                .and_then(|specs| specs.get("n"))
                .cloned()
            {
                config_specs.insert("n".to_string(), n_spec);
            }
            for i in 0..n {
                let name = format!("c{}", i);
                let value = spec
                    .configs
                    .as_ref()
                    .map(|cfg| cfg.get_string_or(&name, ""))
                    .unwrap_or_default();
                configs.set(name.clone(), AgentValue::string(value));
                config_specs.insert(
                    name,
                    AgentConfigSpec {
                        value: AgentValue::string_default(),
                        type_: Some("string".to_string()),
                        ..Default::default()
                    },
                );
            }
            spec.configs = Some(configs);
            spec.config_specs = Some(config_specs);
            spec.outputs = Some((0..n).map(|i| i.to_string()).collect());

            // Reported after the commit above, like Switch keeping an
            // unparsable condition as never-matching.
            if let Some(configs) = &spec.configs {
                for i in 0..n {
                    if configs.get_string_or(&format!("c{}", i), "") == INVALID_CONDITION {
                        return Err(AgentError::InvalidConfig(format!(
                            "condition c{} is not parsable",
                            i
                        )));
                    }
                }
            }
            Ok(())
        }

        #[async_trait]
        impl AsAgent for McpNumberedAgent {
            fn new(ma: ModularAgent, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
                rebuild_numbered(&mut spec)?;
                Ok(Self {
                    data: AgentData::new(ma, id, spec),
                })
            }

            fn configs_changed(&mut self) -> Result<(), AgentError> {
                rebuild_numbered(&mut self.data.spec)?;
                self.emit_agent_spec_updated();
                Ok(())
            }
        }

        async fn setup() -> (ModularAgent, McpServer, String) {
            let ma = ModularAgent::init().expect("init");
            ma.ready().await.expect("ready");
            let preset_id = ma.new_preset().expect("preset");
            let server = McpServer::new(ma.clone(), None, Arc::new(EventRing::new()));
            (ma, server, preset_id)
        }

        fn result_text(result: &CallToolResult) -> &str {
            match &result.content[0] {
                ContentBlock::Text(text) => &text.text,
                other => panic!("expected text content, got {other:?}"),
            }
        }

        fn result_json(result: &CallToolResult) -> Value {
            serde_json::from_str(result_text(result)).expect("tool result must be JSON")
        }

        fn is_error(result: &CallToolResult) -> bool {
            result.is_error == Some(true)
        }

        async fn add_numbered(
            server: &McpServer,
            preset_id: &str,
            configs: serde_json::Map<String, Value>,
        ) -> Result<CallToolResult, McpError> {
            server
                .add_agent(Parameters(AddAgentParams {
                    preset_id: preset_id.to_string(),
                    def_name: McpNumberedAgent::DEF_NAME.to_string(),
                    configs: Some(configs),
                    x: None,
                    y: None,
                    width: None,
                    height: None,
                }))
                .await
        }

        fn json_map(value: Value) -> serde_json::Map<String, Value> {
            match value {
                Value::Object(map) => map,
                _ => unreachable!(),
            }
        }

        #[tokio::test]
        async fn add_agent_applies_deferred_generated_configs() {
            let (ma, server, preset_id) = setup().await;

            let result = add_numbered(
                &server,
                &preset_id,
                json_map(serde_json::json!({ "n": 3, "c2": "hello" })),
            )
            .await
            .unwrap();
            assert!(!is_error(&result), "{}", result_text(&result));

            // The returned spec reflects the deferred key and the ports that
            // n=3 generated, and carries no warning.
            let created = result_json(&result);
            assert_eq!(created["configs"]["c2"], "hello");
            let outputs = created["outputs"].as_array().unwrap();
            assert!(outputs.contains(&Value::String("2".into())));
            assert!(created.get("warning").is_none());

            ma.quit();
        }

        #[tokio::test]
        async fn add_agent_rolls_back_when_deferred_key_is_invalid() {
            let (ma, server, preset_id) = setup().await;

            let result = add_numbered(
                &server,
                &preset_id,
                json_map(serde_json::json!({ "n": 3, "c9": "x" })),
            )
            .await
            .unwrap();
            assert!(is_error(&result));
            let text = result_text(&result);
            assert!(text.contains("c9"), "{text}");
            assert!(text.contains("c2"), "valid keys must be listed: {text}");

            // The half-created agent must not survive the failure.
            let spec = ma.get_preset_spec(&preset_id).await.unwrap();
            assert!(spec.agents.is_empty(), "rollback must remove the agent");

            ma.quit();
        }

        #[tokio::test]
        async fn add_agent_keeps_agent_and_warns_when_configs_error_after_commit() {
            let (ma, server, preset_id) = setup().await;

            let result = add_numbered(
                &server,
                &preset_id,
                json_map(serde_json::json!({ "n": 3, "c2": INVALID_CONDITION })),
            )
            .await
            .unwrap();
            assert!(!is_error(&result), "{}", result_text(&result));

            // The agent committed the condition before reporting it, so it
            // stays, the value is kept and the error surfaces as a warning.
            let created = result_json(&result);
            assert_eq!(created["configs"]["c2"], INVALID_CONDITION);
            let warning = created["warning"].as_str().unwrap();
            assert!(warning.contains("c2"), "{warning}");

            ma.quit();
        }

        #[tokio::test]
        async fn set_agent_configs_tool_accepts_generated_key() {
            let (ma, server, preset_id) = setup().await;

            let result = add_numbered(&server, &preset_id, json_map(serde_json::json!({ "n": 3 })))
                .await
                .unwrap();
            let agent_id = result_json(&result)["id"].as_str().unwrap().to_string();

            let result = server
                .set_agent_configs(Parameters(SetAgentConfigsParams {
                    agent_id: agent_id.clone(),
                    configs: json_map(serde_json::json!({ "c2": "hello" })),
                }))
                .await
                .unwrap();
            assert!(!is_error(&result), "{}", result_text(&result));
            let spec = ma.get_agent_spec(&agent_id).await.unwrap();
            assert_eq!(
                spec.configs.unwrap().get_string_or("c2", ""),
                "hello",
                "the generated key must be settable through the tool"
            );

            let result = server
                .set_agent_configs(Parameters(SetAgentConfigsParams {
                    agent_id,
                    configs: json_map(serde_json::json!({ "c9": "x" })),
                }))
                .await
                .unwrap();
            assert!(is_error(&result));
            assert!(result_text(&result).contains("c2"), "live keys listed");

            ma.quit();
        }

        #[tokio::test]
        async fn update_agent_spec_tool_accepts_generated_key() {
            let (ma, server, preset_id) = setup().await;

            let result = add_numbered(&server, &preset_id, json_map(serde_json::json!({ "n": 3 })))
                .await
                .unwrap();
            let agent_id = result_json(&result)["id"].as_str().unwrap().to_string();

            let result = server
                .update_agent_spec(Parameters(UpdateAgentSpecParams {
                    agent_id: agent_id.clone(),
                    patch: json_map(serde_json::json!({ "configs": { "c2": "v" } })),
                }))
                .await
                .unwrap();
            assert!(!is_error(&result), "{}", result_text(&result));

            // The partial patch must merge: n and the other conditions
            // survive, so configs_changed keeps all three ports.
            let spec = ma.get_agent_spec(&agent_id).await.unwrap();
            let configs = spec.configs.unwrap();
            assert_eq!(configs.get_string_or("c2", ""), "v");
            assert_eq!(configs.get_integer_or("n", 0), 3);
            assert!(spec.outputs.unwrap().iter().any(|p| p == "2"));

            ma.quit();
        }

        #[tokio::test]
        async fn update_agent_spec_tool_reports_committed_config_error() {
            let (ma, server, preset_id) = setup().await;

            let result = add_numbered(&server, &preset_id, json_map(serde_json::json!({ "n": 3 })))
                .await
                .unwrap();
            let agent_id = result_json(&result)["id"].as_str().unwrap().to_string();

            let result = server
                .update_agent_spec(Parameters(UpdateAgentSpecParams {
                    agent_id: agent_id.clone(),
                    patch: json_map(serde_json::json!({ "configs": { "c2": INVALID_CONDITION } })),
                }))
                .await
                .unwrap();

            // configs_changed rejected the committed value: the error must
            // reach the caller while the spec keeps what the agent stored.
            assert!(is_error(&result));
            let spec = ma.get_agent_spec(&agent_id).await.unwrap();
            assert_eq!(
                spec.configs.unwrap().get_string_or("c2", ""),
                INVALID_CONDITION
            );

            ma.quit();
        }
    }
}
