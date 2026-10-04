# CLAUDE.md

See root CLAUDE.md for common module development patterns.

## Overview

The `ma` binary: `ma serve` runs a workspace's auto-start patches as a server process, `ma run` wires one patch to stdin/stdout. User-facing docs are in `README.md`.

## Build Commands

```bash
# Build (modules are configured by ma-config wizard)
cargo build -p modular-agent-cli

# Run a patch / serve a workspace
cargo run -p modular-agent-cli -- run <patch.json>
cargo run -p modular-agent-cli -- serve --workspace ../../examples/workspace --listen 127.0.0.1:8080

# Tests (unit + tests/workspace_test.rs, which runs the binary against examples/workspace)
cargo test -p modular-agent-cli

# Configure modules with the ma-config wizard
cargo run --manifest-path ../../tools/ma-config/Cargo.toml -- cli

# Container image (from the repository root)
docker build -f apps/cli/Dockerfile -t modular-agent .
```

## Usage

```
ma [--log-format text|json] serve --workspace <dir> [--listen <addr:port>] [--allowed-host <host>]... [--shutdown-timeout 5s]
ma [--log-format text|json] run <patch> [--workspace <dir>] [-i <input>] [-o <output>] [--json] [--drain <duration>] [same server options]
```

Option tables and examples are in `README.md`.

## Behavior

