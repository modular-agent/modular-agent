#![cfg(feature = "mcp")]

use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};
use regex::Regex;
use rmcp::{
    model::{CallToolRequestParam, CallToolResult},
    service::ServiceExt,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use tokio::process::Command;

use crate::tool::{Tool, ToolInfo, register_tool, unregister_tool};

static CATEGORY: &str = "LLM/MCP";

static PIN_UNIT: &str = "unit";
static PIN_VALUE: &str = "value";
static PIN_RESPONSE: &str = "response";

static CONFIG_ARGS: &str = "args";
static CONFIG_COMMAND: &str = "command";
static CONFIG_NAME: &str = "name";
static CONFIG_TOOL: &str = "tool";
static CONFIG_TOOL_REGEX: &str = "tool_regex";

#[askit_agent(
    title="MCP Tools List",
    category=CATEGORY,
    inputs=[PIN_UNIT],
    outputs=[PIN_VALUE],
    string_config(name=CONFIG_COMMAND),
    string_config(name=CONFIG_ARGS),
)]
pub struct MCPToolsListAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for MCPToolsListAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }
    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let command = self.configs()?.get_string_or_default(CONFIG_COMMAND);
        let args_str = self.configs()?.get_string_or_default(CONFIG_ARGS);
        let args: Vec<String> = serde_json::from_str(&args_str)
            .map_err(|e| AgentError::InvalidValue(format!("Failed to parse args JSON: {e}")))?;

        let service = ()
            .serve(
                TokioChildProcess::new(Command::new(&command).configure(|cmd| {
                    for arg in &args {
                        cmd.arg(arg);
                    }
                }))
                .map_err(|e| AgentError::Other(format!("Failed to start MCP process: {e}")))?,
            )
            .await
            .map_err(|e| AgentError::Other(format!("Failed to start MCP service: {e}")))?;

        let tools_list = service
            .list_tools(Default::default())
            .await
            .map_err(|e| AgentError::Other(format!("Failed to list MCP tools: {e}")))?;

        service
            .cancel()
            .await
            .map_err(|e| AgentError::Other(format!("Failed to cancel MCP service: {e}")))?;

        let tools_value = AgentValue::from_serialize(&tools_list).map_err(|e| {
            AgentError::Other(format!(
                "Failed to serialize MCP tools list to AgentValue: {e}"
            ))
        })?;

        self.try_output(ctx, PIN_VALUE, tools_value)?;

        Ok(())
    }
}

#[askit_agent(
    title="MCP Call",
    category=CATEGORY,
    inputs=[PIN_VALUE],
    outputs=[PIN_VALUE, PIN_RESPONSE],
    string_config(name=CONFIG_TOOL),
    string_config(name=CONFIG_COMMAND),
    text_config(name=CONFIG_ARGS),
)]
pub struct MCPCallAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for MCPCallAgent {
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
        let command = self.configs()?.get_string_or_default(CONFIG_COMMAND);
        let args_str = self.configs()?.get_string_or_default(CONFIG_ARGS);
        let args: Vec<String> = serde_json::from_str(&args_str)
            .map_err(|e| AgentError::InvalidValue(format!("Failed to parse args JSON: {e}")))?;

        let service = ()
            .serve(
                TokioChildProcess::new(Command::new(&command).configure(|cmd| {
                    for arg in &args {
                        cmd.arg(arg);
                    }
                }))
                .map_err(|e| AgentError::Other(format!("Failed to start MCP process: {e}")))?,
            )
            .await
            .map_err(|e| AgentError::Other(format!("Failed to start MCP service: {e}")))?;

        let tool_name = self.configs()?.get_string_or_default(CONFIG_TOOL);
        if tool_name.is_empty() {
            return Ok(());
        }

        let arguments = value.as_object().map(|obj| {
            obj.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        serde_json::to_value(v).unwrap_or(serde_json::Value::Null),
                    )
                })
                .collect::<serde_json::Map<String, serde_json::Value>>()
        });

        let tool_result = service
            .call_tool(CallToolRequestParam {
                name: tool_name.clone().into(),
                arguments,
            })
            .await
            .map_err(|e| AgentError::Other(format!("Failed to call tool '{}': {e}", tool_name)))?;

        service
            .cancel()
            .await
            .map_err(|e| AgentError::Other(format!("Failed to cancel MCP service: {e}")))?;

        self.try_output(
            ctx.clone(),
            PIN_VALUE,
            call_tool_result_to_agent_value(tool_result.clone())?,
        )?;

        let response = serde_json::to_string_pretty(&tool_result).map_err(|e| {
            AgentError::Other(format!(
                "Failed to serialize tool result content to JSON: {e}"
            ))
        })?;
        self.try_output(ctx, PIN_RESPONSE, AgentValue::string(response))?;

        Ok(())
    }
}

