<div align="center">

<img alt="logo" width="150" height="150" src="src-tauri/icons/Square150x150Logo.png">

<br>

<img alt="Modular Agent" width="353" height="50" src="doc/images/modular_agent_title.png">

<br>

![Developer Preview](https://img.shields.io/badge/Status-Developer_Preview-orange)
<!-- [![GitHub release](https://img.shields.io/github/v/release/modular-agent/modular-agent-desktop?style=flat)](https://github.com/modular-agent/modular-agent-desktop/releases) -->
<!-- [![GitHub downloads](https://img.shields.io/github/downloads/modular-agent/modular-agent-desktop/total?style=flat)](https://github.com/modular-agent/modular-agent-desktop/releases) -->

![Tauri 2](https://img.shields.io/badge/Tauri_2-24C8D8?logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-DEA584?logo=rust&logoColor=white)
![Svelte 5](https://img.shields.io/badge/Svelte_5-FF3E00?logo=svelte&logoColor=white)
![Windows](https://img.shields.io/badge/-Windows-0078D4?logo=windows&logoColor=white)
![macOS](https://img.shields.io/badge/-macOS-000000?logo=apple&logoColor=white)
![Linux](https://img.shields.io/badge/-Linux-FCC624?logo=linux&logoColor=black)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE_APACHE-2.0)

</div>

Modular Agent is a local-first desktop application for building AI workflows visually. Wire together 80+ pre-built agents — LLMs, databases, web scrapers, messaging — into pipelines that run continuously, streaming data in real time. No cloud or Docker required.

[English](README.md) | [日本語](README_ja.md)

<div align="center">
<img alt="Workflow Editor" width="800" src="doc/images/screenshot_editor.jpg">
</div>

## Features

### Agents

- ⚡ **Stream-Based Data Flow** — Real-time data streaming between agents
- 🤖 **80+ Built-in Agents** — LLM, Web/HTTP, Slack, SQL databases, screen capture, and more
- 🧩 **Extensible** — Add agent plugins via Rust crates

### Runtime

- 🏠 **Local Execution** — All processing happens on your machine; no cloud dependency
- 💻 **Cross-Platform** — Windows, macOS, Linux
- 📦 **Embeddable** — The core runtime ([modular-agent-core](https://github.com/modular-agent/modular-agent-core)) has minimal dependencies and can be embedded into various applications to run presets

### Editor

- 🎨 **Visual Workflow Editor** — Node-based drag-and-drop interface for designing agent pipelines
- 💾 **Preset Management** — Save, load, import/export workflow configurations
- 🚀 **Auto-Start** — Configure presets to run on app launch
- 🔲 **System Tray** — Run workflows in the background

## Getting Started

> **Developer Release** - Pre-built binaries are not yet available.

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.85+
- [Node.js](https://nodejs.org/) 20+
- [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/) (platform-specific dependencies)

### Build

```bash
npm install              # Install dependencies
npm run tauri build      # Build for production
```

After building, either copy the executable or run the installer:

- **Executable** - `src-tauri/target/release/modular-agent-desktop.exe` (Windows) / `modular-agent-desktop` (macOS/Linux)
- **Installer** - `src-tauri/target/release/bundle/msi/*.msi` (Windows) / `dmg/*.dmg` (macOS) / `deb/*.deb` (Linux)

### Run

1. Launch Modular Agent
2. Open or create a new preset
3. Right-click on the canvas and select "Add agent", or double-click to open the agent list
4. Connect agents by dragging between ports
5. Right-click and select "Play" (or press `Ctrl+.` / `Cmd+.`) to start the workflow

## Technical Overview

### How It Works

1. **Presets** are workflow configurations — a graph of connected agents
2. **Agents** are processing units (e.g., "Chat Completion", "HTTP Request", "Text Template")
3. Each agent has **input/output ports** — connect them by dragging between ports
4. Press **Play** to start the pipeline — data flows through agents in real time

### Architecture

- **Frontend** - [SvelteKit](https://svelte.dev/docs/kit/) (static adapter) + [Svelte 5](https://svelte.dev/), [TypeScript](https://www.typescriptlang.org/), [Tailwind CSS](https://tailwindcss.com/), [Svelte Flow](https://svelteflow.dev/), [shadcn-svelte](https://www.shadcn-svelte.com/)
- **Backend** - [Rust](https://www.rust-lang.org/) with [Tauri 2](https://v2.tauri.app/)
- **Core** - [`modular-agent-core`](https://github.com/modular-agent/modular-agent-core) agent runtime

```text
src/                    # Svelte frontend
  routes/               # SvelteKit pages
    preset_editor/      # Visual workflow editor
    settings/           # App settings
  lib/
    components/         # UI components
src-tauri/src/          # Rust backend
```

### Related Projects

- [modular-agent-core](https://github.com/modular-agent/modular-agent-core) - Modular Agent core runtime
- [tauri-plugin-modular-agent](https://github.com/modular-agent/tauri-plugin-modular-agent) - Tauri plugin

## Agent Plugins

The following agent crates are included by default:

| Crate | Description |
| ----- | ----------- |
| [modular-agent-std](https://github.com/modular-agent/modular-agent-std) | Standard utility agents |
| [modular-agent-llm](https://github.com/modular-agent/modular-agent-llm) | LLM integrations (OpenAI, Ollama) |
| [modular-agent-web](https://github.com/modular-agent/modular-agent-web) | HTTP, scraping, YouTube |
| [modular-agent-slack](https://github.com/modular-agent/modular-agent-slack) | Slack messaging |
| [modular-agent-sqlx](https://github.com/modular-agent/modular-agent-sqlx) | SQLite, MySQL, PostgreSQL |
| [modular-agent-lifelog](https://github.com/modular-agent/modular-agent-lifelog) | Screen capture, window tracking |

### Adding a New Agent Plugin

1. Add the dependency to `src-tauri/Cargo.toml`:

    ```toml
    [dependencies]
    modular-agent-foo = { git = "https://github.com/modular-agent/modular-agent-foo.git", tag = "v0.1.0" }
    ```

2. Import the crate in `src-tauri/src/lib.rs`:

    ```rust
    #[allow(unused_imports)]
    use modular_agent_foo;
    ```

3. Rebuild the application.

## Contributing

- ⭐ **Star to show support** — Helps the project reach more people
- 🤝 Pull requests welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) (CLA signature required)

## License

This project is licensed under the Apache License, Version 2.0.