- **Startup order** (`host.rs`): logger → `ModularAgent::init` → event-to-log task (subscribed before any patch starts) → workspace (`Workspace::open`, `apply_global_configs`; **every warning but `UnknownModule` is fatal**, unlike the desktop app — a module missing from this build is expected, since builds select different modules, so it is only logged) → `set_current_dir` into the workspace → bind `--listen` → subcommand opens/starts patches. Binding before patches start means a port conflict leaves nothing running.
- **Working directory**: with a workspace, relative paths in patches resolve against it (same rule as the desktop app). `ma run <file>` without `--workspace` keeps the caller's cwd.
- **Shutdown** (`Host::shutdown`): `McpServerHandle::stop` (ends SSE sessions, otherwise the graceful shutdown waits on them) → HTTP graceful shutdown → `ma.shutdown(--shutdown-timeout)` → event-to-log task drains what is queued. Signals: SIGTERM/SIGINT on unix, Ctrl+C on Windows.
- **`serve`**: any auto-start failure stops what started and exits 1 (restarts are the orchestrator's job). `/healthz` is 503 until the patches run.
- **`run`**: EOF starts shutdown immediately, which can cut off in-flight values; `--drain` keeps forwarding outputs for a while first. `--json` parses each line as JSON (malformed lines are logged and skipped).
- **Listening**: `/healthz` + `mcp_router` on one axum Router. Beyond loopback, `MA_TOKEN` is required (env only, never a flag) and becomes the MCP bearer token; `--allowed-host` extends rmcp's loopback-only Host check.

## Logging

- stderr only; stdout is `ma run`'s output channel. Default filter `warn,modular_agent::=info` (env_filter directives are target prefixes, so this excludes `modular_agent_core::*` and rmcp); `RUST_LOG` overrides.
- `events.rs` converts core events: `modular_agent::events` for lifecycle/errors (info/error), `ExternalOutput` (debug), `ModuleIn` (trace); `modular_agent::patch` for Log node records at the node's level. Records carry `patch` (name, or id when unnamed) and `module` (id) as `log` key-values. The module→patch index is built from patch specs under the **patch** lock only — a Log node emits while holding its module lock, so locking modules would stall the log.
- `--log-format json` (`logging.rs`): one object per line, `timestamp` / `severity` (`WARNING`, not `WARN`, for Cloud Logging; Trace → `DEBUG`) / `message` / `target`, then key-values flattened. Written field by field because `serde_json::Map` sorts keys and enabling `preserve_order` would change map ordering workspace-wide.

## Module Selection (ma-config)

Module crates are managed by the `ma-config` TUI wizard at the workspace root (`tools/ma-config/`), shared with the desktop app. Run it as `ma-config cli`. The wizard:

1. Selects which module crates to include
2. Selects per-crate Cargo features via MultiSelect (modules with `available_features` in their catalog entry)
3. Generates `Cargo.toml` dependencies and `src/modules.rs`

- Out-of-tree crates are linked from `custom_modules/<name>` at the workspace root (gitignored, cloned by hand), and only clones that are present are offered. The wizard fails pointing at `custom_modules/README.md` when a selected clone has gone missing, and with a fix-up message when a clone still depends on `modular-agent-core` from crates.io — a second copy of core means a second `inventory` registry and no visible modules.
- `modular-agent-core` is in-tree and always linked with the `mcp-server` feature; it is not selectable.
- In-tree modules (std / llm) are emitted as `{ workspace = true }`; a feature override spells out a path instead, because cargo ignores a member's `default-features = false` when the workspace entry does not set it.
- The catalog has two halves: `tools/ma-config/registry.yaml` holds the in-tree modules only (std / llm; override the path with `--registry <path>`), and every out-of-tree crate is read from the `ma-registry.yaml` at the root of its own clone under `custom_modules/`. The wizard scans that directory and merges both halves by name.
- `registry.yaml`'s root is `Registry { modules: [...] }`, a per-repo `ma-registry.yaml` is a single module at the root; both use `#[serde(deny_unknown_fields)]`. Optional module fields (`default_for`, `available_features`, `default_features`, `conflicts`) can be omitted, and a per-repo file's `name` must match its directory name. A clone with no `ma-registry.yaml` falls back to `name` / `description` from its `Cargo.toml` `[package]`.

Configuration is saved to `apps/cli/ma-config.toml` (gitignored), with local paths relative to the workspace root. Configs written before the monorepo are rejected rather than reinterpreted.

- `--config <path>` replaces that file for loading, saving and the conflict check (relative to the cwd); `apps/cli/ma-config.cloud.toml` is the committed container profile. `--apply` never rewrites the file it read (that only dropped comments). `--no-update` skips the closing `cargo update`, so the Docker build keeps the committed `Cargo.lock` apart from what the selection adds.
- Codegen always emits core with `mcp-server`, `mcp-http-client` and `workspace` (`AppKind::core_features`); both apps need all three.

- `crate_features` in `ma-config.toml` is `Option<Vec<String>>`: `None` (or omitted) = use crate defaults, `Some([])` = disable all features, `Some([...])` = explicit override with `default-features = false`.
- When the user's feature selection matches the crate's `default_features`, it is stored as `None` (no redundant output).
- TLS backend defaults to rustls where a crate offers the choice; a `default-tls` feature is available for native TLS.

Known conflicts, checked against the union of both apps' selections because the workspace resolves dependencies once for all members:
- `cozodb` + `sqlx`: sqlite3 native link conflict
- `duckdb` + `llm`: CRT mismatch on Windows

## Architecture

The CLI uses the `inventory` crate's compile-time registration. Each module crate is linked via `use` imports in `src/modules.rs` (auto-generated by ma-config), which causes `#[modular_agent]` registrations to be included in the binary. `ModularAgent::init()` then collects all registered modules.

## Files

- `src/main.rs` - clap subcommands and dispatch
- `src/host.rs` - shared startup/shutdown, `--listen` (`/healthz`, `/mcp`), signals
- `src/serve.rs`, `src/run.rs` - the two subcommands
- `src/logging.rs` - env_logger setup and the JSON line format
- `src/events.rs` - core events → log records
- `src/modules.rs` - Module crate imports (auto-generated by ma-config)
- `tests/workspace_test.rs` - runs the binary against a copy of `examples/workspace`
- `Dockerfile` - image build; `ma-config.cloud.toml` - the module selection it applies (committed, unlike `ma-config.toml`)
- `Cargo.toml` - Dependencies (managed by ma-config)
- `../../tools/ma-config/` - TUI wizard, shared with the desktop app
