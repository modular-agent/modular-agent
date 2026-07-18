//! Tool registry and agents for LLM function calling.
//!
//! This module provides infrastructure for registering, managing, and invoking tools
//! that can be called by LLMs. It includes:
//!
//! - A global tool registry for registering and looking up tools by name
//! - The `Tool` trait for implementing custom tools
//! - Agents for working with tools in workflows:
//!   - `ListToolsAgent` - Lists available tools matching a pattern
//!   - `PresetToolAgent` - Exposes a workflow as a callable tool
//!   - `CallToolMessageAgent` - Processes tool calls from LLM messages
//!   - `LoopControlAgent` - Guards tool-call cycles with an iteration limit
//!   - `CallToolAgent` - Directly invokes a tool by name
//! ```

#![cfg(feature = "llm")]

use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::Duration,
};

use crate::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentStatus, AgentValue,
    AsAgent, Message, ModularAgent, SharedAgent, ToolCall, async_trait, modular_agent,
};
use im::{Vector, vector};
use regex::RegexSet;
use tokio::sync::oneshot;

const CATEGORY: &str = "Core/Tool";

const PORT_LIMIT_EXCEEDED: &str = "limit_exceeded";
const PORT_MESSAGE: &str = "message";
const PORT_PATTERNS: &str = "patterns";
const PORT_TOOLS: &str = "tools";
const PORT_TOOL_CALL: &str = "tool_call";
const PORT_TOOL_IN: &str = "tool_in";
const PORT_TOOL_OUT: &str = "tool_out";
const PORT_VALUE: &str = "value";

const CONFIG_TOOLS: &str = "tools";
const CONFIG_TOOL_NAME: &str = "name";
const CONFIG_TOOL_DESCRIPTION: &str = "description";
const CONFIG_TOOL_PARAMETERS: &str = "parameters";
const CONFIG_TIMEOUT_SECS: &str = "timeout_secs";
const CONFIG_MAX_ITERATIONS: &str = "max_iterations";

/// Fallback timeout (seconds) when `timeout_secs` config is unset or unreadable.
const DEFAULT_TIMEOUT_SECS: i64 = 60;

/// Fallback loop limit when `max_iterations` config is unset or unreadable.
const DEFAULT_MAX_ITERATIONS: i64 = 25;

/// Metadata describing a tool available for LLM function calling.
///
/// This information is typically sent to the LLM to describe what tools
/// are available and how to call them.
#[derive(Clone, Debug)]
pub struct ToolInfo {
    /// Unique name identifying the tool.
    pub name: String,

    /// Human-readable description of what the tool does.
    pub description: String,

    /// JSON Schema describing the tool's parameters.
    ///
    /// Defaults to `{"type": "object", "properties": {}}` when no schema
    /// is provided (see [`ToolInfo::new`]).
    pub parameters: serde_json::Value,
}

impl ToolInfo {
    /// Creates a new `ToolInfo`.
    ///
    /// When `parameters` is `None`, the empty-object JSON Schema
    /// `{"type": "object", "properties": {}}` is used so providers always
    /// receive a valid schema.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Option<serde_json::Value>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: parameters
                .unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}})),
        }
    }
}

/// Trait for implementing callable tools.
///
/// Tools are functions that can be invoked by LLMs during conversations.
/// Implement this trait to create custom tools that can be registered
/// with the global tool registry.
///
/// # Example
///
/// ```ignore
/// use modular_agent_core::{Tool, ToolInfo, AgentContext, AgentValue, AgentError, async_trait};
///
/// struct MyTool {
///     info: ToolInfo,
/// }
///
/// #[async_trait]
/// impl Tool for MyTool {
///     fn info(&self) -> &ToolInfo {
///         &self.info
///     }
///
///     async fn call(&self, ctx: AgentContext, args: AgentValue) -> Result<AgentValue, AgentError> {
///         // Tool implementation
///         Ok(AgentValue::string("result"))
///     }
/// }
/// ```
#[async_trait]
pub trait Tool {
    /// Returns metadata about this tool.
    fn info(&self) -> &ToolInfo;

    /// Invokes the tool with the given context and arguments.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The agent context for this invocation
    /// * `args` - Arguments passed to the tool (typically from LLM)
    ///
    /// # Returns
    ///
    /// The tool's result as an `AgentValue`, or an error if the call fails.
    async fn call(&self, ctx: AgentContext, args: AgentValue) -> Result<AgentValue, AgentError>;
}

impl From<ToolInfo> for AgentValue {
    fn from(info: ToolInfo) -> Self {
        let mut obj: BTreeMap<String, AgentValue> = BTreeMap::new();
        obj.insert("name".to_string(), AgentValue::from(info.name));
        obj.insert(
            "description".to_string(),
            AgentValue::from(info.description),
        );
        if let Ok(params_value) = AgentValue::from_serialize(&info.parameters) {
            obj.insert("parameters".to_string(), params_value);
        }
        AgentValue::object(obj.into())
    }
}

/// Internal entry for a registered tool.
#[derive(Clone)]
struct ToolEntry {
    info: ToolInfo,
    tool: Arc<Box<dyn Tool + Send + Sync>>,
}

impl ToolEntry {
    /// Creates a new tool entry from a tool implementation.
    fn new<T: Tool + Send + Sync + 'static>(tool: T) -> Self {
        Self {
            info: tool.info().clone(),
            tool: Arc::new(Box::new(tool)),
        }
    }
}

/// Thread-safe registry for managing tools.
struct ToolRegistry {
    tools: HashMap<String, ToolEntry>,
}

