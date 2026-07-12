//! End-to-end MCP connection lifecycle tests (harness = false).
//!
//! The test binary doubles as the MCP server: when spawned with the
//! `MOCK_MCP_SERVER` environment variable set, `main` runs a minimal stdio
//! JSON-RPC server instead of the test driver. This avoids depending on an
//! external runtime (node/python) while still exercising real child
//! processes through rmcp's `TokioChildProcess` transport.
//!
//! The mock server appends a `START <pid>` line to the file named by
//! `MOCK_MCP_LOG` on startup, so the driver can count how many server
//! processes the connection pool spawned and verify reconnect behavior
//! ("exactly once") as well as child-process reclamation on shutdown.

extern crate modular_agent_core as ma;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ma::mcp::{register_tools_from_mcp_json, shutdown_all_mcp_connections};
use ma::tool::{Tool, get_tool};
use ma::{AgentContext, AgentError, AgentValue, ModularAgent};

fn main() {
    if std::env::var("MOCK_MCP_SERVER").is_ok() {
        mock_server::run();
        return;
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--list") {
        // Mimic libtest's --list output so tooling that enumerates tests works.
        println!("mcp_lifecycle: test");
        return;
    }
    // Respect a libtest-style name filter so `cargo test <other_test>` does
    // not drag this suite in. Skip values of value-taking libtest flags so an
    // IDE-supplied `-- --format json` is not mistaken for a name filter.
    const VALUE_FLAGS: &[&str] = &[
        "--test-threads",
        "--skip",
        "--logfile",
        "--color",
        "--format",
        "--shuffle-seed",
    ];
    let mut i = 0;
    let mut filter = None;
    while i < args.len() {
        let a = &args[i];
        if VALUE_FLAGS.contains(&a.as_str()) {
            i += 2;
            continue;
        }
        if a.starts_with('-') {
            i += 1;
            continue;
        }
        filter = Some(a);
        break;
    }
    if let Some(filter) = filter
        && !"mcp_lifecycle".contains(filter.as_str())
    {
        return;
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(run_scenarios());
    println!("test mcp_lifecycle ... ok");
}

async fn run_scenarios() {
    let scratch = std::env::temp_dir().join(format!("ma-mcp-lifecycle-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let log_path = scratch.join("spawns.log");
    let config_path = write_mcp_json(&scratch, &log_path);

    let tools = register_tools_from_mcp_json(&config_path).await.unwrap();
    for name in ["mock::ping", "mock::fail", "mock::die_now"] {
        assert!(tools.contains(&name.to_string()), "missing tool {name}");
    }
    assert_eq!(spawned_pids(&log_path).len(), 1);

    let ping = get_tool("mock::ping").unwrap();
    let fail = get_tool("mock::fail").unwrap();
    let die_now = get_tool("mock::die_now").unwrap();

    // A healthy cached connection is reused across calls.
    for _ in 0..2 {
        assert_pong(&call(&ping).await.unwrap());
    }
    assert_eq!(
        spawned_pids(&log_path).len(),
        1,
        "healthy connection must be reused without reconnecting"
    );

    // A tool-level error (is_error = true) surfaces as Err to the caller but
    // must not invalidate the connection.
    assert!(call(&fail).await.is_err());
    assert_pong(&call(&ping).await.unwrap());
    assert_eq!(
        spawned_pids(&log_path).len(),
        1,
        "tool-level errors must not trigger a reconnect"
    );

    // die_now kills the server without responding: the transport failure
    // triggers one reconnect, the retried call kills the new server too, and
    // the error is returned to the caller.
    assert!(call(&die_now).await.is_err());
    assert_eq!(
        spawned_pids(&log_path).len(),
        2,
        "transport failure must reconnect exactly once"
    );

    // The retry failure marked the replacement connection dead; the next call
    // must discard it, reconnect exactly once, and succeed.
    assert_pong(&call(&ping).await.unwrap());
    assert_eq!(
        spawned_pids(&log_path).len(),
        3,
        "recovery from a dead pooled connection must spawn exactly one new server"
    );

    // A server that dies out-of-band (pool entry NOT marked dead) is
    // detected by the failed call, reconnected exactly once, and the
    // retried call succeeds transparently.
    let pid = *spawned_pids(&log_path).last().unwrap();
    kill_process(pid);
    wait_until_dead(pid).await;
    assert_pong(&call(&ping).await.unwrap());
    assert_eq!(
        spawned_pids(&log_path).len(),
        4,
        "out-of-band server death must reconnect exactly once"
    );

    // ModularAgent::shutdown drains the pool and reclaims the child process.
    let pid = *spawned_pids(&log_path).last().unwrap();
    assert!(process_alive(pid), "server process should be running");
    let magent = ModularAgent::init().unwrap();
    magent.shutdown().await.unwrap();
    wait_until_dead(pid).await;

    // The pool was drained, so the next call establishes a fresh connection.
    assert_pong(&call(&ping).await.unwrap());
    assert_eq!(spawned_pids(&log_path).len(), 5);

    let pid = *spawned_pids(&log_path).last().unwrap();
    shutdown_all_mcp_connections().await.unwrap();
    wait_until_dead(pid).await;

    std::fs::remove_dir_all(&scratch).ok();
}

fn write_mcp_json(scratch: &Path, log_path: &Path) -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let config = serde_json::json!({
        "mcpServers": {
            "mock": {
                "command": exe.to_string_lossy(),
                "args": [],
                "env": {
                    "MOCK_MCP_SERVER": "1",
                    "MOCK_MCP_LOG": log_path.to_string_lossy(),
                }
            }
        }
    });
    let path = scratch.join("mcp.json");
    std::fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
    path
}

async fn call(tool: &Arc<Box<dyn Tool + Send + Sync>>) -> Result<AgentValue, AgentError> {
    tokio::time::timeout(
        Duration::from_secs(30),
        tool.call(AgentContext::new(), AgentValue::object_default()),
    )
    .await
    .expect("MCP tool call timed out")
}

fn assert_pong(value: &AgentValue) {
    let contents = value.as_array().expect("expected array result");
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0].as_str(), Some("pong"));
}

