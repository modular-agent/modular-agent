<div align="center">

<img alt="logo" width="150" height="150" src="../../crates/modular-agent-core/doc/images/Square150x150Logo.png">
<br>

<img alt="Modular Agent" width="343" height="60" src="doc/images/modular_agent_title.svg">
<br>
<br>

![Developer Preview](https://img.shields.io/badge/Status-Developer_Preview-orange)

![Tauri 2](https://img.shields.io/badge/Tauri_2-24C8D8?logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-DEA584?logo=rust&logoColor=white)
![Svelte 5](https://img.shields.io/badge/Svelte_5-FF3E00?logo=svelte&logoColor=white)
![Windows](https://img.shields.io/badge/-Windows-0078D4?logo=windows&logoColor=white)
![macOS](https://img.shields.io/badge/-macOS-000000?logo=apple&logoColor=white)
![Linux](https://img.shields.io/badge/-Linux-FCC624?logo=linux&logoColor=black)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](../../LICENSE_APACHE-2.0)

</div>

Build AI workflows like a modular synth — patch extensible agents together visually into real-time pipelines. LLMs, databases, web scrapers, messaging, and more. Privacy-first, no cloud required.

[English](README.md) | [日本語](README_ja.md)

<div align="center">
<img alt="Workflow Editor" width="800" src="doc/images/screenshot_editor.jpg">
</div>

Modular Agent Desktop is the visual editor for [Modular Agent](../../README.md) patches. This README is aimed at developers building and extending the app — for **usage**, head to the [documentation site](https://modular-agent.github.io/docs/): [Installation](https://modular-agent.github.io/docs/getting-started/installation/), [Your First Patch](https://modular-agent.github.io/docs/getting-started/first-patch/), [Using the Chat Agent](https://modular-agent.github.io/docs/getting-started/chat-patch/).

## Features

### Agents

- ⚡ **Stream-Based Data Flow** — Real-time data streaming between agents
- 🤖 **Agent Libraries** — LLM, Web/HTTP, messaging, databases, screen capture, and more; see the [full list](../../README.md#agent-libraries)
- 🧩 **Extensible** — Add agent packages as Rust crates, with optional [custom node UIs](#custom-node-uis)

### Runtime

- 🏠 **Local Execution** — All processing happens on your machine; no cloud dependency
- 💻 **Cross-Platform** — Windows, macOS, Linux
- 📦 **Embeddable Core** — The runtime ([modular-agent-core](../../crates/modular-agent-core)) is a standalone crate this app embeds via [tauri-plugin-modular-agent](../../crates/tauri-plugin-modular-agent)

### Editor

- 🎨 **Visual Workflow Editor** — Node-based drag-and-drop interface for designing agent pipelines
- 🏃 **Run Switch** — Start and stop a patch from the titlebar or patch list (`Ctrl+.` / `Cmd+.`); running patches keep processing in the background
- 🗂️ **Multi-Tab Editing** — Every open patch keeps its own live editor; tab switching is instant
- ↩️ **Undo / Redo** — Command-pattern history covering node, connection, and config edits
- ⌨️ **Customizable Shortcuts** — All hotkeys rebindable in Settings, including Quick Add slots for placing frequent agents
- 💾 **Patch Management** — Save, organize in folders, import/export as JSON
- 🚀 **Auto-Start** — Configure patches to run on app launch
- 🔲 **System Tray** — Keep workflows running with the window closed
- 🔌 **MCP Server** — Let external AI agents (e.g. Claude Code) inspect and edit patches live

## Getting Started

> **Developer Release** — Pre-built binaries are not yet available; the app is built from source. The [Installation guide](https://modular-agent.github.io/docs/getting-started/installation/) covers the same steps in more detail.

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) and platform-specific dependencies — see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Node.js](https://nodejs.org/)

### Build

From `apps/desktop` in a checkout of the monorepo:

```bash
npm install              # Install dependencies
npm run tauri dev        # Run in development mode
npm run tauri build      # Build for production
```

Cargo artifacts land in the workspace-level `target/` directory at the repository root:

- **Executable** — `target/release/modular-agent-desktop.exe` (Windows) / `modular-agent-desktop` (macOS/Linux)
- **Installer** — `target/release/bundle/nsis/*-setup.exe` (Windows) / `dmg/*.dmg` (macOS) / `deb/*.deb` (Linux)

Other npm scripts: `npm run check` (svelte-check), `npm run format` (prettier), `npm test` (vitest).

## Custom Build (ma-config)

The default build links the in-tree agent crates ([modular-agent-std](../../crates/modular-agent-std), [modular-agent-llm](../../crates/modular-agent-llm)). More agent packages — web scraping, messaging, databases, screen capture, script agents — are added at build time:

1. Clone the agent repositories you want into `custom_agents/` at the repository root. See [custom_agents/README.md](../../custom_agents/README.md) for the repository list.
2. Run the **ma-config** TUI wizard to select agents and per-crate features (only cloned agents are offered):

   ```bash
   cargo run --manifest-path ../../tools/ma-config/Cargo.toml -- desktop
   ```

3. Rebuild with `npm run tauri dev` or `npm run tauri build`.

How it works:

- The wizard updates `src-tauri/Cargo.toml` and regenerates `src-tauri/src/agents.rs` (never edit that file by hand). In-tree agents are emitted as `{ workspace = true }`; out-of-tree agents as `path = "../../../custom_agents/<name>"`, which makes each clone a member of the workspace.
- Your selection is saved to `apps/desktop/ma-config.toml` (gitignored) and reused on later runs. `--apply` re-runs the codegen from that saved selection without the interactive wizard.
- Each out-of-tree package describes itself in an `ma-registry.yaml` at its repository root — description, selectable Cargo features, defaults, and conflicts with other packages. The wizard checks conflicts across both apps (desktop and CLI), because the workspace resolves dependencies once for all members.

## Architecture

- **Frontend** — [SvelteKit](https://svelte.dev/docs/kit/) (static adapter) + [Svelte 5](https://svelte.dev/), [TypeScript](https://www.typescriptlang.org/), [Tailwind CSS 4](https://tailwindcss.com/), [Svelte Flow](https://svelteflow.dev/), [shadcn-svelte](https://www.shadcn-svelte.com/)
- **Backend** — [Rust](https://www.rust-lang.org/) with [Tauri 2](https://v2.tauri.app/)
- **Core** — [modular-agent-core](../../crates/modular-agent-core) agent runtime, accessed through [tauri-plugin-modular-agent](../../crates/tauri-plugin-modular-agent)

```text
src/                          # Svelte frontend
  routes/
    patch_editor/             # editor page shell
    open_patches/             # patch file browser
    settings/                 # app settings
    logs/                     # log viewer
  lib/
    hotkeys.ts                # keyboard shortcut definitions and matching
    shared.svelte.ts          # global agent event bus
    tab-store.svelte.ts       # multi-tab state
    modular_agent.ts          # low-level Tauri invoke wrappers
    components/
      patch-editor/           # editor internals
        context.svelte.ts     # EditorState: per-tab state + operations
        history.svelte.ts     # command-pattern undo/redo
        editor-canvas.svelte  # Svelte Flow canvas, keyboard dispatch
        agent-node.svelte     # agent node rendering
      agent-list/             # agent category tree popup
      ui/                     # shadcn-svelte components (generated)
src-tauri/src/                # Rust backend
  modular_agent_desktop/
    app.rs                    # patch management state (ModularAgentApp)
    settings.rs               # core settings, agent global configs
    observer.rs               # engine events → Tauri events
    tray.rs, window.rs, autostart.rs, shortcut.rs
```

### Frontend ↔ Backend

The frontend calls Rust commands through Tauri's `invoke()` (wrapped in `src/lib/modular_agent.ts`), which reach the engine via `tauri-plugin-modular-agent`. On the way back, `observer.rs` subscribes to engine `ModularAgentEvent`s and relays them as Tauri events: `ma:agent_config_updated`, `ma:agent_error`, `ma:agent_in`, `ma:agent_spec_updated` for per-agent activity, and `ma:patch_list_changed`, `ma:patch_structure_changed`, `ma:patch_removed`, `ma:patch_renamed`, `ma:patch_running_changed` for patch lifecycle. Every payload carries an `origin` (`"desktop"`, `"mcp"`, …) so the frontend can tell its own echoes from external edits.

### Editor Internals

- Each tab owns a long-lived `EditorState` (`context.svelte.ts`); inactive tabs stay mounted and keep receiving events, so switching tabs is instant.
- Undo/redo is a command history (`history.svelte.ts`): every edit is a Command object that knows how to execute, undo, and remap backend-assigned IDs.
- `shared.svelte.ts` is the global event bus: Tauri listeners update per-agent state regardless of which tab is visible.
- External edits (an MCP agent, another window) merge into an open canvas as a diff: origin-based self-echo filtering plus `reconcileFlow`, preserving undo history and selection.

[CLAUDE.md](CLAUDE.md) documents these mechanisms in implementation-level detail.

## Patches on Disk

Patches are JSON files under `~/.modular_agent/patches/`. The folder hierarchy becomes the patch name (`Music/Sampler` is `Sampler` in a `Music` folder), and the sidebar tracks filesystem changes live. The File menu imports and exports individual patch files.

## Settings

- **Core settings auto-save** on change. The one exception is the global "Show App Window" shortcut, which has a manual Save button because it only takes effect on app restart.
- **Keyboard shortcuts** — every editor hotkey is rebindable; Quick Add slots (`mod+1`–`mod+5`) bind both a key and an agent type.
- **Auto-start** — mark patches to run on app launch.
- **System tray** — closing the window keeps the app and running patches alive in the tray.

## MCP Server

The core engine ships a built-in MCP server (Settings → Core → MCP Server) so external AI agents can inspect agent definitions, build and edit patches, and verify running flows. Enabling it auto-generates a Bearer token; connect from Claude Code with:

```bash
claude mcp add --transport http modular-agent http://127.0.0.1:8765/mcp \
    --header "Authorization: Bearer <token>"
```

The server binds `127.0.0.1` only, and edits made over MCP appear on the open canvas live. See the [core README](../../crates/modular-agent-core/README.md#external-agent-editing-mcp-server) for the tool list and semantics.

## Custom Node UIs

Agent packages can replace the default node rendering with their own Svelte 5 components via [`@modular-agent/widget-kit`](widget-kit/README.md):

- **NodeView** — a custom body for a specific agent type
- **ConfigWidget** — a custom input for a config value type
- **NodeStyle** — presentation overrides for the node frame

UI packages ship as a `ui/` npm package inside the agent repository and are picked up at build time from the ma-config selection — no dynamic loading or registry access.

## Related

- [modular-agent-core](../../crates/modular-agent-core) — orchestration engine and agent runtime
- [tauri-plugin-modular-agent](../../crates/tauri-plugin-modular-agent) — the Tauri bridge this app is built on
- [`ma` CLI](../cli) — run the same patches headless
- [Agent libraries](../../README.md#agent-libraries) — the full package list

## Contributing

- ⭐ **Star to show support** — helps the project reach more people
- 🤝 Pull requests welcome — see [CONTRIBUTING.md](../../CONTRIBUTING.md)

## License

This project is licensed under the [Apache License, Version 2.0](../../LICENSE_APACHE-2.0).
