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
use std::sync::atomic::{AtomicBool, Ordering};
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
        let info = ToolInfo::new(
            name,
            tool.description.clone().unwrap_or_default().into_owned(),
            serde_json::to_value(&tool.input_schema).ok(),
        );
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
    /// A transport-level failure invalidates the pooled connection,
    /// reconnects, and retries the call exactly once.
    async fn tool_call(
        &self,
        _ctx: AgentContext,
        value: AgentValue,
    ) -> Result<AgentValue, AgentError> {
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

        let entry = {
            let mut pool = connection_pool().lock().await;
            pool.get_or_create(&self.server_name, &self.server_config)
                .await?
        };

        let tool_result = match self.call_once(&entry, arguments.clone()).await {
            Ok(result) => result,
            Err(e) => {
                log::warn!(
                    "MCP tool call '{}' failed ({}); reconnecting to server '{}' and retrying",
                    self.tool.name,
                    e,
                    self.server_name
                );
                entry.dead.store(true, Ordering::Release);
                let entry = {
                    let mut pool = connection_pool().lock().await;
                    pool.invalidate(&self.server_name);
                    pool.get_or_create(&self.server_name, &self.server_config)
                        .await?
                };
                // Mark the replacement dead too so the next caller reconnects
                // immediately instead of paying a guaranteed-failed call first.
                self.call_once(&entry, arguments)
                    .await
                    .inspect_err(|_| entry.dead.store(true, Ordering::Release))?
            }
        };

        call_tool_result_to_agent_value(tool_result)
    }

    /// Performs a single tool call on the given pooled connection.
    ///
    /// Returns `Err` only for transport/protocol-level failures (missing
    /// service or an `Err` from the rmcp call); tool-level failures arrive
    /// as `Ok` with `is_error` set and must not trigger a reconnect.
    async fn call_once(
        &self,
        entry: &PoolEntry,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, AgentError> {
        let connection = entry.conn.lock().await;
        let service = connection.service.as_ref().ok_or_else(|| {
            AgentError::Other(format!(
                "MCP service for '{}' is not available (tool '{}')",
                self.server_name, self.info.name
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
                AgentError::Other(format!("Failed to call MCP tool '{}': {e}", self.info.name))
            })
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

/// A pooled connection together with its liveness flag.
///
/// The `dead` flag lives outside the connection mutex so that pool
/// operations can check and set liveness without waiting behind an
/// in-flight tool call holding the connection lock.
#[derive(Clone)]
struct PoolEntry {
    conn: Arc<AsyncMutex<MCPConnection>>,
    dead: Arc<AtomicBool>,
}

/// Connection pool for managing MCP server connections.
///
/// Maintains persistent connections to MCP servers and reuses them
/// across multiple tool calls for efficiency.
struct MCPConnectionPool {
    /// Map of server names to their connections.
    connections: HashMap<String, PoolEntry>,
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
    /// A live existing connection is reused. A connection marked dead or
    /// whose service is gone is discarded and replaced with a fresh one.
    async fn get_or_create(
        &mut self,
        server_name: &str,
        config: &MCPServerConfig,
    ) -> Result<PoolEntry, AgentError> {
        if let Some(entry) = self.connections.get(server_name) {
            // try_lock only: a busy connection has an in-flight call, which
            // implies its service is still present, so treat it as live
            // rather than blocking the whole pool behind that call.
            let service_gone = entry
                .conn
                .try_lock()
                .map(|c| c.service.is_none())
                .unwrap_or(false);
            if !entry.dead.load(Ordering::Acquire) && !service_gone {
                log::debug!("Reusing existing MCP connection for '{}'", server_name);
                return Ok(entry.clone());
            }
            log::info!(
                "Discarding dead MCP connection for '{}', creating a new one",
                server_name
            );
            if let Some(stale) = self.connections.remove(server_name) {
                cancel_in_background(stale, server_name.to_string());
            }
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

        let entry = PoolEntry {
            conn: Arc::new(AsyncMutex::new(MCPConnection {
                service: Some(service),
            })),
            dead: Arc::new(AtomicBool::new(false)),
        };
        self.connections
            .insert(server_name.to_string(), entry.clone());
        Ok(entry)
    }

    /// Removes the server's pooled connection if it has been marked dead.
    ///
    /// The dead-flag guard makes late invalidations from concurrently failed
    /// calls no-ops once a healthy replacement connection is in the pool.
    fn invalidate(&mut self, server_name: &str) {
        let is_dead = self
            .connections
            .get(server_name)
            .is_some_and(|e| e.dead.load(Ordering::Acquire));
        if is_dead && let Some(entry) = self.connections.remove(server_name) {
            log::info!("Invalidating MCP connection for '{}'", server_name);
            cancel_in_background(entry, server_name.to_string());
        }
    }

    /// Removes and returns all pooled connections.
    ///
    /// Synchronous on purpose: callers hold the pool lock, and the pool lock
    /// must never be held across an await on a connection lock (an in-flight
    /// rmcp call has no default timeout, so that wait could be unbounded).
    fn take_all(&mut self) -> Vec<(String, PoolEntry)> {
        self.connections.drain().collect()
    }
}

/// Cancels the entry's service in a background task.
///
/// Cancellation needs the connection lock, which may be held by an in-flight
/// call for its full duration; a spawned task keeps pool operations (which
/// may hold the pool lock) from ever waiting on a connection lock.
fn cancel_in_background(entry: PoolEntry, server_name: String) {
    entry.dead.store(true, Ordering::Release);
    tokio::spawn(async move {
        let mut connection = entry.conn.lock().await;
        if let Some(service) = connection.service.take()
            && let Err(e) = service.cancel().await
        {
            log::warn!("Failed to cancel MCP service '{}': {}", server_name, e);
        }
    });
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
/// The pool remains usable after this call: a tool call running concurrently
/// with the drain may re-create a connection, which is only cleaned up by a
/// subsequent call to this function.
///
/// Connections still busy past the shutdown timeout are cancelled best-effort
/// in background tasks, which may not complete if the tokio runtime is torn
/// down immediately afterwards.
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
    let entries = { connection_pool().lock().await.take_all() };
    for (name, entry) in entries {
        entry.dead.store(true, Ordering::Release);
        // Bound the wait: an in-flight rmcp call holds the connection lock
        // with no default request timeout, so a wedged server could otherwise
        // block shutdown forever. Lock via a clone so `entry` stays movable
        // into the busy-connection fallback below.
        let conn = entry.conn.clone();
        match tokio::time::timeout(std::time::Duration::from_secs(5), conn.lock()).await {
            Ok(mut connection) => {
                if let Some(service) = connection.service.take() {
                    if let Err(e) = service.cancel().await {
                        log::error!("Failed to cancel MCP service '{}': {}", name, e);
                    } else {
                        log::debug!("Successfully shut down MCP server '{}'", name);
                    }
                }
            }
            Err(_) => {
                log::warn!(
                    "MCP connection '{}' busy during shutdown; cancelling in background",
                    name
                );
                cancel_in_background(entry, name);
            }
        }
    }
    log::info!("All MCP server connections shut down");
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
    let entry = {
        let mut pool = connection_pool().lock().await;
        pool.get_or_create(&server_name, &server_config).await?
    };

    // List all available tools from this server
    log::debug!("Listing tools from MCP server '{}'", server_name);
    let tools_list = {
        let connection = entry.conn.lock().await;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_entry(dead: bool) -> PoolEntry {
        PoolEntry {
            conn: Arc::new(AsyncMutex::new(MCPConnection { service: None })),
            dead: Arc::new(AtomicBool::new(dead)),
        }
    }

    fn bogus_config() -> MCPServerConfig {
        MCPServerConfig {
            command: "modular-agent-test-nonexistent-command".to_string(),
            args: Vec::new(),
            env: None,
        }
    }

    #[tokio::test]
    async fn invalidate_removes_dead_entry() {
        let mut pool = MCPConnectionPool::new();
        pool.connections.insert("s".to_string(), test_entry(true));
        pool.invalidate("s");
        assert!(pool.connections.is_empty());
    }

    #[tokio::test]
    async fn invalidate_keeps_entry_not_marked_dead() {
        // Guards the race where a late invalidation from a concurrently failed
        // call must not destroy a healthy replacement connection.
        let mut pool = MCPConnectionPool::new();
        pool.connections.insert("s".to_string(), test_entry(false));
        pool.invalidate("s");
        assert!(pool.connections.contains_key("s"));
    }

    #[tokio::test]
    async fn get_or_create_reuses_busy_connection() {
        let mut pool = MCPConnectionPool::new();
        let entry = test_entry(false);
        pool.connections.insert("s".to_string(), entry.clone());
        // Hold the connection lock to simulate an in-flight call: the pool
        // must treat a busy connection as live and return it without
        // awaiting the lock (a bogus config would make any spawn fail).
        let _guard = entry.conn.try_lock().unwrap();
        let got = pool.get_or_create("s", &bogus_config()).await.unwrap();
        assert!(Arc::ptr_eq(&got.conn, &entry.conn));
    }

    #[tokio::test]
    async fn get_or_create_discards_dead_entry() {
        let mut pool = MCPConnectionPool::new();
        pool.connections.insert("s".to_string(), test_entry(true));
        // The Err from the bogus command proves a fresh spawn was attempted
        // instead of reusing the dead entry.
        assert!(pool.get_or_create("s", &bogus_config()).await.is_err());
        assert!(pool.connections.is_empty());
    }

    #[tokio::test]
    async fn get_or_create_discards_entry_with_missing_service() {
        let mut pool = MCPConnectionPool::new();
        pool.connections.insert("s".to_string(), test_entry(false));
        assert!(pool.get_or_create("s", &bogus_config()).await.is_err());
        assert!(pool.connections.is_empty());
    }
}