impl ToolRegistry {
    /// Creates a new empty tool registry.
    fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Registers a tool with the registry.
    fn register_tool<T: Tool + Send + Sync + 'static>(&mut self, tool: T) {
        let name = tool.info().name.to_string();
        let entry = ToolEntry::new(tool);
        self.tools.insert(name, entry);
    }

    /// Removes a tool from the registry by name.
    fn unregister_tool(&mut self, name: &str) {
        self.tools.remove(name);
    }

    /// Retrieves a tool by name, if it exists.
    fn get_tool(&self, name: &str) -> Option<Arc<Box<dyn Tool + Send + Sync>>> {
        self.tools.get(name).map(|entry| entry.tool.clone())
    }
}

/// Global tool registry instance.
static TOOL_REGISTRY: OnceLock<RwLock<ToolRegistry>> = OnceLock::new();

/// Returns the global tool registry, initializing it if necessary.
fn registry() -> &'static RwLock<ToolRegistry> {
    TOOL_REGISTRY.get_or_init(|| RwLock::new(ToolRegistry::new()))
}

/// Registers a tool with the global registry.
///
/// The tool will be available for lookup and invocation by its name.
/// If a tool with the same name already exists, it will be replaced.
///
/// # Arguments
///
/// * `tool` - The tool implementation to register
pub fn register_tool<T: Tool + Send + Sync + 'static>(tool: T) {
    registry().write().unwrap().register_tool(tool);
}

/// Returns whether a tool name satisfies the `^[a-zA-Z0-9_-]{1,64}$` pattern
/// required by the Claude and OpenAI APIs.
fn is_valid_tool_name(name: &str) -> bool {
    let len = name.len();
    (1..=64).contains(&len)
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Removes a tool from the global registry by name.
///
/// # Arguments
///
/// * `name` - The name of the tool to unregister
pub fn unregister_tool(name: &str) {
    registry().write().unwrap().unregister_tool(name);
}

/// Returns information about all registered tools.
///
/// # Returns
///
/// A vector of `ToolInfo` for all currently registered tools.
pub fn list_tool_infos() -> Vec<ToolInfo> {
    registry()
        .read()
        .unwrap()
        .tools
        .values()
        .map(|entry| entry.info.clone())
        .collect()
}

/// Returns tool information for tools matching the given regex patterns.
///
/// Patterns are newline-separated regular expressions. A tool is included
/// if its name matches any of the patterns.
///
/// # Arguments
///
/// * `patterns` - Newline-separated regex patterns to match tool names
///
/// # Returns
///
/// A vector of `ToolInfo` for tools whose names match the patterns.
///
/// # Errors
///
/// Returns an error if any of the patterns are invalid regular expressions.
pub fn list_tool_infos_patterns(patterns: &str) -> Result<Vec<ToolInfo>, regex::Error> {
    // Split patterns by newline and trim whitespace
    let patterns = patterns
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<&str>>();
    let reg_set = RegexSet::new(&patterns)?;
    let tool_names = registry()
        .read()
        .unwrap()
        .tools
        .values()
        .filter_map(|entry| {
            if reg_set.is_match(&entry.info.name) {
                Some(entry.info.clone())
            } else {
                None
            }
        })
        .collect();
    Ok(tool_names)
}

/// Retrieves a tool by name from the global registry.
///
/// # Arguments
///
/// * `name` - The name of the tool to retrieve
///
/// # Returns
///
/// The tool if found, or `None` if no tool with that name is registered.
pub fn get_tool(name: &str) -> Option<Arc<Box<dyn Tool + Send + Sync>>> {
    registry().read().unwrap().get_tool(name)
}

/// Invokes a tool by name with the given arguments.
///
/// # Arguments
///
/// * `ctx` - The agent context for the invocation
/// * `name` - The name of the tool to call
/// * `args` - Arguments to pass to the tool
///
/// # Returns
///
/// The tool's result, or an error if the tool is not found or fails.
pub async fn call_tool(
    ctx: AgentContext,
    name: &str,
    args: AgentValue,
) -> Result<AgentValue, AgentError> {
    let tool = {
        let guard = registry().read().unwrap();
        guard.get_tool(name)
    };

    let Some(tool) = tool else {
        return Err(AgentError::Other(format!("Tool '{}' not found", name)));
    };

    tool.call(ctx, args).await
}

/// Builds an error tool-result message for a failed tool call.
///
/// The message carries `is_error: Some(true)` so LLM clients can report the
/// failure back to the model (Claude's `tool_result` `is_error`) instead of
/// aborting the whole flow. This is the designated return path for parse,
/// validation, and execution failures of tool calls.
pub fn error_tool_result(call: &ToolCall, e: impl ToString) -> Message {
    let mut msg = Message::tool(call.function.name.clone(), e.to_string());
    msg.id = call.function.id.clone();
    msg.is_error = Some(true);
    msg
}

/// Executes multiple tool calls and returns the results as messages.
///
/// Processes each tool call sequentially and returns tool response messages
/// suitable for continuing an LLM conversation. A failure of one call
/// (unparseable parameters or a tool error) does not abort the others: it is
/// returned as a tool message with `is_error: Some(true)` so the LLM can
/// recover.
///
/// # Arguments
///
/// * `ctx` - The agent context for the invocations
/// * `tool_calls` - The tool calls to execute
///
/// # Returns
///
/// A vector of tool response messages, one for each tool call.
pub async fn call_tools(
    ctx: &AgentContext,
    tool_calls: &Vector<ToolCall>,
) -> Result<Vector<Message>, AgentError> {
    if tool_calls.is_empty() {
        return Ok(vector![]);
    };
    let mut resp_messages = vec![];

    for call in tool_calls {
        // A provider argument string that failed to parse even after repair is
        // reported back to the model rather than executed with bogus arguments.
        if let Some(err) = &call.function.parse_error {
            resp_messages.push(error_tool_result(
                call,
                format!(
                    "Tool call arguments could not be parsed as JSON; the call was \
                     not executed. Re-issue the call with valid JSON arguments. {}",
                    err
                ),
            ));
            continue;
        }
        let args = match AgentValue::from_json(call.function.parameters.clone()) {
            Ok(args) => args,
            Err(e) => {
                resp_messages.push(error_tool_result(
                    call,
                    format!("Failed to parse tool call parameters: {}", e),
                ));
                continue;
            }
        };
        let tool_resp = match call_tool(ctx.clone(), call.function.name.as_str(), args).await {
            Ok(resp) => resp,
            Err(e) => {
                resp_messages.push(error_tool_result(call, e));
                continue;
            }
        };
        let mut msg = Message::tool(call.function.name.clone(), tool_resp.to_json().to_string());
        msg.id = call.function.id.clone();
        resp_messages.push(msg);
    }

    Ok(resp_messages.into())
}

// ============================================================================
// Tool Agents
// ============================================================================

/// Agent that lists available tools.
///
/// Outputs tool information for all registered tools, optionally filtered
/// by regex patterns provided on the input port.
///
/// # Inputs
///
/// * `patterns` - Optional regex patterns (newline-separated) to filter tools
///
/// # Outputs
///
/// * `tools` - Array of tool information objects
#[modular_agent(
    title="List Tools",
    category=CATEGORY,
    inputs=[PORT_PATTERNS],
    outputs=[PORT_TOOLS],
)]
pub struct ListToolsAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ListToolsAgent {
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
        let Some(patterns) = value.as_str() else {
            return Err(AgentError::InvalidValue(
                "patterns input must be a string".to_string(),
            ));
        };

