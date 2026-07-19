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
    future::Future,
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::Duration,
};

use tokio_util::sync::CancellationToken;

use crate::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentStatus, AgentValue,
    AsAgent, Message, MessageContent, ModularAgent, SharedAgent, ToolCall, async_trait,
    modular_agent,
};
#[cfg(feature = "image")]
use crate::{ContentBlock, value::IMAGE_BASE64_PREFIX};
use futures_util::{StreamExt, stream};
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
const CONFIG_MAX_CONCURRENCY: &str = "max_concurrency";

/// Fallback timeout (seconds) when `timeout_secs` config is unset or unreadable.
const DEFAULT_TIMEOUT_SECS: i64 = 60;

/// Fallback loop limit when `max_iterations` config is unset or unreadable.
const DEFAULT_MAX_ITERATIONS: i64 = 25;

/// Fallback parallel-tool concurrency cap when `max_concurrency` config is
/// unset or unreadable.
const DEFAULT_MAX_CONCURRENCY: i64 = 8;

/// Content of the synthetic tool result emitted for a tool call whose
/// execution was cancelled before a real result was produced. Carrying the
/// original tool_call id keeps the message history consistent for the model.
const ABORTED_TOOL_RESULT: &str = "Operation aborted";

/// Runs `fut` to completion unless `cancel` fires first.
///
/// Returns `None` when cancelled (dropping `fut` mid-flight). A token that is
/// already cancelled short-circuits before `fut` is first polled.
async fn run_unless_cancelled<T>(
    cancel: Option<&CancellationToken>,
    fut: impl Future<Output = T>,
) -> Option<T> {
    match cancel {
        Some(token) => {
            tokio::select! {
                biased;
                _ = token.cancelled() => None,
                r = fut => Some(r),
            }
        }
        None => Some(fut.await),
    }
}

/// How a tool may be scheduled relative to other calls in the same batch.
///
/// Tools with side effects (database writes, message posting, workflow
/// invocations) must stay [`Sequential`](ExecutionMode::Sequential) so their
/// effects happen one at a time and in input order.
/// [`Parallel`](ExecutionMode::Parallel) is opt-in for independent,
/// read-only tools where concurrent execution is safe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Never runs concurrently with any other tool call (the default).
    #[default]
    Sequential,
    /// May run concurrently with adjacent `Parallel` calls.
    Parallel,
}

/// Metadata describing a tool available for LLM function calling.
///
/// This information is typically sent to the LLM to describe what tools
/// are available and how to call them.
#[derive(Clone, Debug)]
pub struct ToolInfo {
    /// Unique name identifying the tool.
    pub name: String,

    /// Human-readable description of what the tool does.
    ///
    /// Per Claude's tool-use guidance, the description is the single most
    /// important factor for tool-calling performance. Write a detailed
    /// description (3-4+ sentences) covering what the tool does, when it
    /// should (and should not) be used, what each parameter means, and any
    /// caveats or limitations. Registering a tool with a missing or very
    /// short description logs a warning (see [`register_tool`]).
    pub description: String,

    /// JSON Schema describing the tool's parameters.
    ///
    /// Defaults to `{"type": "object", "properties": {}}` when no schema
    /// is provided (see [`ToolInfo::new`]).
    pub parameters: serde_json::Value,

    /// How this tool may be scheduled relative to other calls in the same
    /// batch (see [`ExecutionMode`]). Internal execution metadata — never
    /// sent to the LLM.
    pub execution_mode: ExecutionMode,
}

impl ToolInfo {
    /// Creates a new `ToolInfo` with the default `Sequential` execution mode.
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
            execution_mode: ExecutionMode::default(),
        }
    }

    /// Sets the execution mode, typically to opt a read-only tool into
    /// parallel scheduling (see [`ExecutionMode`]).
    pub fn with_execution_mode(mut self, mode: ExecutionMode) -> Self {
        self.execution_mode = mode;
        self
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
        let info = tool.info();
        if let Some(warning) = description_warning(&info.name, &info.description) {
            log::warn!("{}", warning);
        }
        let name = info.name.to_string();
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
/// A missing or very short description logs a warning at registration time,
/// but never rejects the registration. See [`ToolInfo::description`] for
/// description best practices.
///
/// # Arguments
///
/// * `tool` - The tool implementation to register
pub fn register_tool<T: Tool + Send + Sync + 'static>(tool: T) {
    registry().write().unwrap().register_tool(tool);
}

/// Minimum description length (in characters) below which registration warns.
const MIN_DESCRIPTION_CHARS: usize = 10;

/// Returns a warning message when a tool description is too weak to guide
/// tool selection — missing, whitespace-only, or shorter than
/// [`MIN_DESCRIPTION_CHARS`] characters — or `None` when it is adequate.
fn description_warning(name: &str, description: &str) -> Option<String> {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        Some(format!(
            "Tool '{}' is registered without a description; the description is \
             the most important factor for tool-calling performance and should \
             explain what the tool does, when to use it, and what its \
             parameters mean",
            name
        ))
    } else if trimmed.chars().count() < MIN_DESCRIPTION_CHARS {
        Some(format!(
            "Tool '{}' has a very short description {:?}; detailed descriptions \
             (3-4+ sentences) significantly improve tool-calling performance",
            name, trimmed
        ))
    } else {
        None
    }
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
    if ctx.is_cancelled() {
        return Err(AgentError::Cancelled);
    }

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

