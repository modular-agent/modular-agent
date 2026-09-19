<div align="center">

<img alt="Modular Agent" width="150" height="150" src="https://raw.githubusercontent.com/modular-agent/modular-agent/main/crates/modular-agent-core/doc/images/Square150x150Logo.png">
<br/>

<img alt="modular-agent-core" height="40" src="https://raw.githubusercontent.com/modular-agent/modular-agent/main/crates/modular-agent-core/doc/images/modular_agent_core_title.svg">
<br/>
<br/>

![Language](https://img.shields.io/github/languages/top/modular-agent/modular-agent)
[![Crates.io](https://img.shields.io/crates/v/modular-agent-core.svg)](https://crates.io/crates/modular-agent-core)
[![Documentation](https://docs.rs/modular-agent-core/badge.svg)](https://docs.rs/modular-agent-core)
[![License](https://img.shields.io/crates/l/modular-agent-core.svg)](https://github.com/modular-agent/modular-agent#license)

[English](https://github.com/modular-agent/modular-agent/blob/main/crates/modular-agent-core/README.md) | [日本語](https://github.com/modular-agent/modular-agent/blob/main/crates/modular-agent-core/README_ja.md)

</div>

A Rust framework that composes modules into an agent with stream-based message orchestration.

## Overview

modular-agent-core is the orchestration engine of the [Modular Agent](https://github.com/modular-agent/modular-agent) project: modules are wired into **patches** — JSON-defined graphs of connected modules — and values stream through them asynchronously. This crate provides the runtime (module lifecycle, message routing, patch loading) and the `#[modular_agent]` macro for defining modules; it deliberately has minimal dependencies so it can be embedded into CLI tools, desktop apps, or servers.

Module implementations live in separate crates: [modular-agent-std](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-std) (utilities) and [modular-agent-llm](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-llm) (LLM integrations) in the same repository, plus a growing set of [module libraries](https://github.com/modular-agent/modular-agent#module-libraries) in their own repositories. The [Modular Agent desktop app](https://github.com/modular-agent/modular-agent/tree/main/apps/desktop) is a visual editor built on this crate.

## Installation

```toml
[dependencies]
modular-agent-core = "0.30"
```

To disable default features:

```toml
[dependencies]
modular-agent-core = { version = "0.30", default-features = false, features = ["llm"] }
```

## Quick Start

```rust
use std::time::Duration;

use modular_agent_core::{Error, Value, ModularAgent, ModularAgentEvent};

#[tokio::main]
async fn main() -> Result<(), Error> {
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

    // 3. Load and start patch
    let patch_id = ma.open_patch_from_file("patch.json", None).await?;
    ma.start_patch(&patch_id).await?;

    // 4. Send input / receive output
    ma.write_external_input("input".into(), Value::string("hello")).await?;
    if let Some(value) = rx.recv().await {
        println!("Output: {:?}", value);
    }

    // 5. Cleanup
    ma.shutdown(Duration::from_secs(5)).await?;
    Ok(())
}
```

## Concepts

### ModularAgent

`ModularAgent` is the orchestrator. `init()` collects every module definition registered in the binary; `ready().await` starts the runtime. Patches are loaded with `open_patch_from_file` (or built programmatically) and controlled with `start_patch` / `stop_patch`. `write_external_input` feeds values in, `subscribe_to_event` observes everything the engine emits (external outputs, module errors, structure changes), and `shutdown(timeout)` shuts the runtime down: it stops running patches, waits for their tasks, drains MCP connections, and errors with `Error::ShutdownTimeout` if the timeout elapses. `quit()` remains for tests and simple cases.

### Definitions and Specs

An **`ModuleDefinition`** is the blueprint the `#[modular_agent]` macro generates and registers: kind, title, description, category, UI hints, port lists, and config specs (each config key with its type and default). An **`ModuleSpec`** is one instance of a definition inside a patch: an id, the `def_name` referencing the definition, the instance's ports and config values, and editor metadata such as position.

When a patch written against older definitions is opened, `reconcile_spec()` migrates each spec: missing config keys are filled with the current defaults, keys that no longer exist in the definition are renamed with a `_` prefix (a module can still read them once in `new()` for lazy migration before they are stripped), and ports and config specs are overwritten from the current definition.

### Patch JSON

A patch is a JSON file with `modules` and `connections`. This is [`examples/patches/echo.json`](https://github.com/modular-agent/modular-agent/blob/main/crates/modular-agent-core/examples/patches/echo.json), which routes an external input straight to an external output:

```jsonc
{
  "id": "echo",
  "name": "Echo",
  "modules": [
    {
      "id": "in", // patch-local module id, referenced by connections
      "def_name": "modular_agent_core::external_module::ExternalInputModule",
      "outputs": ["value"], // this module's output ports
      "configs": { "name": "input" } // config values for this instance
    },
    {
      "id": "out",
      "def_name": "modular_agent_core::external_module::ExternalOutputModule",
      "inputs": ["value"], // this module's input ports
      "configs": { "name": "output" }
    }
  ],
  "connections": [
    {
      "source": "in", // source module id
      "source_handle": "value", // output port on the source
      "target": "out", // target module id
      "target_handle": "value" // input port on the target
    }
  ]
}
```

### Ports and `config:` Handles

Connections normally target an input port, but a `target_handle` of the form `config:<key>` streams values into the target's **config** instead — so a config value can be computed by the graph at runtime rather than set statically. For example, feeding a separator into a String Join module:

```json
{
  "source": "sep_input",
  "source_handle": "value",
  "target": "join",
  "target_handle": "config:sep"
}
```

### Value

`Value` is the value type that flows through connections. Cloning is cheap — large payloads are behind `Arc`, and collections are immutable (`im`) structures.

| Variant | Content |
| --- | --- |
| `Unit` | Empty value, used as a trigger signal |
| `Boolean` | `bool` |
| `Integer` | `i64` |
| `Number` | `f64` |
| `String` | UTF-8 string |
| `Image` | Image data (`image` feature) |
| `Array` | Ordered array of values |
| `Object` | String-keyed map of values |
| `Tensor` | `f32` tensor, for embeddings etc. |
| `Message` | LLM chat message (`llm` feature) |
| `Error` | An `Error` carried as a value |

### ModuleContext

Every external trigger creates a `ModuleContext` that travels with the values it produces, identifying one flow across modules end to end. It carries patch-scoped variables, a frame stack that tracks branching lineage through nested map operations (index and length per frame), and an optional `CancellationToken` for cancelling long-running work.

### Built-in External I/O Modules

Four built-in modules bridge the module network with the outside world:

| Module | Title | Role |
| --- | --- | --- |
| `ExternalInputModule` | `ExtIn->` | Entry point: forwards `write_external_input()` values for its configured `name` |
| `ExternalOutputModule` | `->ExtOut` | Exit point: emits `ModularAgentEvent::ExternalOutput` |
| `LocalInputModule` | `LocalIn->` | Patch-scoped local input |
| `LocalOutputModule` | `->LocalOut` | Patch-scoped local output |

### Registration

The `#[modular_agent]` macro registers each definition with the [inventory](https://crates.io/crates/inventory) crate at link time; `ModularAgent::init()` collects them all, so linking a module crate (a `use` is enough) is all it takes to make its modules available. One constraint follows: **exactly one copy of modular-agent-core may exist in the dependency graph**. Two copies mean two separate inventory registries, and modules registered in one are invisible to the other — module crates must depend on the same core (in the Modular Agent workspace, by path).

## Writing an Module

Define a struct with the `#[modular_agent]` macro and implement `AsModule`:

```rust
use modular_agent_core::{
    ModuleContext, ModuleData, Error, ModuleSpec, Value, AsModule, ModularAgent,
    async_trait, modular_agent,
};

/// Repeats the input string.
///
/// # Ports
/// - Input `input`: String to repeat
/// - Output `output`: The repeated string
///
/// # Configuration
/// - `count`: Number of repetitions
#[modular_agent(
    title = "Repeat",
    category = "Example",
    inputs = ["input"],
    outputs = ["output"],
    integer_config(name = "count", default = 2),
)]
struct RepeatModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for RepeatModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self, Error> {
        Ok(Self { data: ModuleData::new(ma, id, spec) })
    }

    async fn process(
        &mut self,
        ctx: ModuleContext,
        _port: String,
        value: Value,
    ) -> Result<(), Error> {
        let count = self.configs()?.get_integer_or("count", 2);
        let out = value.as_str().unwrap_or_default().repeat(count as usize);
        self.output(ctx, "output".into(), Value::string(out)).await
    }
}
```

- The `///` doc comment becomes the definition's `description`, rendered as markdown in the desktop app — write it for the person wiring up the workflow.
- Config macros: `string_config`, `integer_config`, `number_config`, `boolean_config`, `text_config`, `object_config`, `array_config`.
- Optional lifecycle methods: `start()`, `stop()`, and `configs_changed()` for reacting to runtime config edits.

Reference implementations: [modular-agent-std](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-std) and [modular-agent-llm](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-llm) in-tree, or [SlackPostModule](https://github.com/modular-agent/modular-agent-slack/blob/main/src/modules.rs) for a standalone module crate.

## Running a Patch

The bundled CLI example runs a patch with stdin/stdout wired to the named external channels (requires the default `file` feature):

```bash
cargo run --example cli -- examples/patches/echo.json -i input -o output
```

For real use, the [`ma` CLI](https://github.com/modular-agent/modular-agent/tree/main/apps/cli) is a full patch runner built on this crate:

```bash
echo "Hello" | ma ./patch.json
```

## Feature Flags

| Feature           | Default | Description                                              |
| ----------------- | ------- | -------------------------------------------------------- |
| `file`            | Yes     | File handling support for patches                        |
| `image`           | Yes     | Image processing via photon-rs                           |
| `llm`             | Yes     | LLM integration with Message and ToolCall types          |
| `mcp`             | Yes     | Model Context Protocol integration                       |
| `mcp-http-client` | No      | Streamable HTTP client transport for remote MCP servers  |
| `mcp-server`      | No      | Built-in MCP server (implies `file`)                     |
| `test-utils`      | No      | Testing utilities                                        |

## External Agent Editing (MCP Server)

With the `mcp-server` feature, a host application can expose its running `ModularAgent` over a localhost MCP endpoint, so external AI agents such as Claude Code can inspect module definitions, build and edit patches, and verify running flows through natural language.

```toml
modular-agent-core = { version = "0.30", features = ["mcp-server"] }
```

```rust
use modular_agent_core::mcp_server::{McpServerConfig, start_mcp_server};

// Serves streamable HTTP at http://127.0.0.1:8765/mcp (localhost only).
let handle = start_mcp_server(
    ma.clone(),
    McpServerConfig {
        port: 8765,
        // Root directory for the save_patch tool; None disables saving.
        patches_dir: Some("/path/to/patches".into()),
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

> Create a flow that listens to a Slack channel, sends each message to a Chat module, and posts the reply back to the channel.

The server exposes 17 tools:

- **Definitions** — `list_module_definitions`, `get_module_definition`
- **Patch CRUD** — `list_patches`, `create_patch`, `get_patch_spec`, `save_patch`
- **Module / connection editing** — `add_module`, `update_module_spec`, `set_module_configs`, `remove_module`, `add_connection`, `remove_connection`
- **Run & verify** — `start_patch`, `stop_patch`, `write_external_input`, `get_module_errors`, `get_external_outputs`

A typical session: `list_module_definitions` to discover the catalog, then `create_patch` → `add_module` ×4 (Slack Listener, Slack To Message, Chat, Slack Post) → `add_connection` ×3 → `save_patch`. The flow can then be verified end to end: `start_patch`, feed a test value with `write_external_input`, and poll `get_external_outputs` / `get_module_errors`. Both polling tools return `latest_seq` — the seq of the last record returned — which the client passes back as `since_seq` on the next call to receive only new records; `dropped > 0` means the event collector fell behind the broadcast stream and some events were never captured. Separately, the capture buffer keeps only the most recent 200 records per kind, so records not polled in time can age out without affecting `dropped`. Structure changes emit `ModularAgentEvent::PatchStructureChanged` so hosts (e.g. modular-agent-desktop) can refresh their UI live.

The server binds `127.0.0.1` only. When `token` is set, every request must carry an `Authorization: Bearer <token>` header and is rejected with 401 otherwise; without a token the server is unauthenticated, so enable it deliberately. `modular-agent-desktop` exposes it via Settings → Core (with an auto-generated token); `modular-agent-cli` via the `--mcp-port <PORT>` and `--mcp-token <TOKEN>` flags.

## Documentation

- API documentation: [docs.rs/modular-agent-core](https://docs.rs/modular-agent-core)
- Project documentation: [modular-agent.github.io/docs](https://modular-agent.github.io/docs/)

## Related Repositories

### Applications

- [modular-agent-desktop](https://github.com/modular-agent/modular-agent/tree/main/apps/desktop) - Visual patch editor (Tauri 2 + Svelte 5)
- [modular-agent-cli](https://github.com/modular-agent/modular-agent/tree/main/apps/cli) - `ma` command-line patch runner

### In-tree Module Libraries

- [modular-agent-std](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-std) - Standard utility modules (50+)
- [modular-agent-llm](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-llm) - LLM integrations (OpenAI, Claude, Ollama)

Many more module libraries — web, messaging, media, databases — live in their own repositories; see the [full list](https://github.com/modular-agent/modular-agent#module-libraries).

### Plugins

- [tauri-plugin-modular-agent](https://github.com/modular-agent/modular-agent/tree/main/crates/tauri-plugin-modular-agent) - Tauri plugin bridge

## License

Licensed under the [Apache License, Version 2.0](https://github.com/modular-agent/modular-agent/blob/main/LICENSE_APACHE-2.0).
