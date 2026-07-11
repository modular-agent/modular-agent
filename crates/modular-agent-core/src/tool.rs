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
    AsAgent, Message, ModularAgent, ToolCall, async_trait, modular_agent,
};
use im::{Vector, vector};
use regex::RegexSet;
use tokio::sync::{Mutex as AsyncMutex, oneshot};

const CATEGORY: &str = "Core/Tool";

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

    /// JSON Schema describing the tool's parameters (optional).
    pub parameters: Option<serde_json::Value>,
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
        if let Some(params) = &info.parameters {
            if let Ok(params_value) = AgentValue::from_serialize(params) {
                obj.insert("parameters".to_string(), params_value);
            }
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
        self.try_output(ctx.clone(), PORT_TOOL_IN, args)?;

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
    agent: Arc<AsyncMutex<Box<dyn Agent>>>,
}

impl PresetTool {
    /// Creates a new PresetTool wrapping a PresetToolAgent.
    fn new(
        name: String,
        description: String,
        parameters: Option<serde_json::Value>,
        agent: Arc<AsyncMutex<Box<dyn Agent>>>,
    ) -> Self {
        Self {
            info: ToolInfo {
                name: name,
                description: description,
                parameters: parameters,
            },
            agent,
        }
    }

    /// Executes a tool call through the wrapped agent.
    ///
    /// Times out after 60 seconds if no response is received.
    async fn tool_call(
        &self,
        ctx: AgentContext,
        args: AgentValue,
    ) -> Result<AgentValue, AgentError> {
        // Kick off the tool call while holding the lock, then drop it before awaiting the result
        let rx = {
            let mut guard = self.agent.lock().await;
            let Some(preset_tool_agent) = guard.as_agent_mut::<PresetToolAgent>() else {
                return Err(AgentError::Other(
                    "Agent is not PresetToolAgent".to_string(),
                ));
            };
            preset_tool_agent.start_tool_call(ctx, args)?
        };

        tokio::time::timeout(Duration::from_secs(60), rx)
            .await
            .map_err(|_| AgentError::Other("tool_call timed out".to_string()))?
            .map_err(|_| AgentError::Other("tool_out dropped".to_string()))
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

        let resp_messages = call_tools(&ctx, &tool_calls).await?;
        for resp_msg in resp_messages {
            self.output(ctx.clone(), PORT_MESSAGE, AgentValue::message(resp_msg))
                .await?;
        }
        Ok(())
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
    fn test_error_tool_result_shape() {
        let call = ToolCall {
            function: ToolCallFunction {
                name: "my_tool".to_string(),
                parameters: serde_json::json!({}),
                id: Some("call42".to_string()),
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
    fn test_is_valid_tool_name_rejects_invalid() {
        assert!(!is_valid_tool_name(""));
        assert!(!is_valid_tool_name(&"x".repeat(65)));
        assert!(!is_valid_tool_name("my tool"));
        assert!(!is_valid_tool_name("my.tool"));
        assert!(!is_valid_tool_name("ツール"));
        assert!(!is_valid_tool_name("tool@1"));
    }
}
