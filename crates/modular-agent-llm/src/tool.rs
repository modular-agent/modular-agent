use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::Duration,
};

use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};
use regex::Regex;
use tokio::sync::{Mutex as AsyncMutex, oneshot};

use crate::message::{Message, ToolCall};

const CATEGORY: &str = "LLM/Tool";

const PIN_REGEX: &str = "regex";
const PIN_TOOLS: &str = "tools";

const PIN_TOOL_IN: &str = "tool_in";
const PIN_TOOL_OUT: &str = "tool_out";

const CONFIG_TOOL_NAME: &str = "name";
const CONFIG_TOOL_DESCRIPTION: &str = "description";
const CONFIG_TOOL_PARAMETERS: &str = "parameters";

#[derive(Clone, Debug)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters: Option<serde_json::Value>,
}

/// Trait for Tool implementations.
#[async_trait]
pub trait Tool {
    fn info(&self) -> &ToolInfo;

    /// Call the tool with the given context and arguments.
    async fn call(&mut self, ctx: AgentContext, args: AgentValue)
    -> Result<AgentValue, AgentError>;
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
        AgentValue::object(obj)
    }
}

#[derive(Clone)]
struct ToolEntry {
    info: ToolInfo,
    tool: Arc<AsyncMutex<Box<dyn Tool + Send + Sync>>>,
}

impl ToolEntry {
    fn new<T: Tool + Send + Sync + 'static>(tool: T) -> Self {
        Self {
            info: tool.info().clone(),
            tool: Arc::new(AsyncMutex::new(
                Box::new(tool) as Box<dyn Tool + Send + Sync>
            )),
        }
    }
}

struct ToolRegistry {
    tools: HashMap<String, ToolEntry>,
}

impl ToolRegistry {
    fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    fn register_tool<T: Tool + Send + Sync + 'static>(&mut self, tool: T) {
        let name = tool.info().name.to_string();
        let entry = ToolEntry::new(tool);
        self.tools.insert(name, entry);
    }

    fn unregister_tool(&mut self, name: &str) {
        self.tools.remove(name);
    }

    fn get_tool(&self, name: &str) -> Option<Arc<AsyncMutex<Box<dyn Tool + Send + Sync>>>> {
        self.tools.get(name).map(|entry| entry.tool.clone())
    }
}

// Global registry instance.
static TOOL_REGISTRY: OnceLock<RwLock<ToolRegistry>> = OnceLock::new();

fn registry() -> &'static RwLock<ToolRegistry> {
    TOOL_REGISTRY.get_or_init(|| RwLock::new(ToolRegistry::new()))
}

/// Register a new tool.
pub fn register_tool<T: Tool + Send + Sync + 'static>(tool: T) {
    registry().write().unwrap().register_tool(tool);
}

/// Unregister a tool by name.
pub fn unregister_tool(name: &str) {
    registry().write().unwrap().unregister_tool(name);
}

/// List all registered tool infos.
pub fn list_tool_infos() -> Vec<ToolInfo> {
    registry()
        .read()
        .unwrap()
        .tools
        .values()
        .map(|entry| entry.info.clone())
        .collect()
}

/// List registerd tool infos filtered by a regex.
pub fn list_tool_infos_regex(regex: &Regex) -> Vec<ToolInfo> {
    registry()
        .read()
        .unwrap()
        .tools
        .values()
        .filter_map(|entry| {
            if regex.is_match(&entry.info.name) {
                Some(entry.info.clone())
            } else {
                None
            }
        })
        .collect()
}

/// Get a tool by name.
pub fn get_tool(name: &str) -> Option<Arc<AsyncMutex<Box<dyn Tool + Send + Sync>>>> {
    registry().read().unwrap().get_tool(name)
}

/// Call a tool by name.
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

    let mut tool_guard = tool.lock().await;
    tool_guard.call(ctx, args).await
}

pub async fn call_tools(
    ctx: &AgentContext,
    tool_calls: &Vec<ToolCall>,
) -> Result<Vec<Message>, AgentError> {
    if tool_calls.is_empty() {
        return Ok(vec![]);
    };
    let mut resp_messages = vec![];

    for call in tool_calls {
        let args: AgentValue =
            AgentValue::from_json(call.function.parameters.clone()).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to parse tool call parameters: {}", e))
            })?;
        let tool_resp = call_tool(ctx.clone(), call.function.name.as_str(), args).await?;
        resp_messages.push(Message::tool(
            call.function.name.clone(),
            tool_resp.to_json().to_string(),
        ));
    }

    Ok(resp_messages)
}

