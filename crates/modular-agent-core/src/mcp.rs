//! Model Context Protocol (MCP) integration for external tool servers.
//!
//! This module provides integration with MCP-compliant tool servers, allowing
//! external tools to be registered and called through the standard tool registry.
//!
//! MCP is a protocol for connecting LLM applications with external tool providers.
//! This module supports loading MCP server configurations from JSON files
//! (compatible with Claude Desktop format) and manages connection pooling
//! for efficient server communication.
//!
//! # Features
//!
//! - Load MCP server configurations from JSON files
//! - Automatic connection pooling for MCP servers
//! - Register MCP tools with the global tool registry
//! - Graceful shutdown of all MCP connections
//!
//! # Example
//!
//! ```no_run
//! use modular_agent_core::mcp::{register_tools_from_mcp_json, shutdown_all_mcp_connections};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Load and register tools from MCP configuration
//!     let tools = register_tools_from_mcp_json("mcp.json").await?;
//!     println!("Registered {} MCP tools", tools.len());
//!
//!     // ... use tools ...
//!
//!     // Clean up connections on shutdown
//!     shutdown_all_mcp_connections().await?;
//!     Ok(())
//! }
//! ```

#![cfg(feature = "mcp")]

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use modular_agent_core::{AgentContext, AgentError, AgentValue, async_trait};
use rmcp::{
    model::{CallToolRequestParam, CallToolResult},
    service::ServiceExt,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde::Deserialize;
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;

use crate::tool::{Tool, ToolInfo, register_tool};

/// Tool implementation that delegates to an MCP server.
///
/// Uses connection pooling to efficiently reuse connections to MCP servers.
struct MCPTool {
    /// Name of the MCP server this tool belongs to.
    server_name: String,
    /// Configuration for connecting to the MCP server.
    server_config: MCPServerConfig,
    /// The underlying MCP tool definition.
    tool: rmcp::model::Tool,
    /// Tool metadata for registration.
    info: ToolInfo,
}

impl MCPTool {
    /// Creates a new MCPTool from server configuration and tool definition.
    ///
    /// # Arguments
    ///
    /// * `name` - The fully qualified tool name (typically "server::tool")
    /// * `server_name` - Name of the MCP server
    /// * `server_config` - Configuration for the MCP server
    /// * `tool` - The MCP tool definition
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

    /// Invokes the tool on the MCP server.
    ///
    /// Gets or creates a connection from the pool and calls the tool.
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
                    name: self.tool.name.clone(),
                    arguments,
                    task: None,
                })
                .await
                .map_err(|e| {
                    AgentError::Other(format!("Failed to call tool '{}': {e}", self.tool.name))
                })?
        };

        call_tool_result_to_agent_value(tool_result)
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

/// Root configuration structure for MCP servers.
///
/// Compatible with the Claude Desktop MCP configuration format (`mcp.json`).
///
/// # Example JSON
///
/// ```json
/// {
///   "mcpServers": {
///     "filesystem": {
///       "command": "npx",
///       "args": ["-y", "@anthropic/mcp-server-filesystem", "/path/to/dir"]
///     }
///   }
/// }
/// ```
#[derive(Debug, Deserialize)]
pub struct MCPConfig {
    /// Map of server names to their configurations.
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, MCPServerConfig>,
}

/// Configuration for a single MCP server.
///
/// Specifies how to start the MCP server process.
#[derive(Debug, Clone, Deserialize)]
pub struct MCPServerConfig {
    /// The command to execute (e.g., "npx", "node", "python").
    pub command: String,

    /// Arguments to pass to the command.
    pub args: Vec<String>,

    /// Optional environment variables for the process.
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

/// Type alias for a running MCP service connection.
type MCPService = rmcp::service::RunningService<rmcp::service::RoleClient, ()>;

/// A single connection to an MCP server.
struct MCPConnection {
    /// The running service, or None if not connected.
    service: Option<MCPService>,
}

/// Connection pool for managing MCP server connections.
///
/// Maintains persistent connections to MCP servers and reuses them
/// across multiple tool calls for efficiency.
struct MCPConnectionPool {
    /// Map of server names to their connections.
    connections: HashMap<String, Arc<AsyncMutex<MCPConnection>>>,
}

impl MCPConnectionPool {
    /// Creates a new empty connection pool.
    fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    /// Gets an existing connection or creates a new one for the server.
    ///
    /// If a connection already exists for the server, it is reused.
    /// Otherwise, a new MCP server process is started.
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

    /// Shuts down all connections in the pool.
    ///
    /// Cancels all running MCP services and clears the connection map.
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

/// Global connection pool instance.
static CONNECTION_POOL: OnceLock<AsyncMutex<MCPConnectionPool>> = OnceLock::new();

/// Returns the global connection pool, initializing it if necessary.
fn connection_pool() -> &'static AsyncMutex<MCPConnectionPool> {
    CONNECTION_POOL.get_or_init(|| AsyncMutex::new(MCPConnectionPool::new()))
}

/// Shuts down all MCP server connections.
///
/// Call this during application shutdown to cleanly terminate all
/// MCP server processes.
///
/// # Example
///
/// ```no_run
/// use modular_agent_core::mcp::shutdown_all_mcp_connections;
///
/// #[tokio::main]
/// async fn main() {
///     // ... use MCP tools ...
///
///     // Clean shutdown
///     shutdown_all_mcp_connections().await.expect("Failed to shutdown MCP");
/// }
/// ```
pub async fn shutdown_all_mcp_connections() -> Result<(), AgentError> {
    log::info!("Shutting down all MCP server connections");
    connection_pool().lock().await.shutdown_all().await?;
    log::info!("All MCP server connections shut down successfully");
    Ok(())
}

/// Registers all tools from a single MCP server.
///
/// Connects to the MCP server, lists its available tools, and registers
/// each one with the global tool registry.
///
/// # Arguments
///
/// * `server_name` - Name of the MCP server
/// * `server_config` - Configuration for the MCP server
///
/// # Returns
///
/// A vector of registered tool names in the format "server_name::tool_name".
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
/// use modular_agent_core::mcp::register_tools_from_mcp_json;
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

/// Converts an MCP tool call result to an AgentValue.
///
/// Extracts text content from the result and returns it as an array.
/// If the result indicates an error, returns an AgentError instead.
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
