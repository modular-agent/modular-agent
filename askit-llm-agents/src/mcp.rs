#![cfg(feature = "mcp")]

use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentData, AgentDefinition, AgentError, AgentOutput,
    AgentValue, AsAgent, AsAgentData, async_trait, new_agent_boxed,
};
use rmcp::{
    model::{CallToolRequestParam, CallToolResult},
    service::ServiceExt,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use tokio::process::Command;

// MCP Agent
pub struct MCPCallAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for MCPCallAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        data: AgentData,
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

        let arguments = data.as_object().map(|obj| {
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
            PORT_OBJECT,
            call_tool_result_to_agent_data(tool_result.clone())?,
        )?;

        let response = serde_json::to_string_pretty(&tool_result).map_err(|e| {
            AgentError::Other(format!(
                "Failed to serialize tool result content to JSON: {e}"
            ))
        })?;
        self.try_output(ctx, PORT_RESPONSE, AgentData::string(response))?;

        Ok(())
    }
}

fn call_tool_result_to_agent_data(result: CallToolResult) -> Result<AgentData, AgentError> {
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
    let data = AgentData::array("string", contents);
    if result.is_error == Some(true) {
        return Err(AgentError::Other(
            serde_json::to_string(&data.value)
                .map_err(|e| AgentError::InvalidValue(e.to_string()))?,
        ));
    }
    Ok(data)
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "LLM";

static PORT_OBJECT: &str = "object";
static PORT_RESPONSE: &str = "response";

static CONFIG_COMMAND: &str = "command";
static CONFIG_ARGS: &str = "args";
static CONFIG_TOOL: &str = "tool";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "llm_mcp_call",
            Some(new_agent_boxed::<MCPCallAgent>),
        )
        // .use_native_thread()
        .title("MCP Call")
        .category(CATEGORY)
        .inputs(vec![PORT_OBJECT])
        .outputs(vec![PORT_OBJECT, PORT_RESPONSE])
        .string_config_default(CONFIG_COMMAND)
        .string_config_default(CONFIG_ARGS)
        .string_config_default(CONFIG_TOOL),
    );
}