/// Returns whether `value` validates against `schema`, treating an
/// uncompilable schema as non-matching.
fn schema_accepts(schema: &serde_json::Value, value: &serde_json::Value) -> bool {
    jsonschema::validator_for(schema)
        .map(|v| v.is_valid(value))
        .unwrap_or(false)
}

/// Coerces primitive values in-place toward `schema` where an unambiguous
/// string-to-primitive conversion exists.
///
/// LLMs frequently emit numbers and booleans as JSON strings (`"42"`,
/// `"true"`); this repairs such calls instead of bouncing them back to the
/// model. Only lossless conversions are applied — `"42.5"` is never coerced
/// to an integer — and values already matching the schema type are left
/// untouched. Recurses into object `properties` and array `items`. For
/// `anyOf`/`oneOf`, an already-valid value is kept as-is; otherwise coercion
/// is tried against each variant and the first result that validates wins.
/// Sibling keywords (`type`/`properties`/`items`) are still applied after
/// `anyOf`/`oneOf` handling, since schemas like
/// `{"properties": ..., "anyOf": [{"required": ...}]}` rely on property
/// coercion even when a variant already accepts the value. When nothing
/// applies the value is left unchanged so validation can report the error.
fn coerce_by_schema(value: &mut serde_json::Value, schema: &serde_json::Value) {
    let Some(schema_obj) = schema.as_object() else {
        return;
    };

    for key in ["anyOf", "oneOf"] {
        let Some(variants) = schema_obj.get(key).and_then(|v| v.as_array()) else {
            continue;
        };
        if !variants.iter().any(|v| schema_accepts(v, value)) {
            for variant in variants {
                let mut candidate = value.clone();
                coerce_by_schema(&mut candidate, variant);
                if schema_accepts(variant, &candidate) {
                    *value = candidate;
                    break;
                }
            }
        }
        break;
    }

    match schema_obj.get("type").and_then(|t| t.as_str()) {
        Some("integer") => {
            // i64 parsing rejects fractional strings, so "42.5" is never
            // silently truncated.
            if let Some(s) = value.as_str()
                && let Ok(i) = s.parse::<i64>()
            {
                *value = serde_json::Value::from(i);
            }
        }
        Some("number") => {
            // from_f64 rejects NaN/infinity, which have no JSON representation.
            if let Some(s) = value.as_str()
                && let Ok(f) = s.parse::<f64>()
                && let Some(n) = serde_json::Number::from_f64(f)
            {
                *value = serde_json::Value::Number(n);
            }
        }
        Some("boolean") => match value.as_str() {
            Some("true") => *value = serde_json::Value::Bool(true),
            Some("false") => *value = serde_json::Value::Bool(false),
            _ => {}
        },
        _ => {}
    }

    if let Some(props) = schema_obj.get("properties").and_then(|p| p.as_object())
        && let Some(map) = value.as_object_mut()
    {
        for (name, prop_schema) in props {
            if let Some(v) = map.get_mut(name) {
                coerce_by_schema(v, prop_schema);
            }
        }
    }

    if let Some(items) = schema_obj.get("items")
        && let Some(arr) = value.as_array_mut()
    {
        for v in arr {
            coerce_by_schema(v, items);
        }
    }
}