fn call_tool_result_to_agent_value(result: CallToolResult) -> Result<AgentValue, AgentError> {
    let mut contents = Vec::new();
    for c in result.content.iter() {
        match &c.raw {
            rmcp::model::RawContent::Text(text) => {
                contents.push(AgentValue::string(text.text.clone()));
            }
            _ => {
                // Handle other content types as needed
            }
        }
    }
    let data = AgentValue::array(contents.into());
    if result.is_error == Some(true) {
        return Err(AgentError::Other(
            serde_json::to_string(&data).map_err(|e| AgentError::InvalidValue(e.to_string()))?,
        ));
    }
    Ok(data)
}

#[askit_agent(
    title="MCP Tool",
    category=CATEGORY,
    inputs=[],
    outputs=[],
    string_config(name=CONFIG_NAME),
    string_config(name=CONFIG_TOOL_REGEX),
    string_config(name=CONFIG_COMMAND),
    text_config(name=CONFIG_ARGS),
)]
pub struct MCPToolAgent {
    data: AgentData,
    mcp_tool_names: Vec<String>,
}

#[async_trait]
impl AsAgent for MCPToolAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            mcp_tool_names: Vec::new(),
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        let name = self.configs()?.get_string_or_default(CONFIG_NAME);
        if name.is_empty() {
            return Ok(());
        }

        let tool_regex = Regex::new(&self.configs()?.get_string_or(CONFIG_TOOL_REGEX, "*"))
            .map_err(|e| AgentError::InvalidValue(format!("Invalid tool regex: {e}")))?;

        let command = self.configs()?.get_string_or_default(CONFIG_COMMAND);
        let args_str = self.configs()?.get_string_or_default(CONFIG_ARGS);
        let args: Vec<String> = serde_json::from_str(&args_str)
            .map_err(|e| AgentError::InvalidValue(format!("Failed to parse args JSON: {e}")))?;

        let service = ()
            .serve(
                TokioChildProcess::new(Command::new(&command).configure(|cmd| {
                    for arg in &args {
                        cmd.arg(arg);
                    }
                }))
                .map_err(|e| AgentError::Other(format!("Failed to start MCP process: {e}")))?,
            )
            .await
            .map_err(|e| AgentError::Other(format!("Failed to start MCP service: {e}")))?;

        let tools_list = service
            .list_tools(Default::default())
            .await
            .map_err(|e| AgentError::Other(format!("Failed to list MCP tools: {e}")))?;

        service
            .cancel()
            .await
            .map_err(|e| AgentError::Other(format!("Failed to cancel MCP service: {e}")))?;

        let tool_infos = tools_list
            .tools
            .into_iter()
            .filter(|t| tool_regex.is_match(&t.name))
            .collect::<Vec<_>>();
        if tool_infos.is_empty() {
            return Err(AgentError::InvalidValue(format!(
                "No MCP tools found matching regex '{}'",
                tool_regex.as_str()
            )));
        }

        for tool_info in tool_infos {
            let mcp_tool_name = format!("{}::{}", name, tool_info.name);
            self.mcp_tool_names.push(mcp_tool_name.clone());
            register_tool(MCPTool::new(
                mcp_tool_name,
                command.clone(),
                args.clone(),
                tool_info,
            ));
        }

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        for mcp_tool_name in &self.mcp_tool_names {
            unregister_tool(mcp_tool_name);
        }
        Ok(())
    }
}

struct MCPTool {
    command: String,
    args: Vec<String>,
    tool: rmcp::model::Tool,
    info: ToolInfo,
}

impl MCPTool {
    fn new(name: String, command: String, args: Vec<String>, tool: rmcp::model::Tool) -> Self {
        let info = ToolInfo {
            name,
            description: tool.description.clone().unwrap_or_default().into_owned(),
            parameters: serde_json::to_value(&tool.input_schema).ok(),
        };
        Self {
            command,
            args,
            tool,
            info,
        }
    }

    async fn tool_call(
        &self,
        _ctx: AgentContext,
        value: AgentValue,
    ) -> Result<AgentValue, AgentError> {
        let service = ()
            .serve(
                TokioChildProcess::new(Command::new(&self.command).configure(|cmd| {
                    for arg in &self.args {
                        cmd.arg(arg);
                    }
                }))
                .map_err(|e| AgentError::Other(format!("Failed to start MCP process: {e}")))?,
            )
            .await
            .map_err(|e| AgentError::Other(format!("Failed to start MCP service: {e}")))?;

        let arguments = value.as_object().map(|obj| {
            obj.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        serde_json::to_value(v).unwrap_or(serde_json::Value::Null),
                    )
                })
                .collect::<serde_json::Map<String, serde_json::Value>>()
        });

        let tool_result = service
            .call_tool(CallToolRequestParam {
                name: self.tool.name.clone().into(),
                arguments,
            })
            .await
            .map_err(|e| {
                AgentError::Other(format!("Failed to call tool '{}': {e}", self.tool.name))
            })?;

        service
            .cancel()
            .await
            .map_err(|e| AgentError::Other(format!("Failed to cancel MCP service: {e}")))?;

        Ok(call_tool_result_to_agent_value(tool_result)?)
    }
}

#[async_trait]
impl Tool for MCPTool {
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