/// Returns the pids of all mock server processes spawned so far, in order.
fn spawned_pids(log_path: &Path) -> Vec<u32> {
    std::fs::read_to_string(log_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.strip_prefix("START "))
        .filter_map(|pid| pid.trim().parse().ok())
        .collect()
}

#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    let output = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).contains(&format!(" {pid} "))
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn kill_process(pid: u32) {
    std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
        .unwrap();
}

#[cfg(unix)]
fn kill_process(pid: u32) {
    std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()
        .unwrap();
}

async fn wait_until_dead(pid: u32) {
    for _ in 0..100 {
        if !process_alive(pid) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("mock MCP server process {pid} still alive after shutdown");
}

/// Minimal MCP stdio server: newline-delimited JSON-RPC over stdin/stdout.
mod mock_server {
    use std::io::{BufRead, Write};

    use serde_json::{Value, json};

    pub(crate) fn run() {
        log_start();
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            // Messages without an id are notifications and need no response.
            let Some(id) = msg.get("id").cloned() else {
                continue;
            };
            let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
            let result = match method {
                "initialize" => json!({
                    "protocolVersion": msg
                        .pointer("/params/protocolVersion")
                        .cloned()
                        .unwrap_or_else(|| json!("2025-03-26")),
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "mock-mcp-server", "version": "0.0.0"},
                }),
                "tools/list" => json!({
                    "tools": [
                        {"name": "ping", "inputSchema": {"type": "object"}},
                        {"name": "fail", "inputSchema": {"type": "object"}},
                        {"name": "die_now", "inputSchema": {"type": "object"}},
                    ],
                }),
                "tools/call" => {
                    match msg.pointer("/params/name").and_then(Value::as_str) {
                        Some("ping") => json!({
                            "content": [{"type": "text", "text": "pong"}],
                            "isError": false,
                        }),
                        Some("fail") => json!({
                            "content": [{"type": "text", "text": "mock tool failure"}],
                            "isError": true,
                        }),
                        // Simulate a crashed server: exit without responding.
                        _ => std::process::exit(1),
                    }
                }
                _ => json!({}),
            };
            let response = json!({"jsonrpc": "2.0", "id": id, "result": result});
            let mut out = stdout.lock();
            writeln!(out, "{response}").ok();
            out.flush().ok();
        }
    }

    /// Appends a `START <pid>` line to the spawn log so the test driver can
    /// count server processes and check their liveness.
    fn log_start() {
        let Ok(path) = std::env::var("MOCK_MCP_LOG") else {
            return;
        };
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
        {
            writeln!(file, "START {}", std::process::id()).ok();
        }
    }
}
