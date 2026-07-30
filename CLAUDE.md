# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Project Overview

Modular Agent is a multi-agent orchestration framework for building AI-powered workflows.
Agents are composed into workflows through JSON preset configurations.

This repository is the monorepo holding the engine, the agent libraries every app needs,
both applications, and the build configurator. Agent libraries that only some builds
need live in their own repositories under `github.com/modular-agent` and are consumed as
git dependencies.

## Repository Structure

| Path | Crate / App | Tech Stack |
|------|-------------|------------|
| crates/modular-agent-core/ | Orchestration engine, agent runtime, preset loader | Rust |
| crates/modular-agent-macros/ | `#[modular_agent]` procedural macro | Rust |
| crates/modular-agent-std/ | Utility agents (50+) | Rust |
| crates/modular-agent-llm/ | OpenAI, Claude, Ollama integration | Rust |
| crates/modular-agent-web/ | HTTP, scraping, search, YouTube | Rust |
| crates/tauri-plugin-modular-agent/ | Tauri plugin bridge | Rust + TypeScript |
| apps/desktop/ | Visual workflow editor | Tauri 2 + Svelte 5 |
| apps/cli/ | `ma` preset runner | Rust |
| tools/ma-config/ | Agent selection / build configuration TUI | Rust |

Out-of-tree agent packages (separate repositories): sqlx, duckdb, mongodb, surrealdb,
cozodb, lancedb, audio, voicevox, slack, mattermost, lifelog, monty. Also separate:
`modular-agent-com` (homepage), `modular-agent-chatvrm` (avatar chat),
`modular-agent-doc` (documentation site), `browsing-recorder` (browser extension).

## Workspace Layout

- **One workspace, one `Cargo.lock`.** Both apps are workspace members, so `[patch]` at
  the root covers them both.
- **Versions are per crate.** core and macros are bumped together; std / llm / web /
  the plugin keep their own semver lines. `[workspace.dependencies]` carries
  `version` + `path` for each in-tree crate, so in-tree builds use the path and a
  published crate records the version.
- **A permanent `[patch.crates-io]`** redirects `modular-agent-core` and
  `tauri-plugin-modular-agent` to the in-tree crates. Out-of-tree agents depend on core
  via crates.io, and two linked copies of core mean two separate `inventory` registries —
  agents registered in one are invisible to the other. When core's minor version moves,
  every out-of-tree agent has to declare the new version before the workspace resolves
  to a single copy.
- **`tools/ma-config` is excluded from the workspace** so it still builds when the
  managed `[patch]` region points at a broken local path.

## Build Commands

```bash
# Whole workspace
cargo check --workspace --all-targets
cargo test --workspace --all-features

# One package — always use -p for release artifacts. The v2 resolver unifies
# features across packages built together, so --workspace release builds can
# enable features an app does not want.
cargo build -p modular-agent-cli --release

# Desktop app
cd apps/desktop && npm install && npm run tauri dev

# Format and lint
cargo fmt -p <package>
cargo clippy -p <package>

# Agent selection wizard (writes apps/<app>/ma-config.toml)
cargo run --manifest-path tools/ma-config/Cargo.toml -- desktop
cargo run --manifest-path tools/ma-config/Cargo.toml -- cli

# Generate title SVG (font: Funnel Display SemiBold, default size 30)
# apps/desktop uses --size 48
cd crates/modular-agent-core
uv run scripts/text_to_title.py \
    --font-file <path-to-FunnelDisplay-SemiBold.ttf> \
    --text "modular agent" [--size 48] \
    -o ../../<path>/doc/images/modular_agent_title.svg
```

## Agent Development Pattern

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

### Description from Doc Comments

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

### Description i18n Policy

Policy only — not implemented yet. Do not restructure descriptions in anticipation of it.