        let tools = if !patterns.is_empty() {
            list_tool_infos_patterns(patterns)
                .map_err(|e| AgentError::InvalidValue(format!("Invalid regex patterns: {}", e)))?
        } else {
            list_tool_infos()
        };
        let tools = tools
            .into_iter()
            .map(|tool| tool.into())
            .collect::<Vector<AgentValue>>();
        let tools_array = AgentValue::array(tools);

        self.output(ctx, PORT_TOOLS, tools_array).await?;

        Ok(())
    }
}

/// Agent that exposes a workflow as a callable tool.
///
/// This agent registers itself as a tool that can be invoked by LLMs.
/// When called, it forwards the arguments to the `tool_in` output port
/// and waits for a response on the `tool_out` input port.
///
/// # Configuration
///
/// * `name` - The tool name (defaults to agent definition name)
/// * `description` - Human-readable description of the tool
/// * `parameters` - JSON Schema describing the tool's parameters
/// * `timeout_secs` - Seconds to wait for the workflow's result before timing
///   out (default: 60). `0` waits indefinitely. On timeout the caller receives
///   a tool result with `is_error: true` so the LLM can recover.
///
/// # Ports
///
/// * Input `tool_out` - Receives the tool's result from the workflow
/// * Output `tool_in` - Emits the tool call arguments to the workflow
#[modular_agent(
    title="Preset Tool",
    category=CATEGORY,
    inputs=[PORT_TOOL_OUT],
    outputs=[PORT_TOOL_IN],
    string_config(name=CONFIG_TOOL_NAME),
    text_config(name=CONFIG_TOOL_DESCRIPTION),
    object_config(name=CONFIG_TOOL_PARAMETERS),
    integer_config(name=CONFIG_TIMEOUT_SECS, default=60),
)]
pub struct PresetToolAgent {
    data: AgentData,
    name: String,
    description: String,
    parameters: Option<serde_json::Value>,
    /// Pending tool calls awaiting results, keyed by context ID.
    pending: Arc<Mutex<HashMap<usize, oneshot::Sender<AgentValue>>>>,
}

impl PresetToolAgent {
    /// Initiates a tool call and returns a receiver for the result.
    ///
    /// Emits the arguments to the workflow and registers a pending receiver
    /// that will be fulfilled when the result arrives on the input port.
    fn start_tool_call(
        &mut self,
        ctx: AgentContext,
        args: AgentValue,
    ) -> Result<oneshot::Receiver<AgentValue>, AgentError> {
        let (tx, rx) = oneshot::channel();

        self.pending.lock().unwrap().insert(ctx.id(), tx);
        if let Err(e) = self.try_output(ctx.clone(), PORT_TOOL_IN, args) {
            // Nothing was emitted, so no result can ever arrive; drop the entry
            // now or it would linger until stop() and could swallow the result
            // of a later call reusing this context id.
            self.pending.lock().unwrap().remove(&ctx.id());
            return Err(e);
        }

        Ok(rx)
    }
}

