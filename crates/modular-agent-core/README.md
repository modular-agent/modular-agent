<div align="center">

<img alt="Modular Agent" width="150" height="150" src="https://raw.githubusercontent.com/modular-agent/modular-agent-core/main/doc/images/Square150x150Logo.png">
<br/>

<img alt="modular-agent-core" height="40" src="https://raw.githubusercontent.com/modular-agent/modular-agent-core/main/doc/images/modular_agent_core_title.svg">
<br/>
<br/>

![Language](https://img.shields.io/github/languages/top/modular-agent/modular-agent-core)
[![Crates.io](https://img.shields.io/crates/v/modular-agent-core.svg)](https://crates.io/crates/modular-agent-core)
[![Documentation](https://docs.rs/modular-agent-core/badge.svg)](https://docs.rs/modular-agent-core)
[![License](https://img.shields.io/crates/l/modular-agent-core.svg)](https://github.com/modular-agent/modular-agent-core#license)

[English](README.md) | [日本語](README_ja.md)

</div>

A Rust framework for building modular multi-agent systems with stream-based message orchestration.

## Features

### Agents

- **Stream-Based Data Flow** — Real-time data streaming between agents
- **Built-in Agents** — LLM, Web/HTTP, Slack, SQL databases, screen capture, and more (via [agent libraries](#related-repositories))
- **Extensible** — Add agent plugins via Rust crates

### Runtime

- **Local Execution** — All processing happens on your machine; no cloud dependency
- **Cross-Platform** — Windows, macOS, Linux
- **Embeddable** — Minimal dependencies; embed into CLI tools, desktop apps, servers, or any Rust application

## Overview

modular-agent-core provides an asynchronous, stream-based architecture for orchestrating multiple agents. Agents communicate through message passing and can be composed into networks using Presets. This is the core library with minimal dependencies—individual agent implementations are available separately.

## Installation

```toml
[dependencies]
modular-agent-core = "0.26"
```

To disable default features:

```toml
[dependencies]
modular-agent-core = { version = "0.26", default-features = false, features = ["llm"] }
```

## Quick Start

```rust
use modular_agent_core::{AgentError, AgentValue, ModularAgent, ModularAgentEvent};

#[tokio::main]
async fn main() -> Result<(), AgentError> {
    // 1. Initialize
    let ma = ModularAgent::init()?;
    ma.ready().await?;

    // 2. Subscribe to output BEFORE starting (avoid race condition)
    let mut rx = ma.subscribe_to_event(|event| {
        if let ModularAgentEvent::ExternalOutput(name, value) = event {
            if name == "output" { return Some(value); }
        }
        None
    });

    // 3. Load and start preset
    let preset_id = ma.open_preset_from_file("preset.json", None).await?;
    ma.start_preset(&preset_id).await?;

    // 4. Send input / receive output
    ma.write_external_input("input".into(), AgentValue::string("hello")).await?;
    if let Some(value) = rx.recv().await {
        println!("Output: {:?}", value);
    }

    // 5. Cleanup
    ma.stop_preset(&preset_id).await?;
    ma.quit();
    Ok(())
}
```

## Feature Flags

| Feature      | Default | Description                                     |
| ------------ | ------- | ----------------------------------------------- |
| `file`       | Yes     | File handling support for presets                |
| `image`      | Yes     | Image processing via photon-rs                  |
| `llm`        | Yes     | LLM integration with Message and ToolCall types |
| `mcp`        | Yes     | Model Context Protocol integration              |
| `mcp-server` | No      | Built-in MCP server (implies `file`)            |
| `test-utils` | No      | Testing utilities                               |

## External Agent Editing (MCP Server)

With the `mcp-server` feature, a host application can expose its running `ModularAgent` over a localhost MCP endpoint, so external AI agents such as Claude Code can inspect agent definitions, build and edit presets, and verify running flows through natural language.

```toml
modular-agent-core = { version = "0.26", features = ["mcp-server"] }
```

```rust
use modular_agent_core::mcp_server::{McpServerConfig, start_mcp_server};

// Serves streamable HTTP at http://127.0.0.1:8765/mcp (localhost only).
let handle = start_mcp_server(
    ma.clone(),
    McpServerConfig {
        port: 8765,
        // Root directory for the save_preset tool; None disables saving.
        presets_dir: Some("/path/to/presets".into()),
        // Required Bearer token; None disables authentication.
        token: Some("secret".into()),
    },
)
.await?;
// ...
handle.stop().await;
```

Connect from Claude Code:

```bash
claude mcp add --transport http modular-agent http://127.0.0.1:8765/mcp \
    --header "Authorization: Bearer secret"
```

Then ask, for example:

> Create a flow that listens to a Slack channel, sends each message to a Chat agent, and posts the reply back to the channel.

The external agent typically calls `list_agent_definitions` to discover the catalog, then `create_preset` → `add_agent` ×4 (Slack Listener, Slack To Message, Chat, Slack Post) → `add_connection` ×3 → `save_preset`. It can then verify the flow end to end: `start_preset`, feed a test value with `write_external_input`, and poll `get_external_outputs` / `get_agent_errors`. Both polling tools return `latest_seq` — the seq of the last record returned — which the agent passes back as `since_seq` on the next call to receive only new records; `dropped > 0` means the event collector fell behind the broadcast stream and some events were never captured. Separately, the capture buffer keeps only the most recent 200 records per kind, so records not polled in time can age out without affecting `dropped`. Structure changes emit `ModularAgentEvent::PresetStructureChanged` so hosts (e.g. modular-agent-desktop) can refresh their UI live. The full tool list (17 tools) covers definitions, preset CRUD, agent/connection editing, config updates, start/stop, and runtime verification.

The server binds `127.0.0.1` only. When `token` is set, every request must carry an `Authorization: Bearer <token>` header and is rejected with 401 otherwise; without a token the server is unauthenticated, so enable it deliberately. `modular-agent-desktop` exposes it via Settings → Core (with an auto-generated token); `modular-agent-cli` via the `--mcp-port <PORT>` and `--mcp-token <TOKEN>` flags.

## Documentation

Full API documentation is available at [docs.rs/modular-agent-core](https://docs.rs/modular-agent-core).

## Related Repositories

### Applications

- [modular-agent-desktop](https://github.com/modular-agent/modular-agent-desktop) - Visual presets editor (Tauri 2 + Svelte 5)

### Agent Libraries — General

- [modular-agent-std](https://github.com/modular-agent/modular-agent-std) - Standard utility agents (50+)
- [modular-agent-llm](https://github.com/modular-agent/modular-agent-llm) - OpenAI, Ollama integration

### Agent Libraries — Data Sources

- [modular-agent-lifelog](https://github.com/modular-agent/modular-agent-lifelog) - Screen capture, window tracking agents
- [modular-agent-slack](https://github.com/modular-agent/modular-agent-slack) - Slack messaging agents
- [modular-agent-web](https://github.com/modular-agent/modular-agent-web) - HTTP, scraping, search, YouTube agents

### Agent Libraries — Databases

- [modular-agent-duckdb](https://github.com/modular-agent/modular-agent-duckdb) - DuckDB analytics agents
- [modular-agent-lancedb](https://github.com/modular-agent/modular-agent-lancedb) - Vector database agents
- [modular-agent-mongodb](https://github.com/modular-agent/modular-agent-mongodb) - MongoDB CRUD agents
- [modular-agent-sqlx](https://github.com/modular-agent/modular-agent-sqlx) - SQLite, MySQL, PostgreSQL agents
- [modular-agent-surrealdb](https://github.com/modular-agent/modular-agent-surrealdb) - SurrealDB graph DB agents

### Plugins

- [tauri-plugin-modular-agent](https://github.com/modular-agent/tauri-plugin-modular-agent) - Tauri plugin bridge

## License

Dual-licensed under [Apache 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT).
