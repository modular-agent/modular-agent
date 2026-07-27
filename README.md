<div align="center">

<img alt="Modular Agent" width="150" height="150" src="https://raw.githubusercontent.com/modular-agent/modular-agent/main/crates/modular-agent-core/doc/images/Square150x150Logo.png">
<br/>
<br/>

![Language](https://img.shields.io/github/languages/top/modular-agent/modular-agent)
[![Crates.io](https://img.shields.io/crates/v/modular-agent-core.svg)](https://crates.io/crates/modular-agent-core)
[![License](https://img.shields.io/crates/l/modular-agent-core.svg)](#license)

</div>

Monorepo for the Modular Agent framework — a Rust framework for building modular
multi-agent systems with stream-based message orchestration.

## Contents

| Path | Crate / App | Description |
|---|---|---|
| `crates/modular-agent-core` | `modular-agent-core` | Orchestration engine, agent runtime, preset loader |
| `crates/modular-agent-macros` | `modular-agent-macros` | `#[modular_agent]` procedural macro |
| `crates/modular-agent-std` | `modular-agent-std` | Standard utility agents |
| `crates/modular-agent-llm` | `modular-agent-llm` | OpenAI / Claude / Ollama agents |
| `crates/modular-agent-web` | `modular-agent-web` | HTTP, scraping, search, YouTube agents |
| `crates/tauri-plugin-modular-agent` | `tauri-plugin-modular-agent` | Tauri plugin bridge (Rust + guest-js) |
| `apps/desktop` | `modular-agent-desktop` | Visual workflow editor (Tauri 2 + Svelte 5) |
| `apps/cli` | `modular-agent-cli` | `ma` command-line preset runner |
| `tools/ma-config` | `ma-config` | Agent selection / build configuration TUI |

Agent libraries outside this repository (databases, audio, VoiceVox, Slack, lifelog,
and others) live in their own repositories under
[github.com/modular-agent](https://github.com/modular-agent) and are consumed as
regular git or crates.io dependencies.

## Build

```sh
# Whole workspace
cargo check --workspace --all-targets

# Individual crate / app (always use -p for release builds; see below)
cargo build -p modular-agent-cli --release

# Desktop app
cd apps/desktop && npm install && npm run tauri dev
```

`ma-config` selects which agent crates each app links against and writes the
resulting dependency lists into the app manifests:

```sh
cargo run --manifest-path tools/ma-config/Cargo.toml -- desktop
cargo run --manifest-path tools/ma-config/Cargo.toml -- cli
```

> **Note** — build a single package with `-p` rather than `--workspace` for release
> artifacts. Cargo's v2 resolver unifies features across packages built together, so a
> `--workspace` build can enable features the app does not want.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE_APACHE-2.0](LICENSE_APACHE-2.0).