#[async_trait]
impl AsAgent for PresetToolAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let def_name = spec.def_name.clone();
        let configs = spec.configs.clone();
        let name = configs
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_TOOL_NAME).ok())
            .unwrap_or_else(|| def_name.clone());
        let description = configs
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_TOOL_DESCRIPTION).ok())
            .unwrap_or_default();
        let parameters = configs
            .as_ref()
            .and_then(|c| c.get(CONFIG_TOOL_PARAMETERS).ok())
            .and_then(|v| serde_json::to_value(v).ok());
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            name,
            description,
            parameters,
            pending: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let old_name = self.name.clone();
        self.name = self.configs()?.get_string_or_default(CONFIG_TOOL_NAME);
        self.description = self
            .configs()?
            .get_string_or_default(CONFIG_TOOL_DESCRIPTION);
        self.parameters = self
            .configs()?
            .get(CONFIG_TOOL_PARAMETERS)
            .ok()
            .and_then(|v| serde_json::to_value(v).ok());

        // Refresh the registration only while running; otherwise start() will
        // register the tool with the new values later.
        if self.data.status == AgentStatus::Start {
            if !is_valid_tool_name(&self.name) {
                log::warn!(
                    "PresetToolAgent {} has invalid tool name {:?}; \
                     tool names must match ^[a-zA-Z0-9_-]{{1,64}}$",
                    self.id(),
                    self.name
                );
            }
            let agent_handle = self
                .ma()
                .get_agent(self.id())
                .ok_or_else(|| AgentError::AgentNotFound(self.id().to_string()))?;
            let tool = PresetTool::new(
                self.name.clone(),
                self.description.clone(),
                self.parameters.clone(),
                agent_handle,
            );
            // Register first: for an in-place refresh this overwrites the entry
            // atomically, so concurrent lookups never hit a missing tool. The
            // registry is name-keyed and process-global, so on rename the old
            // name must still be removed explicitly or it would leak a stale
            // entry that stop() (which unregisters the new name) never cleans up.
            register_tool(tool);
            if old_name != self.name {
                unregister_tool(&old_name);
            }
        }

        Ok(())
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        // Claude and OpenAI both require tool names to match ^[a-zA-Z0-9_-]{1,64}$;
        // an invalid name only fails later at API-call time, so surface it early.
        if !is_valid_tool_name(&self.name) {
            log::warn!(
                "PresetToolAgent {} has invalid tool name {:?}; \
                 tool names must match ^[a-zA-Z0-9_-]{{1,64}}$",
                self.id(),
                self.name
            );
        }
        let agent_handle = self
            .ma()
            .get_agent(self.id())
            .ok_or_else(|| AgentError::AgentNotFound(self.id().to_string()))?;
        let tool = PresetTool::new(
            self.name.clone(),
            self.description.clone(),
            self.parameters.clone(),
            agent_handle,
        );
        register_tool(tool);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        unregister_tool(&self.name);
        self.pending.lock().unwrap().clear();
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if let Some(tx) = self.pending.lock().unwrap().remove(&ctx.id()) {
            let _ = tx.send(value);
        }
        Ok(())
    }
}

/// Internal Tool implementation that delegates to a PresetToolAgent.
struct PresetTool {
    info: ToolInfo,
    agent: SharedAgent,
}

impl PresetTool {
    /// Creates a new PresetTool wrapping a PresetToolAgent.
    fn new(
        name: String,
        description: String,
        parameters: Option<serde_json::Value>,
        agent: SharedAgent,
    ) -> Self {
        Self {
            info: ToolInfo::new(name, description, parameters),
            agent,
        }
    }

    /// Executes a tool call through the wrapped agent.
    ///
    /// Waits up to the agent's `timeout_secs` config (default 60) for a result;
    /// `0` waits indefinitely. The timeout is read at call time so runtime config
    /// changes take effect. On timeout an `AgentError::Timeout` is returned, which
    /// the LLM tool-call path (`call_tools`) turns into an `is_error` tool result.
    async fn tool_call(
        &self,
        ctx: AgentContext,
        args: AgentValue,
    ) -> Result<AgentValue, AgentError> {
        // Kick off the tool call while holding the lock, then drop it before awaiting the result
        let ctx_id = ctx.id();
        let (rx, timeout_secs, pending) = {
            let mut guard = self.agent.lock().await;
            let Some(preset_tool_agent) = guard.as_agent_mut::<PresetToolAgent>() else {
                return Err(AgentError::Other(
                    "Agent is not PresetToolAgent".to_string(),
                ));
            };
            let timeout_secs = preset_tool_agent
                .configs()
                .map(|c| c.get_integer_or(CONFIG_TIMEOUT_SECS, DEFAULT_TIMEOUT_SECS))
                .unwrap_or(DEFAULT_TIMEOUT_SECS);
            let pending = preset_tool_agent.pending.clone();
            let rx = preset_tool_agent.start_tool_call(ctx, args)?;
            (rx, timeout_secs, pending)
        };

        if timeout_secs <= 0 {
            return rx
                .await
                .map_err(|_| AgentError::Other("tool_out dropped".to_string()));
        }

        match tokio::time::timeout(Duration::from_secs(timeout_secs as u64), rx).await {
            Ok(result) => result.map_err(|_| AgentError::Other("tool_out dropped".to_string())),
            Err(_) => {
                // The pending map is keyed by context id, which is shared by every
                // tool call in the same flow (including an LLM retry after this
                // error). Drop the stale sender now, otherwise a late tool_out from
                // this timed-out invocation would be delivered to whichever call
                // registers under the same id next. Tool calls within a flow run
                // sequentially (call_tools awaits each call), so this cannot remove
                // a newer call's sender.
                pending.lock().unwrap().remove(&ctx_id);
                Err(AgentError::Timeout(format!(
                    "Tool call timed out after {} seconds",
                    timeout_secs
                )))
            }
        }
    }
}

#[async_trait]
impl Tool for PresetTool {
    fn info(&self) -> &ToolInfo {
        &self.info
    }

    async fn call(&self, ctx: AgentContext, args: AgentValue) -> Result<AgentValue, AgentError> {
        self.tool_call(ctx, args).await
    }
}

/// Agent that processes tool calls from LLM messages.
///
/// When an LLM response contains tool calls, this agent executes them
/// and outputs the results as tool response messages.
///
/// # Configuration
///
/// * `tools` - Optional regex patterns to filter which tools can be called
///
/// # Ports
///
/// * Input `message` - LLM message that may contain tool calls
/// * Output `message` - Tool response messages (one per tool call)
#[modular_agent(
    title="Call Tool Message",
    category=CATEGORY,
    inputs=[PORT_MESSAGE],
    outputs=[PORT_MESSAGE],
    string_config(name=CONFIG_TOOLS),
)]
pub struct CallToolMessageAgent {
    data: AgentData,
    /// Tool-call ids already executed, keyed by ctx_key, guarding against a
    /// streaming turn re-delivering the same final message (e.g. Claude emits
    /// identical tool_calls on both ContentBlockStop and MessageStop).
    executed: BTreeMap<String, HashSet<String>>,
    /// Insertion order of ctx_keys, enabling oldest-first eviction once capped.
    ctx_key_order: VecDeque<String>,
}