- English descriptions live in code doc comments and are the single source of truth. English text is never moved out into separate files.
- Translations will ship as package-bundled `locales/<lang>.json` catalogs (agent name → title / description / config descriptions), resolved by core as an overlay in `get_agent_definitions`. The `AgentDefinition` data model is unchanged; untranslated keys fall back to English.
- A catalog format (same shape as Node-RED / VS Code extensions) rather than per-agent markdown files, because the translation targets include `title` and config descriptions, not just `description`.

### UI Hints

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
| bg_color | integer (1-7) or "#rrggbb" | unset | Node background color. Body rendered at 85% opacity, title bar opaque |

Definition-level `hints` apply to all instances of the agent type.
Instance-level overrides use `AgentSpec.extensions`.

### Configuration Types

| Type | Macro | Example |
|------|-------|---------|
| String | `string_config` | `string_config(name = "url", default = "")` |
| Integer | `integer_config` | `integer_config(name = "limit", default = 100)` |
| Number | `number_config` | `number_config(name = "threshold", default = 0.5)` |
| Boolean | `boolean_config` | `boolean_config(name = "enabled", default = true)` |
| Text (multiline) | `text_config` | `text_config(name = "script")` |
| Object (JSON) | `object_config` | `object_config(name = "options", hidden = true)` |
| Array | `array_config` | `array_config(name = "items")` |

### Lifecycle Methods

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

### Accessing Configs

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

## Common Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| im | 15.1.0 | Immutable Vector/HashMap |
| serde / serde_json | latest | Serialization |
| async-trait | 0.1 | Async trait support |
| tokio | 1.48+ | Async runtime |

Shared external dependencies live in `[workspace.dependencies]`; crates whose feature
sets diverge (tokio, reqwest) keep their own entries.

## Rust Conventions

- Edition: 2024 (`edition.workspace = true` in every crate)
- Minimum Rust: 1.92.0
- Use `?` operator for error propagation
- Prefer `async fn` over manual Future
- Use `Arc<Mutex<T>>` for shared mutable state
- Port names as `static` constants at module top

## Formatting

The repository is fully formatted. Keep it that way — an unformatted edit shows up as
noise the next time the file is opened in an editor with format-on-save.

- **Rust**: `cargo fmt` (edition 2024).
- **JS / TS / Svelte / CSS**: prettier. `apps/desktop` and
  `crates/tauri-plugin-modular-agent` each carry a `.prettierrc` (printWidth 100, svelte
  plugin, import sorting) and a `.prettierignore`. Generated code is excluded: build
  output, `src-tauri/gen/`, and the shadcn-svelte components under
  `src/lib/components/ui/` — never format those, the CLI regenerates them.
- **Format only the files you changed.** A repo-wide `prettier --write` / `cargo fmt` is
  unnecessary and buries the real diff.
- Install the formatting hook in a new clone with:

  ```sh
  cp ~/.git-hooks/pre-commit .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
  ```

  It formats staged files, leaves partially-staged files alone and names them instead —
  format those by hand. Bypass with `git commit --no-verify`.
- **Formatting-only commits** are recorded in `.git-blame-ignore-revs`. Enable it once
  per clone: `git config blame.ignoreRevsFile .git-blame-ignore-revs`. When you make a
  new formatting-only commit, append its full 40-char SHA to that file.

## Tags

Component tags carry a prefix, since one repository now holds several release lines:
`core-v0.26.0`, `std-v0.16.0`, `llm-v0.14.0`, `web-v0.9.0`, `plugin-v0.17.0`,
`desktop-v0.18.0`, `cli-v0.4.0`. Tags from before the merge were rewritten with the same
prefixes.

## See Also

- `crates/modular-agent-core/CLAUDE.md` - Core engine details, AgentValue types
- `crates/modular-agent-std/CLAUDE.md` - Standard utility agents
- `crates/modular-agent-llm/CLAUDE.md` - LLM integration agents
- `apps/desktop/CLAUDE.md` - Desktop app architecture, ma-config
- `apps/cli/CLAUDE.md` - CLI runner
- `../modular-agent-com/design-brief.md` - Homepage design brief (Japanese)
