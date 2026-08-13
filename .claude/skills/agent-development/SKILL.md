---
name: agent-development
description: Modular Agent development reference — the #[modular_agent] macro, AsAgent trait and lifecycle, doc-comment rules for agent descriptions, UI hints, config types and access, AgentError handling, and the DB connection caching pattern. Use when writing, modifying, or reviewing agents in this workspace.
---

# Agent Development

All agents use the `#[modular_agent]` macro from `modular-agent-core`:

```rust
use modular_agent_core::{
    ModularAgent, AgentContext, AgentData, AgentError, AgentSpec, AgentValue, AsAgent,
    modular_agent, async_trait,
};

// Port name constants
const PORT_INPUT: &str = "input";
const PORT_OUTPUT: &str = "output";

#[modular_agent(
    title = "My Agent",
    category = "Category/Subcategory",
    inputs = [PORT_INPUT],
    outputs = [PORT_OUTPUT],
    string_config(name = "param", default = "value"),
    integer_config(name = "count", default = 10),
    boolean_config(name = "enabled", default = true),
)]
struct MyAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for MyAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self { data: AgentData::new(ma, id, spec) })
    }

    async fn process(&mut self, ctx: AgentContext, port: String, value: AgentValue) -> Result<(), AgentError> {
        // Process input and emit output
        self.output(ctx, PORT_OUTPUT.into(), result).await
    }
}
```

Conventions: port names as `static` constants at module top; `Arc<Mutex<T>>` for shared
mutable state; prefer `async fn` over manual Future.

## Description from Doc Comments

- `///` doc comments on the struct automatically become `AgentDefinition::description`; no need to use `description = "..."` in the macro attribute.
- Explicit `description = "..."` overrides the doc comment when both are present.
- Struct doc comments are **user-facing documentation**, rendered as-is in the UI (markdown-rendered in the desktop inspector and Settings → Agent screens). Write them for the person wiring up the workflow, not for the person maintaining the code.

All agents should have doc comments following this structure:

```rust
/// One-line summary of the agent.
///
/// Detailed description: behavior, features, edge cases, error conditions.
///
/// # Ports
/// - Input `message`: Description
/// - Output `response`: Description
///
/// # Configuration
/// - `model`: Description (default: "value")
/// - `stream`: Description
///
/// # Global Configuration
/// - `api_key`: Description
/// - `api_base`: Description
///
/// # Example
/// Given input `["a", "b"]` with separator `","`, outputs `"a,b"`.
```

**Rules:**

- `# Ports`: Required when the agent has input/output ports
- `# Configuration`: Required when the agent has configs
- `# Global Configuration`: Required when the agent uses `custom_global_config` / `string_global_config`
- `# Example`: Recommended when input/output transformation is non-trivial (optional)
- Edge cases and error conditions: Document in the detailed description when behavior is non-obvious
- External dependencies (API tokens, etc.): Document in `# Global Configuration` or a dedicated section
- Implementation notes do not belong in doc comments — no internal type/function references, no "why the code is written this way". Put those in `//` comments or on the `impl` block side
- Refer to configs and ports by their user-visible names (`sep`, `input`), never by the Rust constant names (`CONFIG_SEP`, `PORT_INPUT`)

**Reference implementations:** `ResponsesAgent` (`crates/modular-agent-llm/src/responses.rs`), `SlackPostAgent` (`modular-agent-slack/src/agents.rs`)

## Description i18n Policy

Policy only — not implemented yet. Do not restructure descriptions in anticipation of it.

- English descriptions live in code doc comments and are the single source of truth. English text is never moved out into separate files.
- Translations will ship as package-bundled `locales/<lang>.json` catalogs (agent name → title / description / config descriptions), resolved by core as an overlay in `get_agent_definitions`. The `AgentDefinition` data model is unchanged; untranslated keys fall back to English.
- A catalog format (same shape as Node-RED / VS Code extensions) rather than per-agent markdown files, because the translation targets include `title` and config descriptions, not just `description`.

## UI Hints

Use `hint(...)` to attach UI presentation metadata at the definition level:

```rust
#[modular_agent(
    title = "My Agent",
    category = "LLM",
    hint(color = 3, width = 2, height = 1),
)]
```

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| color | integer (1-7) | unset | Node color index |
| width | integer (>0) | 1 | Block width (pixels when `free_size` is set) |
| height | integer (>0) | 1 | Block height (pixels when `free_size` is set) |
| free_size | boolean | unset | Not tied to the grid: `width`/`height` are read as pixels, and neither resizing nor dragging snaps to the grid |
| no_resize | boolean | unset | Node cannot be resized (no resize handles) |
| background | boolean | unset | Render the node behind other nodes |
| bg_color | integer (1-7) or "#rrggbb" | unset | Node background color. Opaque by default; translucency comes from a UI-package `nodeStyles` entry |
| fg_color | integer (1-7) or "#rrggbb" | unset | Node foreground (text) color for the node body |

Definition-level `hints` apply to all instances of the agent type.
Instance-level overrides use `AgentSpec.extensions`.

## Configuration Types

| Type | Macro | Example |
|------|-------|---------|
| String | `string_config` | `string_config(name = "url", default = "")` |
| Integer | `integer_config` | `integer_config(name = "limit", default = 100)` |
| Number | `number_config` | `number_config(name = "threshold", default = 0.5)` |
| Boolean | `boolean_config` | `boolean_config(name = "enabled", default = true)` |
| Text (multiline) | `text_config` | `text_config(name = "script")` |
| Object (JSON) | `object_config` | `object_config(name = "options", hidden = true)` |
| Array | `array_config` | `array_config(name = "items")` |

## Lifecycle Methods

```rust
#[async_trait]
impl AsAgent for MyAgent {
    // Required: Constructor
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError>;

    // Optional: Called when agent starts
    async fn start(&mut self) -> Result<(), AgentError> { Ok(()) }

    // Optional: Called when agent stops
    async fn stop(&mut self) -> Result<(), AgentError> { Ok(()) }

    // Optional: Called when config changes at runtime
    fn configs_changed(&mut self) -> Result<(), AgentError> { Ok(()) }

    // Main processing logic
    async fn process(&mut self, ctx: AgentContext, port: String, value: AgentValue)
        -> Result<(), AgentError>;
}
```

## Accessing Configs

```rust
let config = self.configs()?;
let url = config.get_string_or_default("url");    // Returns empty string if not set
let limit = config.get_integer_or("limit", 100);  // Use get_*_or for custom default
let enabled = config.get_bool_or("enabled", true);
// Note: get_*_or_default() uses built-in defaults, get_*_or(key, default) for custom
```

## Error Handling

Use `AgentError` variants:

```rust
AgentError::InvalidValue("Invalid input format".into())
AgentError::InvalidConfig("Missing required config".into())
AgentError::IoError(format!("Network error: {}", e))
```

## Connection Caching Pattern (DB Agents)

All database agents use this pattern for connection pooling:

```rust
static CLIENT_MAP: OnceLock<Mutex<BTreeMap<String, Client>>> = OnceLock::new();

fn get_client_map() -> &'static Mutex<BTreeMap<String, Client>> {
    CLIENT_MAP.get_or_init(|| Mutex::new(BTreeMap::new()))
}

async fn get_client(url: &str) -> Result<Client, AgentError> {
    let mut map = get_client_map().lock().unwrap();
    if let Some(client) = map.get(url) {
        return Ok(client.clone());
    }
    let client = create_client(url).await?;
    map.insert(url.to_string(), client.clone());
    Ok(client)
}
```