/// Upper bound on tracked ctx_keys; the oldest entry is evicted when exceeded.
const MAX_TRACKED_CTX_KEYS: usize = 1024;

impl CallToolMessageAgent {
    fn is_executed(&self, ctx_key: &str, id: &str) -> bool {
        self.executed
            .get(ctx_key)
            .is_some_and(|ids| ids.contains(id))
    }

    fn mark_executed(&mut self, ctx_key: &str, id: String) {
        if !self.executed.contains_key(ctx_key) {
            if self.ctx_key_order.len() >= MAX_TRACKED_CTX_KEYS
                && let Some(oldest) = self.ctx_key_order.pop_front()
            {
                self.executed.remove(&oldest);
            }
            self.ctx_key_order.push_back(ctx_key.to_string());
        }
        self.executed
            .entry(ctx_key.to_string())
            .or_default()
            .insert(id);
    }
}

#[async_trait]
impl AsAgent for CallToolMessageAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            executed: BTreeMap::new(),
            ctx_key_order: VecDeque::new(),
        })
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        self.executed.clear();
        self.ctx_key_order.clear();
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let Some(message) = value.as_message() else {
            return Ok(());
        };
        // Partial streaming messages carry accumulated tool_calls but are not final;
        // only the streaming=false message for the turn may trigger execution.
        if message.streaming {
            return Ok(());
        }
        let Some(mut tool_calls) = message.tool_calls.clone() else {
            return Ok(());
        };

        // Filter tools
        let config_tools = self.configs()?.get_string_or_default(CONFIG_TOOLS);
        if !config_tools.is_empty() {
            let tools = list_tool_infos_patterns(&config_tools)
                .map_err(|e| AgentError::InvalidValue(format!("Invalid regex patterns: {}", e)))?;
            // FIXME: cache allowed tool names
            let allowed_tool_names: HashSet<String> = tools.into_iter().map(|t| t.name).collect();
            tool_calls = tool_calls
                .iter()
                .filter(|call| allowed_tool_names.contains(&call.function.name))
                .cloned()
                .collect();
        }

        // Defensive dedup: skip calls whose id was already executed for this flow.
        // Calls without an id keep legacy behavior and always execute.
        let ctx_key = ctx.ctx_key()?;
        tool_calls = tool_calls
            .iter()
            .filter(|call| match &call.function.id {
                Some(id) => !self.is_executed(&ctx_key, id),
                None => true,
            })
            .cloned()
            .collect();

        // Record ids before executing: for side-effecting tools (Slack posts, DB
        // writes) skipping a retry is safer than double execution.
        for call in &tool_calls {
            if let Some(id) = &call.function.id {
                self.mark_executed(&ctx_key, id.clone());
            }
        }

        // A response truncated by the token limit may carry incomplete tool-call
        // arguments; report that back to the model instead of executing them.
        // Placed after the dedup bookkeeping so a re-delivered final message does
        // not emit duplicate synthetic results.
        if message.stop_reason.as_deref() == Some("length") {
            for call in &tool_calls {
                let resp_msg = error_tool_result(
                    call,
                    format!(
                        "Tool call \"{}\" was not executed: output hit the token \
                         limit; arguments may be truncated. Re-issue with complete \
                         arguments.",
                        call.function.name
                    ),
                );
                self.output(ctx.clone(), PORT_MESSAGE, AgentValue::message(resp_msg))
                    .await?;
            }
            return Ok(());
        }

        let resp_messages = call_tools(&ctx, &tool_calls).await?;
        for resp_msg in resp_messages {
            self.output(ctx.clone(), PORT_MESSAGE, AgentValue::message(resp_msg))
                .await?;
        }
        Ok(())
    }
}

/// Agent that guards LLM tool-call cycles against runaway iteration.
///
/// Insert this agent between a chat agent's message output and the
/// tool-execution node. It forwards traffic transparently while counting,
/// per flow, the final assistant messages that request tool calls. Once the
/// count would exceed `max_iterations`, the triggering message is not
/// forwarded — severing the cycle — and a synthesized assistant message
/// explaining the stop is emitted on `limit_exceeded` instead.
///
/// A message is counted only when all of the following hold: role is
/// "assistant", `tool_calls` is present and non-empty, `streaming` is false,
/// and its id differs from the last counted id for the flow. Streaming turns
/// re-deliver the same tool_calls-bearing message under one id (partials plus
/// a possibly duplicated final), so counting every delivery would exhaust the
/// limit within a few turns. Messages without an id cannot be deduplicated
/// and are always counted. Everything else — non-message values, user / tool /
/// system messages, streaming partials, and assistant messages without tool
/// calls — passes through unchanged.
///
/// Setting `max_iterations` to zero or a negative value disables the limit.
/// Counters are kept per flow (`ctx_key`), capped at 1024 flows with
/// oldest-first eviction, and cleared when the agent stops.
///
/// # Configuration
///
/// * `max_iterations` - Maximum tool-call iterations per flow; `<= 0` disables the limit (default: 25)
///
/// # Ports
///
/// * Input `message` - Messages flowing through the tool-call cycle
/// * Output `message` - The forwarded input while within the limit
/// * Output `limit_exceeded` - Synthesized assistant message emitted when the limit is exceeded
#[modular_agent(
    title="Loop Control",
    category=CATEGORY,
    inputs=[PORT_MESSAGE],
    outputs=[PORT_MESSAGE, PORT_LIMIT_EXCEEDED],
    integer_config(name=CONFIG_MAX_ITERATIONS, default=25),
)]
pub struct LoopControlAgent {
    data: AgentData,
    /// Iteration count and last counted message id, keyed by ctx_key. The id
    /// guards against a streaming turn re-delivering the same final message.
    counts: BTreeMap<String, (u32, Option<String>)>,
    /// Insertion order of ctx_keys, enabling oldest-first eviction once capped.
    ctx_key_order: VecDeque<String>,
}

