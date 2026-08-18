<div align="center">

<img alt="Modular Agent" width="150" height="150" src="https://raw.githubusercontent.com/modular-agent/modular-agent/main/crates/modular-agent-core/doc/images/Square150x150Logo.png">
<br/>
<br/>

<img alt="Modular Agent" width="343" height="60" src="https://raw.githubusercontent.com/modular-agent/modular-agent/main/apps/desktop/doc/images/modular_agent_title.svg">
<br>
<br>

![Developer Preview](https://img.shields.io/badge/Status-Developer_Preview-orange)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE_APACHE-2.0)
[![Crates.io](https://img.shields.io/crates/v/modular-agent-core.svg)](https://crates.io/crates/modular-agent-core)
[![Documentation](https://docs.rs/modular-agent-core/badge.svg)](https://docs.rs/modular-agent-core)

![Tauri 2](https://img.shields.io/badge/Tauri_2-24C8D8?logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-DEA584?logo=rust&logoColor=white)
![Svelte 5](https://img.shields.io/badge/Svelte_5-FF3E00?logo=svelte&logoColor=white)
![Windows](https://img.shields.io/badge/-Windows-0078D4?logo=windows&logoColor=white)
![macOS](https://img.shields.io/badge/-macOS-000000?logo=apple&logoColor=white)
![Linux](https://img.shields.io/badge/-Linux-FCC624?logo=linux&logoColor=black)

[English](README.md) | [日本語](README_ja.md)

</div>

Build AI workflows like a modular synth — patch extensible agents together visually into real-time pipelines. LLMs, databases, web scrapers, messaging, and more. Privacy-first, no cloud required.

<div align="center">
<img alt="Workflow Editor" width="800" src="https://raw.githubusercontent.com/modular-agent/modular-agent/main/apps/desktop/doc/images/screenshot_editor.jpg">
</div>

Modular Agent is a desktop app and Rust framework for composing AI agents into workflows the way you wire modules on a synthesizer. Drop agents onto a canvas, connect their ports, and flip the Run switch — values stream through the graph in real time, and a patch keeps running like a live pipeline rather than a one-shot script. Everything executes on your machine; LLM endpoints, databases, and messaging services are reached only when you patch them in.

## How It Works

- **Patch** — a workflow saved as JSON: a set of agents plus the connections between them.
- **Agent** — a processing unit instantiated from an agent definition, with named input/output ports and a config.
- **Connection** — wires an output port to an input port. Targeting the special `config:<key>` handle streams values into an agent's config instead.
- Values (`AgentValue`) stream through the graph asynchronously; each external trigger carries an `AgentContext` that identifies its flow across agents end to end.

Running patches can also be inspected and edited live by external AI agents (e.g. Claude Code) through the built-in [MCP server](crates/modular-agent-core/README.md#external-agent-editing-mcp-server).

The [core README](crates/modular-agent-core/README.md) covers these concepts in depth; the [documentation site](https://modular-agent.github.io/docs/) walks through building your first patches.

## Documentation

> **Developer Preview** — pre-built binaries are not yet available; the docs walk through building from source.

- **[Documentation site](https://modular-agent.github.io/docs/)** — [Installation](https://modular-agent.github.io/docs/getting-started/installation/), [Your First Patch](https://modular-agent.github.io/docs/getting-started/first-patch/), [Using the Chat Agent](https://modular-agent.github.io/docs/getting-started/chat-patch/)
- [Desktop app](apps/desktop) — the visual patch editor
- [`ma` CLI](apps/cli) — run patches from the command line
- [modular-agent-core](crates/modular-agent-core) — the engine as an embeddable library ([crates.io](https://crates.io/crates/modular-agent-core) / [docs.rs](https://docs.rs/modular-agent-core))
- [tauri-plugin-modular-agent](crates/tauri-plugin-modular-agent) — embed the engine in your own Tauri app
- [Agent libraries](#agent-libraries) — everything that can be patched in; [custom_agents/README.md](custom_agents/README.md) explains how out-of-tree packages are built
- [CONTRIBUTING.md](CONTRIBUTING.md) / [GitHub Discussions](https://github.com/orgs/modular-agent/discussions)

## Contents

| Path | App / Crate | Description |
|---|---|---|
| [`apps/desktop`](apps/desktop) | `modular-agent-desktop` | Visual workflow editor (Tauri 2 + Svelte 5) |
| [`apps/cli`](apps/cli) | `modular-agent-cli` | `ma` command-line patch runner |
| [`crates/modular-agent-core`](crates/modular-agent-core) | `modular-agent-core` | Orchestration engine, agent runtime, patch loader |
| [`crates/modular-agent-macros`](crates/modular-agent-macros) | `modular-agent-macros` | `#[modular_agent]` procedural macro |
| [`crates/modular-agent-std`](crates/modular-agent-std) | `modular-agent-std` | Standard utility agents |
| [`crates/modular-agent-llm`](crates/modular-agent-llm) | `modular-agent-llm` | OpenAI / Claude / Ollama agents |
| [`crates/tauri-plugin-modular-agent`](crates/tauri-plugin-modular-agent) | `tauri-plugin-modular-agent` | Tauri plugin bridge (Rust + guest-js) |
| [`tools/ma-config`](tools/ma-config) | `ma-config` | Agent selection / build configuration TUI |

## Agent Libraries

Agents come in packages. `std` and `llm` live in this repository and are part of every build. The rest live in their own repositories under [github.com/modular-agent](https://github.com/modular-agent): clone the ones you want into `custom_agents/` and select them with the ma-config wizard — see [custom_agents/README.md](custom_agents/README.md) for details.

| Category | Package | Agents |
|---|---|---|
| In-tree | [modular-agent-std](crates/modular-agent-std) | Standard utilities: arrays, strings, templates, files, timers, filters (50+) |
| In-tree | [modular-agent-llm](crates/modular-agent-llm) | LLM integrations: OpenAI, Claude, Ollama |
| General | [modular-agent-web](https://github.com/modular-agent/modular-agent-web) | Web/HTTP, scraping, search, YouTube agents |
| General | [modular-agent-monty](https://github.com/modular-agent/modular-agent-monty) | Monty script agents |
| General | [modular-agent-zapcode](https://github.com/modular-agent/modular-agent-zapcode) | ZapCode TypeScript script agents |
| Messaging | [modular-agent-slack](https://github.com/modular-agent/modular-agent-slack) | Slack messaging agents |
| Messaging | [modular-agent-mattermost](https://github.com/modular-agent/modular-agent-mattermost) | Mattermost messaging agents |
| Data / Media | [modular-agent-lifelog](https://github.com/modular-agent/modular-agent-lifelog) | Screen capture, window tracking agents |
| Databases | [modular-agent-sqlx](https://github.com/modular-agent/modular-agent-sqlx) | SQL database agents (PostgreSQL, MySQL, SQLite) |

Nothing is limited to this organization — any repository holding an agent crate can be cloned into `custom_agents/` and picked up by the wizard.

## Contributing

- ⭐ **Star to show support** — helps the project reach more people
- 🤝 Pull requests welcome — see [CONTRIBUTING.md](CONTRIBUTING.md), or start a thread in [GitHub Discussions](https://github.com/orgs/modular-agent/discussions)

## License

This project is licensed under the [Apache License, Version 2.0](LICENSE_APACHE-2.0).