// Agents

#[askit_agent(
    title="List Tools",
    category=CATEGORY,
    inputs=[PIN_REGEX],
    outputs=[PIN_TOOLS],
)]
pub struct ListToolsAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ListToolsAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let regex = value
            .as_str()
            .map(|s| Regex::new(s))
            .transpose()
            .map_err(|e| {
                AgentError::Other(format!("Failed to compile regex from input value: {e}"))
            })?;

        let tools = if let Some(regex) = regex {
            list_tool_infos_regex(&regex)
        } else {
            list_tool_infos()
        };
        let tools = tools
            .into_iter()
            .map(|tool| tool.into())
            .collect::<Vec<AgentValue>>();
        let tools_array = AgentValue::array(tools);

        self.try_output(ctx, PIN_TOOLS, tools_array)?;

        Ok(())
    }
}

#[askit_agent(
    title="Flow Tool",
    category=CATEGORY,
    inputs=[PIN_TOOL_OUT],
    outputs=[PIN_TOOL_IN],
    string_config(name=CONFIG_TOOL_NAME),
    text_config(name=CONFIG_TOOL_DESCRIPTION),
    text_config(name=CONFIG_TOOL_PARAMETERS),
)]
pub struct FlowToolAgent {
    data: AgentData,
    name: String,
    description: String,
    parameters: Option<serde_json::Value>,
    pending: Arc<Mutex<HashMap<usize, oneshot::Sender<AgentValue>>>>,
}

impl FlowToolAgent {
    fn start_tool_call(
        &mut self,
        ctx: AgentContext,
        args: AgentValue,
    ) -> Result<oneshot::Receiver<AgentValue>, AgentError> {
        let (tx, rx) = oneshot::channel();

        self.pending.lock().unwrap().insert(ctx.id(), tx);
        self.try_output(ctx.clone(), PIN_TOOL_IN, args)?;

        Ok(rx)
    }
}

#[async_trait]
impl AsAgent for FlowToolAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
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
            data: AgentData::new(askit, id, spec),
            name,
            description,
            parameters,
            pending: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        self.name = self.configs()?.get_string_or_default(CONFIG_TOOL_NAME);
        self.description = self
            .configs()?
            .get_string_or_default(CONFIG_TOOL_DESCRIPTION);
        self.parameters = self
            .configs()?
            .get(CONFIG_TOOL_PARAMETERS)
            .ok()
            .and_then(|v| serde_json::to_value(v).ok());

        // TODO: update registered tool info

        Ok(())
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        let agent_handle = self
            .askit()
            .get_agent(self.id())
            .ok_or_else(|| AgentError::AgentNotFound(self.id().to_string()))?;
        let tool = FlowTool::new(
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
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if let Some(tx) = self.pending.lock().unwrap().remove(&ctx.id()) {
            let _ = tx.send(value);
        }
        Ok(())
    }
}

struct FlowTool {
    info: ToolInfo,
    agent: Arc<AsyncMutex<Box<dyn Agent>>>,
}

impl FlowTool {
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

    async fn tool_call(
        &self,
        ctx: AgentContext,
        args: AgentValue,
    ) -> Result<AgentValue, AgentError> {
        // Kick off the tool call while holding the lock, then drop it before awaiting the result
        let rx = {
            let mut guard = self.agent.lock().await;
            let Some(flow_agent) = guard.as_agent_mut::<FlowToolAgent>() else {
                return Err(AgentError::Other("Agent is not FlowToolAgent".to_string()));
            };
            flow_agent.start_tool_call(ctx, args)?
        };

        tokio::time::timeout(Duration::from_secs(60), rx)
            .await
            .map_err(|_| AgentError::Other("tool_call timed out".to_string()))?
            .map_err(|_| AgentError::Other("tool_out dropped".to_string()))
    }
}

#[async_trait]
impl Tool for FlowTool {
    fn info(&self) -> &ToolInfo {
        &self.info
    }

    async fn call(
        &mut self,
        ctx: AgentContext,
        args: AgentValue,
    ) -> Result<AgentValue, AgentError> {
        self.tool_call(ctx, args).await
    }
}