impl LoopControlAgent {
    fn record_count(&mut self, ctx_key: &str, count: u32, id: Option<String>) {
        if !self.counts.contains_key(ctx_key) {
            if self.ctx_key_order.len() >= MAX_TRACKED_CTX_KEYS
                && let Some(oldest) = self.ctx_key_order.pop_front()
            {
                self.counts.remove(&oldest);
            }
            self.ctx_key_order.push_back(ctx_key.to_string());
        }
        self.counts.insert(ctx_key.to_string(), (count, id));
    }
}

#[async_trait]
impl AsAgent for LoopControlAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            counts: BTreeMap::new(),
            ctx_key_order: VecDeque::new(),
        })
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        self.counts.clear();
        self.ctx_key_order.clear();
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let countable = value.as_message().is_some_and(|m| {
            m.role == "assistant"
                && !m.streaming
                && m.tool_calls.as_ref().is_some_and(|calls| !calls.is_empty())
        });
        if !countable {
            // The node must stay transparent for everything it does not count:
            // non-message values, user / tool / system messages, streaming
            // partials, and assistant messages without tool calls.
            return self.output(ctx, PORT_MESSAGE, value).await;
        }
        let message_id = value.as_message().and_then(|m| m.id.clone());

        let max_iterations = self
            .configs()?
            .get_integer_or(CONFIG_MAX_ITERATIONS, DEFAULT_MAX_ITERATIONS);
        let ctx_key = ctx.ctx_key()?;
        let (count, last_counted_id) = self.counts.get(&ctx_key).cloned().unwrap_or((0, None));

        // Re-delivery of the last counted message (e.g. Claude emits the same
        // final message on both ContentBlockStop and MessageStop). Never
        // re-count it, and forward it only if it was forwarded the first time,
        // so a duplicate of the blocked message neither re-opens the cycle nor
        // emits limit_exceeded twice.
        if let Some(id) = &message_id
            && last_counted_id.as_deref() == Some(id)
        {
            if max_iterations > 0 && i64::from(count) > max_iterations {
                return Ok(());
            }
            return self.output(ctx, PORT_MESSAGE, value).await;
        }

        let count = count.saturating_add(1);
        self.record_count(&ctx_key, count, message_id);

        if max_iterations > 0 && i64::from(count) > max_iterations {
            let notice = Message::assistant(format!(
                "Loop limit reached: the tool-call cycle exceeded the configured max_iterations of {} and has been stopped.",
                max_iterations
            ));
            return self
                .output(ctx, PORT_LIMIT_EXCEEDED, AgentValue::message(notice))
                .await;
        }

        self.output(ctx, PORT_MESSAGE, value).await
    }
}

