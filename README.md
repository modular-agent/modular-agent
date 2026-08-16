<div align="center">

<img alt="Modular Agent" width="150" height="150" src="https://raw.githubusercontent.com/modular-agent/modular-agent/main/crates/modular-agent-core/doc/images/Square150x150Logo.png">
<br/>
<br/>

<img alt="Modular Agent" width="343" height="60" src="https://raw.githubusercontent.com/modular-agent/modular-agent/main/apps/desktop/doc/images/modular_agent_title.svg">
<br>
<br>

</div>

Build AI workflows like a modular synth — patch extensible agents together visually into real-time pipelines. LLMs, databases, web scrapers, messaging, and more. Privacy-first, no cloud required.

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

Agent libraries outside this repository (databases, Slack, lifelog,
and others) live in their own repositories under
[github.com/modular-agent](https://github.com/modular-agent) and are cloned into
`custom_agents/` to build as part of this workspace.
