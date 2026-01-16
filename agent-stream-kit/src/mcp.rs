#![cfg(feature = "mcp")]

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use agent_stream_kit::{AgentContext, AgentError, AgentValue, async_trait};
use rmcp::{
    model::{CallToolRequestParam, CallToolResult},
    service::ServiceExt,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde::Deserialize;
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;

use crate::tool::{Tool, ToolInfo, register_tool};

/// MCP Tool with connection pool support
struct MCPTool {
    server_name: String,
    server_config: MCPServerConfig,
    tool: rmcp::model::Tool,
    info: ToolInfo,
}

impl MCPTool {
    fn new(
        name: String,
        server_name: String,
        server_config: MCPServerConfig,
        tool: rmcp::model::Tool,
    ) -> Self {
        let info = ToolInfo {
            name,
            description: tool.description.clone().unwrap_or_default().into_owned(),
            parameters: serde_json::to_value(&tool.input_schema).ok(),
        };
        Self {
            server_name,
            server_config,
            tool,
            info,
        }
    }

    async fn tool_call(
        &self,
        _ctx: AgentContext,
        value: AgentValue,
    ) -> Result<AgentValue, AgentError> {
        // Get or create connection from pool
        let conn = {
            let mut pool = connection_pool().lock().await;
            pool.get_or_create(&self.server_name, &self.server_config)
                .await?
        };

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

        let tool_result = {
            let connection = conn.lock().await;
            let service = connection.service.as_ref().ok_or_else(|| {
                AgentError::Other(format!(
                    "MCP service for '{}' is not available",
                    self.server_name
                ))
            })?;
            service
                .call_tool(CallToolRequestParam {
                    name: self.tool.name.clone().into(),
                    arguments,
                    task: None,
                })
                .await
                .map_err(|e| {
                    AgentError::Other(format!("Failed to call tool '{}': {e}", self.tool.name))
                })?
        };

        Ok(call_tool_result_to_agent_value(tool_result)?)
    }
}

#[async_trait]
impl Tool for MCPTool {
    fn info(&self) -> &ToolInfo {
        &self.info
    }

    async fn call(&self, ctx: AgentContext, args: AgentValue) -> Result<AgentValue, AgentError> {
        self.tool_call(ctx, args).await
    }
}

/// Structure representing the Claude Desktop MCP configuration format
#[derive(Debug, Deserialize)]
pub struct MCPConfig {
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, MCPServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MCPServerConfig {
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

type MCPService = rmcp::service::RunningService<rmcp::service::RoleClient, ()>;

/// Connection pool entry for an MCP server
struct MCPConnection {
    service: Option<MCPService>,
}

/// Connection pool for MCP servers
struct MCPConnectionPool {
    connections: HashMap<String, Arc<AsyncMutex<MCPConnection>>>,
}

impl MCPConnectionPool {
    fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    async fn get_or_create(
        &mut self,
        server_name: &str,
        config: &MCPServerConfig,
    ) -> Result<Arc<AsyncMutex<MCPConnection>>, AgentError> {
        // Check if connection already exists
        if let Some(conn) = self.connections.get(server_name) {
            log::debug!("Reusing existing MCP connection for '{}'", server_name);
            return Ok(conn.clone());
        }

        log::info!(
            "Starting MCP server '{}' (command: {})",
            server_name,
            config.command
        );

        // Start new MCP service
        let service = ()
            .serve(
                TokioChildProcess::new(Command::new(&config.command).configure(|cmd| {
                    for arg in &config.args {
                        cmd.arg(arg);
                    }
                    if let Some(env) = &config.env {
                        for (key, value) in env {
                            cmd.env(key, value);
                        }
                    }
                }))
                .map_err(|e| {
                    log::error!("Failed to start MCP process for '{}': {}", server_name, e);
                    AgentError::Other(format!(
                        "Failed to start MCP process for '{}': {e}",
                        server_name
                    ))
                })?,
            )
            .await
            .map_err(|e| {
                log::error!("Failed to start MCP service for '{}': {}", server_name, e);
                AgentError::Other(format!(
                    "Failed to start MCP service for '{}': {e}",
                    server_name
                ))
            })?;

        log::info!("Successfully started MCP server '{}'", server_name);

        let connection = MCPConnection {
            service: Some(service),
        };

        let conn_arc = Arc::new(AsyncMutex::new(connection));
        self.connections
            .insert(server_name.to_string(), conn_arc.clone());
        Ok(conn_arc)
    }

    async fn shutdown_all(&mut self) -> Result<(), AgentError> {
        let count = self.connections.len();
        log::debug!("Shutting down {} MCP server connection(s)", count);

        for (name, conn) in self.connections.drain() {
            log::debug!("Shutting down MCP server '{}'", name);
            let mut connection = conn.lock().await;
            if let Some(service) = connection.service.take() {
                service.cancel().await.map_err(|e| {
                    log::error!("Failed to cancel MCP service '{}': {}", name, e);
                    AgentError::Other(format!("Failed to cancel MCP service: {e}"))
                })?;
                log::debug!("Successfully shut down MCP server '{}'", name);
            }
        }
        Ok(())
    }
}

// Global connection pool
static CONNECTION_POOL: OnceLock<AsyncMutex<MCPConnectionPool>> = OnceLock::new();

fn connection_pool() -> &'static AsyncMutex<MCPConnectionPool> {
    CONNECTION_POOL.get_or_init(|| AsyncMutex::new(MCPConnectionPool::new()))
}

/// Shuts down all MCP server connections in the pool
pub async fn shutdown_all_mcp_connections() -> Result<(), AgentError> {
    log::info!("Shutting down all MCP server connections");
    connection_pool().lock().await.shutdown_all().await?;
    log::info!("All MCP server connections shut down successfully");
    Ok(())
}

/// Registers tools from a single MCP server
///
/// # Arguments
/// * `server_name` - Name of the MCP server
/// * `server_config` - Configuration for the MCP server
///
/// # Returns
/// A vector of registered tool names in the format "server_name::tool_name"
async fn register_tools_from_server(
    server_name: String,
    server_config: MCPServerConfig,
) -> Result<Vec<String>, AgentError> {
    log::debug!("Registering tools from MCP server '{}'", server_name);

    // Get or create connection from pool
    let conn = {
        let mut pool = connection_pool().lock().await;
        pool.get_or_create(&server_name, &server_config).await?
    };

    // List all available tools from this server
    log::debug!("Listing tools from MCP server '{}'", server_name);
    let tools_list = {
        let connection = conn.lock().await;
        let service = connection.service.as_ref().ok_or_else(|| {
            log::error!("MCP service for '{}' is not available", server_name);
            AgentError::Other(format!(
                "MCP service for '{}' is not available",
                server_name
            ))
        })?;
        service.list_tools(Default::default()).await.map_err(|e| {
            log::error!("Failed to list MCP tools for '{}': {}", server_name, e);
            AgentError::Other(format!(
                "Failed to list MCP tools for '{}': {e}",
                server_name
            ))
        })?
    };

    let mut registered_tool_names = Vec::new();

    // Register all tools from this server using connection pool
    for tool_info in tools_list.tools {
        let mcp_tool_name = format!("{}::{}", server_name, tool_info.name);
        registered_tool_names.push(mcp_tool_name.clone());

        register_tool(MCPTool::new(
            mcp_tool_name.clone(),
            server_name.clone(),
            server_config.clone(),
            tool_info,
        ));
        log::debug!("Registered MCP tool '{}'", mcp_tool_name);
    }

    log::info!(
        "Registered {} tools from MCP server '{}'",
        registered_tool_names.len(),
        server_name
    );

    Ok(registered_tool_names)
}

/// Loads MCP configuration from a JSON file and registers all tools
///
/// # Arguments
/// * `json_path` - Path to the mcp.json file
///
/// # Returns
/// A vector of registered tool names in the format "server_name::tool_name"
///
/// # Example
/// ```no_run
/// use agent_stream_kit::mcp::register_tools_from_mcp_json;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let tool_names = register_tools_from_mcp_json("mcp.json").await?;
///     println!("Registered {} tools", tool_names.len());
///     Ok(())
/// }
/// ```
pub async fn register_tools_from_mcp_json<P: AsRef<Path>>(
    json_path: P,
) -> Result<Vec<String>, AgentError> {
    let path = json_path.as_ref();
    log::info!("Loading MCP configuration from: {}", path.display());

    // Read the JSON file
    let json_content = std::fs::read_to_string(path).map_err(|e| {
        log::error!("Failed to read MCP config file '{}': {}", path.display(), e);
        AgentError::Other(format!("Failed to read MCP config file: {e}"))
    })?;

    // Parse the JSON
    let config: MCPConfig = serde_json::from_str(&json_content).map_err(|e| {
        log::error!("Failed to parse MCP config JSON: {}", e);
        AgentError::Other(format!("Failed to parse MCP config JSON: {e}"))
    })?;

    log::info!("Found {} MCP servers in config", config.mcp_servers.len());

    let mut registered_tool_names = Vec::new();

    // Iterate through each MCP server
    for (server_name, server_config) in config.mcp_servers {
        let tools = register_tools_from_server(server_name, server_config).await?;
        registered_tool_names.extend(tools);
    }

    log::info!(
        "Successfully registered {} MCP tools total",
        registered_tool_names.len()
    );

    Ok(registered_tool_names)
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