/// Agent that directly invokes a tool by name.
///
/// Takes a tool call specification (name and parameters) and invokes
/// the corresponding registered tool, outputting the result.
///
/// # Ports
///
/// * Input `tool_call` - Object with `name` (string) and optional `parameters`
/// * Output `value` - The tool's return value
#[modular_agent(
    title="Call Tool",
    category=CATEGORY,
    inputs=[PORT_TOOL_CALL],
    outputs=[PORT_VALUE],
)]
pub struct CallToolAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for CallToolAgent {
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
        let obj = value.as_object().ok_or_else(|| {
            AgentError::InvalidValue("tool_call input must be an object".to_string())
        })?;
        let tool_name = obj.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
            AgentError::InvalidValue("tool_call.name must be a string".to_string())
        })?;
        let tool_parameters = obj.get("parameters").cloned().unwrap_or(AgentValue::unit());

        let resp = call_tool(ctx.clone(), tool_name, tool_parameters).await?;
        self.output(ctx, PORT_VALUE, resp).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolCallFunction;

    #[test]
    fn test_tool_info_new_parameters_default() {
        let info = ToolInfo::new("t", "d", None);
        assert_eq!(
            info.parameters,
            serde_json::json!({"type": "object", "properties": {}})
        );

        let schema = serde_json::json!({
            "type": "object",
            "properties": {"x": {"type": "string"}},
            "required": ["x"]
        });
        let info = ToolInfo::new("t", "d", Some(schema.clone()));
        assert_eq!(info.parameters, schema);
    }

    #[test]
    fn test_error_tool_result_shape() {
        let call = ToolCall {
            function: ToolCallFunction {
                name: "my_tool".to_string(),
                parameters: serde_json::json!({}),
                id: Some("call42".to_string()),
                parse_error: None,
            },
        };
        let msg = error_tool_result(&call, "something went wrong");

        assert_eq!(msg.role, "tool");
        assert_eq!(msg.tool_name.as_deref(), Some("my_tool"));
        assert_eq!(msg.id.as_deref(), Some("call42"));
        assert_eq!(msg.is_error, Some(true));
        assert_eq!(msg.content, "something went wrong");
    }

    #[test]
    fn test_is_valid_tool_name_accepts_valid() {
        assert!(is_valid_tool_name("a"));
        assert!(is_valid_tool_name("my_tool"));
        assert!(is_valid_tool_name("my-tool"));
        assert!(is_valid_tool_name("Tool_123-ABC"));
        assert!(is_valid_tool_name(&"x".repeat(64)));
    }

    #[test]
    fn test_preset_tool_timeout_config_default() {
        let def = PresetToolAgent::agent_definition();
        let specs = def
            .configs
            .as_ref()
            .expect("PresetToolAgent should have config specs");
        let spec = specs
            .get(CONFIG_TIMEOUT_SECS)
            .expect("timeout_secs config should be present");
        assert_eq!(spec.value, AgentValue::integer(DEFAULT_TIMEOUT_SECS));
    }

    #[test]
    fn test_is_valid_tool_name_rejects_invalid() {
        assert!(!is_valid_tool_name(""));
        assert!(!is_valid_tool_name(&"x".repeat(65)));
        assert!(!is_valid_tool_name("my tool"));
        assert!(!is_valid_tool_name("my.tool"));
        assert!(!is_valid_tool_name("ツール"));
        assert!(!is_valid_tool_name("tool@1"));
    }

    #[cfg(feature = "test-utils")]
    mod loop_control {
        use super::*;
        use crate::test_utils::{ProbeReceiver, probe_receiver};
        use crate::{AgentContext, ConnectionSpec, SharedAgent};

        const LOOP_DEF: &str = "modular_agent_core::tool::LoopControlAgent";
        const PROBE_DEF: &str = "modular_agent_core::test_utils::TestProbeAgent";
        const PROBE_PORT: &str = "value";

        struct Fixture {
            ma: ModularAgent,
            loop_agent: SharedAgent,
            forwarded: ProbeReceiver,
            limit: ProbeReceiver,
        }

        /// Builds a running preset: LoopControlAgent with its `message` and
        /// `limit_exceeded` outputs each wired to a TestProbeAgent.
        async fn setup(max_iterations: i64) -> Fixture {
            let ma = ModularAgent::init().unwrap();
            ma.ready().await.unwrap();
            let preset_id = ma.new_preset().unwrap();

            let loop_def = ma.get_agent_definition(LOOP_DEF).unwrap();
            let loop_id = ma
                .add_agent(preset_id.clone(), loop_def.to_spec())
                .await
                .unwrap();

            let probe_def = ma.get_agent_definition(PROBE_DEF).unwrap();
            let fwd_id = ma
                .add_agent(preset_id.clone(), probe_def.to_spec())
                .await
                .unwrap();
            let lim_id = ma
                .add_agent(preset_id.clone(), probe_def.to_spec())
                .await
                .unwrap();

            for (source_handle, target) in [
                (PORT_MESSAGE, fwd_id.clone()),
                (PORT_LIMIT_EXCEEDED, lim_id.clone()),
            ] {
                ma.add_connection(
                    &preset_id,
                    ConnectionSpec {
                        source: loop_id.clone(),
                        source_handle: source_handle.to_string(),
                        target,
                        target_handle: PROBE_PORT.to_string(),
                    },
                )
                .await
                .unwrap();
            }

            let loop_agent = ma.get_agent(&loop_id).unwrap();
            loop_agent
                .lock()
                .await
                .set_config(
                    CONFIG_MAX_ITERATIONS.into(),
                    AgentValue::integer(max_iterations),
                )
                .unwrap();

            ma.start_preset(&preset_id).await.unwrap();

            let forwarded = probe_receiver(&ma, &fwd_id).await.unwrap();
            let limit = probe_receiver(&ma, &lim_id).await.unwrap();

            Fixture {
                ma,
                loop_agent,
                forwarded,
                limit,
            }
        }

        fn assistant_tool_call_msg(id: Option<&str>, streaming: bool) -> AgentValue {
            let mut msg = Message::assistant("use tools".to_string());
            msg.id = id.map(str::to_string);
            msg.streaming = streaming;
            msg.tool_calls = Some(vector![ToolCall {
                function: ToolCallFunction {
                    name: "my_tool".to_string(),
                    parameters: serde_json::json!({}),
                    id: Some("call1".to_string()),
                    parse_error: None,
                },
            }]);
            AgentValue::message(msg)
        }

        async fn send(fixture: &Fixture, ctx: &AgentContext, value: AgentValue) {
            fixture
                .loop_agent
                .lock()
                .await
                .process(ctx.clone(), PORT_MESSAGE.to_string(), value)
                .await
                .unwrap();
        }

        async fn recv(rx: &ProbeReceiver) -> AgentValue {
            let (_ctx, value) = rx.recv().await.unwrap();
            value
        }

        async fn expect_no_event(rx: &ProbeReceiver) {
            assert!(
                rx.recv_with_timeout(Duration::from_millis(200))
                    .await
                    .is_err()
            );
        }

        async fn count_for(fixture: &Fixture, ctx_key: &str) -> Option<(u32, Option<String>)> {
            let guard = fixture.loop_agent.lock().await;
            let agent = guard.as_agent::<LoopControlAgent>().unwrap();
            agent.counts.get(ctx_key).cloned()
        }

        #[tokio::test]
        async fn streaming_partials_are_not_double_counted() {
            let fixture = setup(25).await;
            let ctx = AgentContext::new();
            let ctx_key = ctx.ctx_key().unwrap();

            // Streaming partials re-deliver accumulated tool_calls under the
            // same id, followed by the streaming=false final message.
            send(&fixture, &ctx, assistant_tool_call_msg(Some("m1"), true)).await;
            send(&fixture, &ctx, assistant_tool_call_msg(Some("m1"), true)).await;
            send(&fixture, &ctx, assistant_tool_call_msg(Some("m1"), false)).await;

            // All three deliveries pass through transparently.
            for _ in 0..3 {
                let value = recv(&fixture.forwarded).await;
                assert_eq!(value.as_message().unwrap().id.as_deref(), Some("m1"));
            }
            // Only the final message is counted.
            assert_eq!(
                count_for(&fixture, &ctx_key).await,
                Some((1, Some("m1".to_string())))
            );
            expect_no_event(&fixture.limit).await;

            fixture.ma.quit();
        }

        #[tokio::test]
        async fn same_id_final_redelivered_counts_once() {
            let fixture = setup(25).await;
            let ctx = AgentContext::new();
            let ctx_key = ctx.ctx_key().unwrap();

            send(&fixture, &ctx, assistant_tool_call_msg(Some("m1"), false)).await;
            send(&fixture, &ctx, assistant_tool_call_msg(Some("m1"), false)).await;

            // Both deliveries are forwarded, but counted only once.
            for _ in 0..2 {
                let value = recv(&fixture.forwarded).await;
                assert_eq!(value.as_message().unwrap().id.as_deref(), Some("m1"));
            }
            assert_eq!(
                count_for(&fixture, &ctx_key).await,
                Some((1, Some("m1".to_string())))
            );
            expect_no_event(&fixture.limit).await;

            fixture.ma.quit();
        }

        #[tokio::test]
        async fn blocks_and_emits_limit_exceeded_after_max_iterations() {
            let fixture = setup(2).await;
            let ctx = AgentContext::new();

            send(&fixture, &ctx, assistant_tool_call_msg(Some("m1"), false)).await;
            send(&fixture, &ctx, assistant_tool_call_msg(Some("m2"), false)).await;
            for expected in ["m1", "m2"] {
                let value = recv(&fixture.forwarded).await;
                assert_eq!(value.as_message().unwrap().id.as_deref(), Some(expected));
            }

            // The third distinct countable message exceeds max_iterations=2:
            // it must not be forwarded, and limit_exceeded fires instead.
            send(&fixture, &ctx, assistant_tool_call_msg(Some("m3"), false)).await;
            let notice = recv(&fixture.limit).await;
            let notice = notice.as_message().unwrap();
            assert_eq!(notice.role, "assistant");
            assert!(!notice.streaming);
            assert!(notice.tool_calls.is_none());
            assert!(notice.content.contains("max_iterations of 2"));
            expect_no_event(&fixture.forwarded).await;

            // A re-delivered duplicate of the blocked message (same id) is
            // neither forwarded nor reported again.
            send(&fixture, &ctx, assistant_tool_call_msg(Some("m3"), false)).await;
            expect_no_event(&fixture.forwarded).await;
            expect_no_event(&fixture.limit).await;

            fixture.ma.quit();
        }

        #[tokio::test]
        async fn non_countable_values_pass_through_even_over_limit() {
            let fixture = setup(1).await;
            let ctx = AgentContext::new();

            send(&fixture, &ctx, assistant_tool_call_msg(Some("m1"), false)).await;
            let _ = recv(&fixture.forwarded).await;
            // Trip the limit for this flow.
            send(&fixture, &ctx, assistant_tool_call_msg(Some("m2"), false)).await;
            let _ = recv(&fixture.limit).await;

            // Non-countable traffic must keep flowing untouched.
            let passthrough = [
                AgentValue::message(Message::user("hi".to_string())),
                AgentValue::message(Message::tool("my_tool".to_string(), "ok".to_string())),
                AgentValue::message(Message::assistant("no tools".to_string())),
                assistant_tool_call_msg(Some("m4"), true),
                AgentValue::string("not a message"),
            ];
            for value in passthrough {
                send(&fixture, &ctx, value.clone()).await;
                let received = recv(&fixture.forwarded).await;
                assert_eq!(received, value);
            }
            expect_no_event(&fixture.limit).await;

            fixture.ma.quit();
        }

        #[tokio::test]
        async fn separate_ctx_keys_count_independently() {
            let fixture = setup(1).await;
            let ctx_a = AgentContext::new();
            let ctx_b = AgentContext::new();

            send(&fixture, &ctx_a, assistant_tool_call_msg(Some("a1"), false)).await;
            let value = recv(&fixture.forwarded).await;
            assert_eq!(value.as_message().unwrap().id.as_deref(), Some("a1"));

            // ctx_a hits its limit...
            send(&fixture, &ctx_a, assistant_tool_call_msg(Some("a2"), false)).await;
            let _ = recv(&fixture.limit).await;
            expect_no_event(&fixture.forwarded).await;

            // ...but ctx_b still has its own budget.
            send(&fixture, &ctx_b, assistant_tool_call_msg(Some("b1"), false)).await;
            let value = recv(&fixture.forwarded).await;
            assert_eq!(value.as_message().unwrap().id.as_deref(), Some("b1"));
            expect_no_event(&fixture.limit).await;

            fixture.ma.quit();
        }

        #[tokio::test]
        async fn stop_clears_counters() {
            let fixture = setup(1).await;
            let ctx = AgentContext::new();

            send(&fixture, &ctx, assistant_tool_call_msg(Some("m1"), false)).await;
            let _ = recv(&fixture.forwarded).await;
            send(&fixture, &ctx, assistant_tool_call_msg(Some("m2"), false)).await;
            let _ = recv(&fixture.limit).await;

            {
                let mut guard = fixture.loop_agent.lock().await;
                guard.stop().await.unwrap();
                guard.start().await.unwrap();
            }

            // After a restart the flow gets a fresh budget.
            send(&fixture, &ctx, assistant_tool_call_msg(Some("m3"), false)).await;
            let value = recv(&fixture.forwarded).await;
            assert_eq!(value.as_message().unwrap().id.as_deref(), Some("m3"));
            expect_no_event(&fixture.limit).await;

            fixture.ma.quit();
        }
    }
}