/// Validates (and lightly coerces) tool-call arguments against a tool's
/// JSON Schema.
///
/// [`coerce_by_schema`] is applied first, so string-encoded primitives from
/// the model are repaired before validation. All validation errors are
/// collected with their instance paths into one readable string. If the
/// schema itself fails to compile (e.g. supplied by a misbehaving MCP
/// server), validation and coercion are skipped with a warning — a broken
/// schema must not block tool execution.
fn validate_tool_args(
    schema: &serde_json::Value,
    args: &mut serde_json::Value,
) -> Result<(), String> {
    let validator = match jsonschema::validator_for(schema) {
        Ok(v) => v,
        Err(e) => {
            log::warn!(
                "Tool parameter schema failed to compile; skipping argument validation: {}",
                e
            );
            return Ok(());
        }
    };

    coerce_by_schema(args, schema);

    let errors = validator
        .iter_errors(args)
        .map(|e| format!("at '{}': {}", e.instance_path(), e))
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// Executes a single tool call, funneling every failure (unparseable
/// parameters, schema validation failure, tool error) through
/// [`error_tool_result`] so a failing call never aborts its siblings.
async fn execute_tool_call(ctx: &AgentContext, call: &ToolCall) -> Message {
    // A provider argument string that failed to parse even after repair is
    // reported back to the model rather than executed with bogus arguments.
    if let Some(err) = &call.function.parse_error {
        return error_tool_result(
            call,
            format!(
                "Tool call arguments could not be parsed as JSON; the call was \
                 not executed. Re-issue the call with valid JSON arguments. {}",
                err
            ),
        );
    }
    // Validate and coerce arguments against the tool's declared schema
    // before execution. An unknown tool falls through unchanged so
    // call_tool() reports the not-found error as usual.
    let mut parameters = call.function.parameters.clone();
    if let Some(tool) = get_tool(call.function.name.as_str())
        && let Err(msg) = validate_tool_args(&tool.info().parameters, &mut parameters)
    {
        return error_tool_result(
            call,
            format!(
                "Tool call arguments failed schema validation; the call was \
                 not executed. Re-issue the call with corrected arguments. \
                 Errors: {}",
                msg
            ),
        );
    }
    let args = match AgentValue::from_json(parameters) {
        Ok(args) => args,
        Err(e) => {
            return error_tool_result(call, format!("Failed to parse tool call parameters: {}", e));
        }
    };
    match call_tool(ctx.clone(), call.function.name.as_str(), args).await {
        Ok(tool_resp) => {
            let mut msg = Message::tool_with_content(
                call.function.name.clone(),
                tool_result_content(&tool_resp),
            );
            msg.id = call.function.id.clone();
            msg
        }
        Err(e) => error_tool_result(call, e),
    }
}

/// Builds the message content for a successful tool response.
///
/// An image result — or an array containing at least one image — becomes
/// structured content blocks so multimodal LLM clients can send the image
/// itself instead of an opaque base64 string. Every other value keeps the
/// legacy stringified-JSON form byte-for-byte: persisted sessions and
/// downstream consumers compare against that exact string, so it must not
/// change shape.
fn tool_result_content(resp: &AgentValue) -> MessageContent {
    #[cfg(feature = "image")]
    match resp {
        AgentValue::Image(img) => {
            return MessageContent::Blocks(vec![image_block(img)]);
        }
        AgentValue::Array(arr) if arr.iter().any(|v| matches!(v, AgentValue::Image(_))) => {
            let blocks = arr
                .iter()
                .map(|v| match v {
                    AgentValue::Image(img) => image_block(img),
                    other => ContentBlock::Text {
                        text: other.to_json().to_string(),
                    },
                })
                .collect();
            return MessageContent::Blocks(blocks);
        }
        _ => {}
    }
    MessageContent::Text(resp.to_json().to_string())
}

/// Converts a tool-result image into an image content block.
///
/// `get_base64()` returns a PNG data URL, while [`ContentBlock::Image`]
/// carries the raw base64 payload and the MIME type separately.
#[cfg(feature = "image")]
fn image_block(img: &photon_rs::PhotonImage) -> ContentBlock {
    ContentBlock::Image {
        data: img
            .get_base64()
            .trim_start_matches(IMAGE_BASE64_PREFIX)
            .to_string(),
        mime_type: "image/png".to_string(),
    }
}

/// Runs the accumulated batch of `Parallel` calls concurrently, bounded by
/// `max_concurrency`, and appends the results to `out` in the batch's order.
///
/// Returns `false` when `cancel` fired mid-batch. Even then `out` gains one
/// message per batch entry: in-flight calls are dropped and reported with a
/// synthetic aborted result, but calls that already completed keep their
/// real result — their side effects happened, so reporting them aborted
/// would mislead the model into re-issuing them.
async fn flush_parallel_batch(
    ctx: &AgentContext,
    batch: &mut Vec<&ToolCall>,
    max_concurrency: usize,
    out: &mut Vec<Message>,
    cancel: Option<&CancellationToken>,
) -> bool {
    if batch.is_empty() {
        return true;
    }
    // Materialize the futures before streaming: mapping lazily inside
    // stream::iter trips a higher-ranked lifetime inference error when the
    // stream is held across an await in the caller.
    let mut futures = Vec::with_capacity(batch.len());
    for (index, &call) in batch.iter().enumerate() {
        futures.push(async move { (index, execute_tool_call(ctx, call).await) });
    }
    // `buffer_unordered` with explicit indices instead of the in-order
    // `buffered`: a completed result must be observable even while an
    // earlier call is still running, or cancellation would discard it.
    let mut results = stream::iter(futures).buffer_unordered(max_concurrency);
    let mut slots: Vec<Option<Message>> = Vec::new();
    slots.resize_with(batch.len(), || None);
    let mut aborted = false;
    loop {
        match run_unless_cancelled(cancel, results.next()).await {
            Some(Some((index, msg))) => slots[index] = Some(msg),
            Some(None) => break,
            None => {
                aborted = true;
                break;
            }
        }
    }
    if aborted {
        // Salvage results that completed but were not yet yielded by the
        // stream; only still-pending calls are left to the aborted fill.
        use futures_util::FutureExt;
        while let Some(Some((index, msg))) = results.next().now_or_never() {
            slots[index] = Some(msg);
        }
    }
    drop(results);
    for (slot, call) in slots.iter_mut().zip(batch.drain(..)) {
        out.push(match slot.take() {
            Some(msg) => msg,
            None => error_tool_result(call, ABORTED_TOOL_RESULT),
        });
    }
    !aborted
}

/// Executes multiple tool calls and returns the results as messages.
///
/// Returns tool response messages suitable for continuing an LLM
/// conversation, always in the same order as `tool_calls` regardless of
/// completion order. Before execution, each call's arguments are validated
/// against the tool's JSON Schema with lightweight type coercion (see
/// [`coerce_by_schema`]); the coerced arguments are what the tool receives.
/// A failure of one call (unparseable parameters, schema validation failure,
/// or a tool error) does not abort the others: it is returned as a tool
/// message with `is_error: Some(true)` so the LLM can recover.
///
/// Scheduling follows each tool's [`ExecutionMode`]:
///
/// * Consecutive calls to `Parallel` tools are executed concurrently, with
///   at most `max_concurrency` calls in flight at once.
/// * A call to a `Sequential` tool (the default, including tools not found
///   in the registry) acts as a barrier: every earlier call must finish
///   first, and the sequential call runs alone.
///
/// # Cancellation
///
/// When `ctx` carries a cancellation token (see
/// [`AgentContext::cancel_token`]) and it fires mid-execution, in-flight
/// calls are dropped and every call without a real result receives a
/// synthetic `is_error` tool message with content `"Operation aborted"`,
/// carrying the original tool_call id. Calls that already completed keep
/// their real result. This keeps the message history consistent: the model
/// sees a result for every call it issued, and finished side effects are
/// not misreported as aborted.
///
/// # Arguments
///
/// * `ctx` - The agent context for the invocations
/// * `tool_calls` - The tool calls to execute
/// * `max_concurrency` - Upper bound on concurrently running `Parallel`
///   calls; values below 1 are treated as 1
///
/// # Returns
///
/// A vector of tool response messages, one for each tool call, in input order.
pub async fn call_tools(
    ctx: &AgentContext,
    tool_calls: &Vector<ToolCall>,
    max_concurrency: usize,
) -> Result<Vector<Message>, AgentError> {
    if tool_calls.is_empty() {
        return Ok(vector![]);
    };
    let max_concurrency = max_concurrency.max(1);
    let cancel = ctx.cancel_token();

    // Results land in input order throughout (flush_parallel_batch appends
    // its whole batch in order and sequential calls are barriers), so at any
    // point the calls still lacking a result are exactly
    // tool_calls[resp_messages.len()..].
    let mut aborted = false;
    let mut resp_messages = Vec::with_capacity(tool_calls.len());
    let mut parallel_batch: Vec<&ToolCall> = Vec::new();
    for call in tool_calls {
        // An unknown tool counts as Sequential so it flows through
        // execute_tool_call alone and call_tool() reports the not-found error.
        let mode = get_tool(call.function.name.as_str())
            .map(|tool| tool.info().execution_mode)
            .unwrap_or_default();
        if mode == ExecutionMode::Parallel {
            parallel_batch.push(call);
            continue;
        }
        // A Sequential call is a barrier: flush the pending parallel batch
        // first, then run the sequential call with nothing else in flight.
        if !flush_parallel_batch(
            ctx,
            &mut parallel_batch,
            max_concurrency,
            &mut resp_messages,
            cancel,
        )
        .await
        {
            aborted = true;
            break;
        }
        match run_unless_cancelled(cancel, execute_tool_call(ctx, call)).await {
            Some(msg) => resp_messages.push(msg),
            None => {
                aborted = true;
                break;
            }
        }
    }
    if !aborted
        && !flush_parallel_batch(
            ctx,
            &mut parallel_batch,
            max_concurrency,
            &mut resp_messages,
            cancel,
        )
        .await
    {
        aborted = true;
    }

    // History consistency on cancellation: every tool_call the model issued
    // must receive a result, so calls interrupted or never started get a
    // synthetic aborted result carrying the original tool_call id (mirrors
    // the stop_reason == "length" guard in CallToolMessageAgent).
    if aborted {
        for call in tool_calls.iter().skip(resp_messages.len()) {
            resp_messages.push(error_tool_result(call, ABORTED_TOOL_RESULT));
        }
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
    ///
    /// When `ctx` carries a cancellation token, the wait also aborts as soon
    /// as the token fires, returning [`AgentError::Cancelled`].
    async fn tool_call(
        &self,
        ctx: AgentContext,
        args: AgentValue,
    ) -> Result<AgentValue, AgentError> {
        if ctx.is_cancelled() {
            return Err(AgentError::Cancelled);
        }

        // Kick off the tool call while holding the lock, then drop it before awaiting the result
        let ctx_id = ctx.id();
        let cancel = ctx.cancel_token().cloned();
        let (rx, timeout_secs, pending) = {
            let mut guard = self.agent.lock().await;
            let Some(preset_tool_agent) = guard.as_agent_mut::<PresetToolAgent>() else {
                return Err(AgentError::Other(
                    "Agent is not PresetToolAgent".to_string(),
                ));
            };
            // Cancellation may have fired while waiting for the agent lock.
            // Check again immediately before pending state is registered and
            // `tool_in` is emitted.
            if ctx.is_cancelled() {
                return Err(AgentError::Cancelled);
            }
            let timeout_secs = preset_tool_agent
                .configs()
                .map(|c| c.get_integer_or(CONFIG_TIMEOUT_SECS, DEFAULT_TIMEOUT_SECS))
                .unwrap_or(DEFAULT_TIMEOUT_SECS);
            let pending = preset_tool_agent.pending.clone();
            let rx = preset_tool_agent.start_tool_call(ctx, args)?;
            (rx, timeout_secs, pending)
        };

        // Deregister the pending sender however this future ends — including
        // being dropped mid-await: call_tools races this whole future against
        // the flow token and drops it on cancellation without polling it
        // again, so cleanup after the await would never run. The pending map
        // is keyed by context id, which is shared by every tool call in the
        // same flow (including an LLM retry after an error), so a leftover
        // sender would deliver a late tool_out to whichever call registers
        // under the same id next. PresetTool registers with
        // ExecutionMode::Sequential and call_tools never runs a Sequential
        // call concurrently with anything, so within a flow no newer call can
        // have registered a sender while this one is alive — the guard cannot
        // remove a newer call's sender.
        struct PendingGuard {
            pending: Arc<Mutex<HashMap<usize, oneshot::Sender<AgentValue>>>>,
            ctx_id: usize,
        }
        impl Drop for PendingGuard {
            fn drop(&mut self) {
                self.pending.lock().unwrap().remove(&self.ctx_id);
            }
        }
        let _guard = PendingGuard { pending, ctx_id };

        let wait = async {
            let rx = async {
                rx.await
                    .map_err(|_| AgentError::Other("tool_out dropped".to_string()))
            };
            if timeout_secs <= 0 {
                rx.await
            } else {
                match tokio::time::timeout(Duration::from_secs(timeout_secs as u64), rx).await {
                    Ok(result) => result,
                    Err(_) => Err(AgentError::Timeout(format!(
                        "Tool call timed out after {} seconds",
                        timeout_secs
                    ))),
                }
            }
        };
        run_unless_cancelled(cancel.as_ref(), wait)
            .await
            .unwrap_or(Err(AgentError::Cancelled))
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
/// and outputs the results as tool response messages. Consecutive calls to
/// tools registered with `ExecutionMode::Parallel` run concurrently (bounded
/// by `max_concurrency`); calls to `Sequential` tools — the default — run one
/// at a time and act as barriers. Results are emitted in input order.
///
/// # Configuration
///
/// * `tools` - Optional regex patterns to filter which tools can be called
/// * `max_concurrency` - Maximum number of `Parallel`-mode tool calls in
///   flight at once; values below 1 are treated as 1 (default: 8)
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
    integer_config(name=CONFIG_MAX_CONCURRENCY, default=8),
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

        // Read at call time (like timeout_secs) so runtime config changes
        // take effect without a restart.
        let max_concurrency = self
            .configs()?
            .get_integer_or(CONFIG_MAX_CONCURRENCY, DEFAULT_MAX_CONCURRENCY)
            .max(1) as usize;

        let resp_messages = call_tools(&ctx, &tool_calls, max_concurrency).await?;
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
        assert_eq!(msg.text(), "something went wrong");
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
    fn test_description_warning_flags_weak_descriptions() {
        let warning = description_warning("my_tool", "").expect("empty should warn");
        assert!(warning.contains("my_tool"));

        let warning = description_warning("my_tool", "  \t\n  ").expect("whitespace should warn");
        assert!(warning.contains("my_tool"));

        // 9 characters (including the space) is below the threshold.
        let warning = description_warning("my_tool", "too short").expect("short should warn");
        assert!(warning.contains("my_tool"));
        assert!(warning.contains("too short"));
    }

    #[test]
    fn test_description_warning_accepts_adequate_descriptions() {
        assert!(description_warning("t", "0123456789").is_none());
        assert!(
            description_warning("t", "Fetches the current weather for a given city.").is_none()
        );
        // Threshold counts characters, not bytes: 12 chars, 36 bytes.
        assert!(description_warning("t", "ツールの詳細な説明です。").is_none());
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

    mod arg_validation {
        use super::*;
        use serde_json::json;

        /// Tool that echoes its arguments back, exposing what reached it
        /// after validation and coercion.
        struct EchoTool {
            info: ToolInfo,
        }

        #[async_trait]
        impl Tool for EchoTool {
            fn info(&self) -> &ToolInfo {
                &self.info
            }

            async fn call(
                &self,
                _ctx: AgentContext,
                args: AgentValue,
            ) -> Result<AgentValue, AgentError> {
                Ok(args)
            }
        }

        fn register_echo(name: &str, schema: Option<serde_json::Value>) {
            register_tool(EchoTool {
                info: ToolInfo::new(name, "echoes args", schema),
            });
        }

        fn call(name: &str, id: &str, params: serde_json::Value) -> ToolCall {
            ToolCall {
                function: ToolCallFunction {
                    name: name.to_string(),
                    parameters: params,
                    id: Some(id.to_string()),
                    parse_error: None,
                },
            }
        }

        fn content_json(msg: &Message) -> serde_json::Value {
            serde_json::from_str(&msg.text()).unwrap()
        }

        #[test]
        fn coerce_does_not_truncate_float_string_to_integer() {
            let schema = json!({"type": "integer"});
            let mut v = json!("42.5");
            coerce_by_schema(&mut v, &schema);
            assert_eq!(v, json!("42.5"));
        }

        #[test]
        fn coerce_number_accepts_float_string_and_keeps_integer() {
            let schema = json!({"type": "number"});
            let mut v = json!("42.5");
            coerce_by_schema(&mut v, &schema);
            assert_eq!(v, json!(42.5));

            // An integer is already a valid number and must stay untouched.
            let mut v = json!(3);
            coerce_by_schema(&mut v, &schema);
            assert_eq!(v, json!(3));
        }

        #[test]
        fn coerce_array_items() {
            let schema = json!({"type": "array", "items": {"type": "integer"}});
            let mut v = json!(["1", 2, "3"]);
            coerce_by_schema(&mut v, &schema);
            assert_eq!(v, json!([1, 2, 3]));
        }

        #[tokio::test]
        async fn string_primitives_coerced_before_tool_call() {
            let name = "p14_coerce_prims";
            register_echo(
                name,
                Some(json!({
                    "type": "object",
                    "properties": {
                        "count": {"type": "integer"},
                        "flag": {"type": "boolean"}
                    },
                    "required": ["count", "flag"]
                })),
            );

            let calls = vector![call(name, "c1", json!({"count": "42", "flag": "true"}))];
            let msgs = call_tools(&AgentContext::new(), &calls, 8).await.unwrap();
            unregister_tool(name);

            assert_eq!(msgs.len(), 1);
            assert_eq!(msgs[0].is_error, None);
            assert_eq!(msgs[0].id.as_deref(), Some("c1"));
            assert_eq!(content_json(&msgs[0]), json!({"count": 42, "flag": true}));
        }

        #[tokio::test]
        async fn nested_object_property_coerced() {
            let name = "p14_coerce_nested";
            register_echo(
                name,
                Some(json!({
                    "type": "object",
                    "properties": {
                        "outer": {
                            "type": "object",
                            "properties": {"n": {"type": "integer"}}
                        }
                    }
                })),
            );

            let calls = vector![call(name, "c1", json!({"outer": {"n": "7"}}))];
            let msgs = call_tools(&AgentContext::new(), &calls, 8).await.unwrap();
            unregister_tool(name);

            assert_eq!(msgs.len(), 1);
            assert_eq!(msgs[0].is_error, None);
            assert_eq!(content_json(&msgs[0]), json!({"outer": {"n": 7}}));
        }

        #[tokio::test]
        async fn validation_failure_reports_error_and_batch_continues() {
            let name = "p14_validation_failure";
            register_echo(
                name,
                Some(json!({
                    "type": "object",
                    "properties": {"count": {"type": "integer"}},
                    "required": ["count"]
                })),
            );

            let calls = vector![
                call(name, "bad", json!({"count": "abc"})),
                call(name, "good", json!({"count": "5"})),
            ];
            let msgs = call_tools(&AgentContext::new(), &calls, 8).await.unwrap();
            unregister_tool(name);

            assert_eq!(msgs.len(), 2);
            assert_eq!(msgs[0].is_error, Some(true));
            assert_eq!(msgs[0].id.as_deref(), Some("bad"));
            assert!(msgs[0].text().contains("not executed"));
            assert!(msgs[0].text().contains("/count"));

            // The failure of the first call must not abort the second.
            assert_eq!(msgs[1].is_error, None);
            assert_eq!(msgs[1].id.as_deref(), Some("good"));
            assert_eq!(content_json(&msgs[1]), json!({"count": 5}));
        }

        #[tokio::test]
        async fn validation_failure_collects_all_errors() {
            let name = "p14_all_errors";
            register_echo(
                name,
                Some(json!({
                    "type": "object",
                    "properties": {
                        "count": {"type": "integer"},
                        "flag": {"type": "boolean"}
                    },
                    "required": ["count", "flag"]
                })),
            );

            let calls = vector![call(name, "bad", json!({"count": "abc", "flag": "xyz"}))];
            let msgs = call_tools(&AgentContext::new(), &calls, 8).await.unwrap();
            unregister_tool(name);

            assert_eq!(msgs.len(), 1);
            assert_eq!(msgs[0].is_error, Some(true));
            // Every invalid path must be reported, not just the first error.
            assert!(msgs[0].text().contains("/count"));
            assert!(msgs[0].text().contains("/flag"));
        }

        #[tokio::test]
        async fn default_empty_object_schema_accepts_arbitrary_args() {
            let name = "p14_default_schema";
            register_echo(name, None);

            let args = json!({"anything": [1, 2, 3], "nested": {"x": "y"}});
            let calls = vector![call(name, "c1", args.clone())];
            let msgs = call_tools(&AgentContext::new(), &calls, 8).await.unwrap();
            unregister_tool(name);

            assert_eq!(msgs.len(), 1);
            assert_eq!(msgs[0].is_error, None);
            assert_eq!(content_json(&msgs[0]), args);
        }

        #[tokio::test]
        async fn uncompilable_schema_skips_validation_and_executes() {
            let name = "p14_broken_schema";
            register_echo(name, Some(json!({"type": 123})));

            let args = json!({"x": "1"});
            let calls = vector![call(name, "c1", args.clone())];
            let msgs = call_tools(&AgentContext::new(), &calls, 8).await.unwrap();
            unregister_tool(name);

            assert_eq!(msgs.len(), 1);
            assert_eq!(msgs[0].is_error, None);
            // Coercion is also skipped: the original string arrives untouched.
            assert_eq!(content_json(&msgs[0]), args);
        }

        #[tokio::test]
        async fn any_of_coercion_picks_validating_variant() {
            let name = "p14_any_of";
            register_echo(
                name,
                Some(json!({
                    "type": "object",
                    "properties": {
                        "v": {"anyOf": [{"type": "integer"}, {"type": "boolean"}]}
                    }
                })),
            );

            let calls = vector![
                call(name, "c1", json!({"v": "true"})),
                call(name, "c2", json!({"v": "7"})),
                call(name, "c3", json!({"v": 5})),
            ];
            let msgs = call_tools(&AgentContext::new(), &calls, 8).await.unwrap();
            unregister_tool(name);

            assert_eq!(msgs.len(), 3);
            // "true" fails the integer variant but coerces to the boolean one.
            assert_eq!(content_json(&msgs[0]), json!({"v": true}));
            assert_eq!(content_json(&msgs[1]), json!({"v": 7}));
            // An already-valid value is left untouched.
            assert_eq!(content_json(&msgs[2]), json!({"v": 5}));
        }

        #[tokio::test]
        async fn any_of_with_sibling_properties_still_coerces() {
            let name = "p14_any_of_siblings";
            // Common "at least one of" pattern: anyOf lists required variants
            // while sibling properties carry the type information. A variant
            // accepting the raw object must not skip property coercion.
            register_echo(
                name,
                Some(json!({
                    "type": "object",
                    "properties": {
                        "a": {"type": "integer"},
                        "b": {"type": "integer"}
                    },
                    "anyOf": [{"required": ["a"]}, {"required": ["b"]}]
                })),
            );

            let calls = vector![call(name, "c1", json!({"a": "5"}))];
            let msgs = call_tools(&AgentContext::new(), &calls, 8).await.unwrap();
            unregister_tool(name);

            assert_eq!(msgs.len(), 1);
            assert_eq!(msgs[0].is_error, None);
            assert_eq!(content_json(&msgs[0]), json!({"a": 5}));
        }
    }

    mod result_content {
        use super::*;

        /// Tool that returns a fixed value regardless of its arguments.
        struct FixedTool {
            info: ToolInfo,
            value: AgentValue,
        }

        #[async_trait]
        impl Tool for FixedTool {
            fn info(&self) -> &ToolInfo {
                &self.info
            }

            async fn call(
                &self,
                _ctx: AgentContext,
                _args: AgentValue,
            ) -> Result<AgentValue, AgentError> {
                Ok(self.value.clone())
            }
        }

        fn register_fixed(name: &str, value: AgentValue) {
            register_tool(FixedTool {
                info: ToolInfo::new(name, "returns a fixed value for tests", None),
                value,
            });
        }

        fn call(name: &str, id: &str) -> ToolCall {
            ToolCall {
                function: ToolCallFunction {
                    name: name.to_string(),
                    parameters: serde_json::json!({}),
                    id: Some(id.to_string()),
                    parse_error: None,
                },
            }
        }

        async fn run_single(name: &str) -> Message {
            let calls = vector![call(name, "c1")];
            let mut msgs = call_tools(&AgentContext::new(), &calls, 8).await.unwrap();
            unregister_tool(name);
            assert_eq!(msgs.len(), 1);
            msgs.remove(0)
        }

        #[cfg(feature = "image")]
        #[tokio::test]
        async fn image_result_becomes_single_image_block() {
            let name = "result_content_image";
            register_fixed(name, AgentValue::image_default());

            let msg = run_single(name).await;
            assert_eq!(msg.role, "tool");
            assert_eq!(msg.tool_name.as_deref(), Some(name));
            assert_eq!(msg.id.as_deref(), Some("c1"));
            assert_eq!(msg.is_error, None);

            let MessageContent::Blocks(blocks) = &msg.content else {
                panic!("expected block content, got {:?}", msg.content);
            };
            assert_eq!(blocks.len(), 1);
            let ContentBlock::Image { data, mime_type } = &blocks[0] else {
                panic!("expected image block, got {:?}", blocks[0]);
            };
            assert_eq!(mime_type, "image/png");
            assert!(!data.starts_with("data:"));
            assert!(!data.is_empty());
        }

        #[cfg(feature = "image")]
        #[tokio::test]
        async fn mixed_array_result_becomes_ordered_blocks() {
            let name = "result_content_mixed_array";
            register_fixed(
                name,
                AgentValue::array(vector![
                    AgentValue::image_default(),
                    AgentValue::string("caption"),
                ]),
            );

            let msg = run_single(name).await;
            let MessageContent::Blocks(blocks) = &msg.content else {
                panic!("expected block content, got {:?}", msg.content);
            };
            assert_eq!(blocks.len(), 2);
            assert!(matches!(&blocks[0], ContentBlock::Image { .. }));
            let ContentBlock::Text { text } = &blocks[1] else {
                panic!("expected text block, got {:?}", blocks[1]);
            };
            // Non-image array elements keep their stringified-JSON form.
            assert_eq!(text, "\"caption\"");
        }

        #[tokio::test]
        async fn non_image_results_keep_legacy_text_form() {
            let object = AgentValue::from_json(serde_json::json!({"a": 1, "b": "x"})).unwrap();
            let imageless_array = AgentValue::from_json(serde_json::json!([1, "two", null])).unwrap();
            let cases = [
                ("result_content_object", object),
                ("result_content_string", AgentValue::string("plain")),
                ("result_content_array", imageless_array),
            ];
            for (name, value) in cases {
                let expected = value.to_json().to_string();
                register_fixed(name, value);
                let msg = run_single(name).await;
                assert_eq!(msg.content, MessageContent::Text(expected));
            }
        }

        #[tokio::test]
        async fn error_result_stays_text() {
            // An unregistered tool routes through error_tool_result.
            let calls = vector![call("result_content_no_such_tool", "c1")];
            let msgs = call_tools(&AgentContext::new(), &calls, 8).await.unwrap();
            assert_eq!(msgs.len(), 1);
            assert_eq!(msgs[0].is_error, Some(true));
            assert!(matches!(msgs[0].content, MessageContent::Text(_)));
            assert!(msgs[0].text().contains("not found"));
        }
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
            assert!(notice.text().contains("max_iterations of 2"));
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

    mod cancellation {
        use super::*;
        use std::time::Duration;

        /// Tool that never finishes on its own.
        struct SlowTool {
            info: ToolInfo,
        }

        #[async_trait]
        impl Tool for SlowTool {
            fn info(&self) -> &ToolInfo {
                &self.info
            }

            async fn call(
                &self,
                _ctx: AgentContext,
                _args: AgentValue,
            ) -> Result<AgentValue, AgentError> {
                tokio::time::sleep(Duration::from_secs(30)).await;
                Ok(AgentValue::string("done"))
            }
        }

        /// Tool that completes immediately.
        struct FastTool {
            info: ToolInfo,
        }

        #[async_trait]
        impl Tool for FastTool {
            fn info(&self) -> &ToolInfo {
                &self.info
            }

            async fn call(
                &self,
                _ctx: AgentContext,
                _args: AgentValue,
            ) -> Result<AgentValue, AgentError> {
                Ok(AgentValue::string("fast done"))
            }
        }

        fn slow_call(name: &str, id: &str) -> ToolCall {
            ToolCall {
                function: ToolCallFunction {
                    name: name.to_string(),
                    parameters: serde_json::json!({}),
                    id: Some(id.to_string()),
                    parse_error: None,
                },
            }
        }

        #[tokio::test]
        async fn cancelled_call_tools_synthesizes_aborted_results() {
            // Unique name: the registry is process-global and shared with
            // other tests running in parallel.
            let tool_name = "cancel_test_slow_tool";
            register_tool(SlowTool {
                info: ToolInfo::new(tool_name, "sleeps forever for cancellation tests", None),
            });

            let token = CancellationToken::new();
            let ctx = AgentContext::new().with_cancel_token(token.clone());
            let calls: Vector<ToolCall> =
                vector![slow_call(tool_name, "c1"), slow_call(tool_name, "c2")];

            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(50)).await;
                token.cancel();
            });

            let msgs = tokio::time::timeout(Duration::from_secs(5), call_tools(&ctx, &calls, 8))
                .await
                .expect("cancelled call_tools must return promptly")
                .unwrap();

            // Every issued tool_call receives a result carrying its id.
            assert_eq!(msgs.len(), 2);
            for (msg, id) in msgs.iter().zip(["c1", "c2"]) {
                assert_eq!(msg.role, "tool");
                assert_eq!(msg.id.as_deref(), Some(id));
                assert_eq!(msg.is_error, Some(true));
                assert_eq!(msg.text(), ABORTED_TOOL_RESULT);
            }

            unregister_tool(tool_name);
        }

        #[tokio::test]
        async fn cancellation_keeps_results_of_completed_parallel_calls() {
            // Unique names: the registry is process-global and shared with
            // other tests running in parallel.
            let slow_name = "cancel_test_slow_parallel_tool";
            let fast_name = "cancel_test_fast_parallel_tool";
            register_tool(SlowTool {
                info: ToolInfo::new(slow_name, "sleeps forever for cancellation tests", None)
                    .with_execution_mode(ExecutionMode::Parallel),
            });
            register_tool(FastTool {
                info: ToolInfo::new(
                    fast_name,
                    "completes immediately for cancellation tests",
                    None,
                )
                .with_execution_mode(ExecutionMode::Parallel),
            });

            let token = CancellationToken::new();
            let ctx = AgentContext::new().with_cancel_token(token.clone());
            // The fast call sits behind the still-running slow call in input
            // order — its completed result must survive the abort.
            let calls: Vector<ToolCall> =
                vector![slow_call(slow_name, "c1"), slow_call(fast_name, "c2")];

            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(50)).await;
                token.cancel();
            });

            let msgs = tokio::time::timeout(Duration::from_secs(5), call_tools(&ctx, &calls, 8))
                .await
                .expect("cancelled call_tools must return promptly")
                .unwrap();

            assert_eq!(msgs.len(), 2);
            assert_eq!(msgs[0].id.as_deref(), Some("c1"));
            assert_eq!(msgs[0].is_error, Some(true));
            assert_eq!(msgs[0].text(), ABORTED_TOOL_RESULT);
            // The completed call keeps its real result instead of being
            // misreported as aborted.
            assert_eq!(msgs[1].id.as_deref(), Some("c2"));
            assert_eq!(msgs[1].is_error, None);
            assert!(msgs[1].text().contains("fast done"));

            unregister_tool(slow_name);
            unregister_tool(fast_name);
        }
    }
}
