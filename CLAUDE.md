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

## Agent Development

The `#[modular_agent]` macro pattern, doc-comment rules for agent descriptions, UI
hints, config types, lifecycle methods, error handling, and the DB connection caching
pattern are documented in the `agent-development` skill
(`.claude/skills/agent-development/SKILL.md`). Load it before writing or reviewing
agents.

## Dependencies

Shared external dependencies live in `[workspace.dependencies]`; crates whose feature
sets diverge (tokio, reqwest) keep their own entries.

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
`core-v0.27.0`, `std-v0.17.0`, `llm-v0.15.0`, `web-v0.10.0`, `plugin-v0.18.0`,
`desktop-v0.19.0`, `cli-v0.4.1`. Tags from before the merge were rewritten with the same
prefixes.

## See Also

- `.claude/skills/agent-development/SKILL.md` - Agent development patterns
- `crates/modular-agent-core/CLAUDE.md` - Core engine details, AgentValue types
- `crates/modular-agent-std/CLAUDE.md` - Standard utility agents
- `crates/modular-agent-llm/CLAUDE.md` - LLM integration agents
- `apps/desktop/CLAUDE.md` - Desktop app architecture, ma-config
- `apps/cli/CLAUDE.md` - CLI runner
- `../modular-agent-com/design-brief.md` - Homepage design brief (Japanese)
