# modular-agent-cli

CLI runner for [Modular Agent](https://github.com/modular-agent) patches. `ma serve` keeps a workspace's patches running as a server process (logs, health check, signals, MCP); `ma run` wires one patch to stdin/stdout.

## Build

From a checkout of the monorepo:

```bash
cargo build -p modular-agent-cli
```

The `ma` binary lands in the workspace-level `target/` directory at the repository root.

### Custom Configuration with ma-config

`ma-config` is a TUI wizard that lets you select which module crates to include and which Cargo features they build with. Module crates from outside this repository are linked from `custom_modules/<name>` at the repository root, and only clones that are already there are offered — see `custom_modules/README.md` for the list of module repositories and the `git clone` commands.

```bash
cargo run --manifest-path tools/ma-config/Cargo.toml -- cli
```

The wizard generates `apps/cli/Cargo.toml` dependencies and `apps/cli/src/modules.rs` based on your selections. Configuration is saved to `apps/cli/ma-config.toml` for subsequent runs.

`--config <path>` applies a different selection file instead, such as the committed profile for the container image:

```bash
cargo run --manifest-path tools/ma-config/Cargo.toml -- cli --apply --config apps/cli/ma-config.cloud.toml
```

`Cargo.lock` is left as it is apart from what the selection adds on the next build; pass `--update` to run `cargo update` afterwards.

## Workspaces

A workspace is a directory that `ma serve` runs from and the desktop app edits (`~/.modular_agent` there):

```text
settings.json   # global configs and the patches to start automatically
patches/        # patch JSON files; a "/" in a patch name is a subfolder
data/           # conventional place for files patches write
```

```json
{
  "modules": { "modular_agent_llm::chat::ChatModule": { "openai_api_key": "${OPENAI_API_KEY}" } },
  "auto_start_patches": ["bugs/triage", "digest"]
}
```

- `${VAR}` in a global config value is replaced from the environment, so secrets stay out of the file. An unset variable or an unknown key stops `ma` from starting. Settings for a module this build does not include (a desktop-only one, say) are skipped with a warning, since builds select different modules.
- Relative paths in patches (for example `data/journal.jsonl`) resolve against the workspace.

[examples/workspace](../../examples/workspace) is a small workspace to try both commands with.

## Usage

```bash
ma [--log-format text|json] serve --workspace <dir> [server options]
ma [--log-format text|json] run <patch> [--workspace <dir>] [-i <input>] [-o <output>] [--json] [--drain <duration>] [server options]
```

### ma serve

Applies the workspace's global configs, starts its `auto_start_patches`, and runs until SIGTERM or SIGINT, then stops the patches gracefully. It exits with an error if a setting cannot be applied or a patch fails to start, leaving restarts to the orchestrator.

### ma run

Runs one patch with stdin/stdout wired to its external channels, until stdin ends.

| Argument | Default | Description |
| --- | --- | --- |
| `patch` | (required) | Patch name in the workspace with `--workspace`, otherwise a patch file path |
| `--workspace` | none | Apply this workspace's global configs and resolve relative paths against it (its auto-start list is not used) |
| `-i, --input` | `input` | External input channel fed one value per stdin line |
| `-o, --output` | `output` | External output channel written to stdout |
| `--json` | off | Read and write one JSON value per line, so values keep their types; otherwise lines are strings and non-string outputs print as JSON |
| `--drain` | none | Keep writing outputs this long after stdin ends (e.g. `2s`), for patches that answer asynchronously |

```bash
echo "Hello" | ma run ./patch.json
cat data.txt | ma run ./patch.json -i query -o result --drain 2s > out.txt
echo '"{\"a\": 1}"' | ma run --workspace examples/workspace --json --drain 1s -i parse -o parsed parse | jq .a
```

### Server options

| Option | Default | Description |
| --- | --- | --- |
| `--listen` | off | Serve `/healthz` and `/mcp` on this address (e.g. `127.0.0.1:8080`) |
| `--allowed-host` | none | Extra `Host` header accepted by `/mcp` when clients reach it by a name other than loopback (repeatable) |
| `--shutdown-timeout` | `5s` | Time allowed for patches to stop on shutdown |

- `/healthz` answers 200 once the patches are running and 503 before that.
- `/mcp` is the built-in MCP server, for agents such as Claude Code. Listening beyond loopback requires the `MA_TOKEN` environment variable, and `/mcp` then requires `Authorization: Bearer <MA_TOKEN>`. The token is read from the environment so it never appears in a process listing.

### Logs

Logs go to stderr; stdout carries only `ma run`'s output. `--log-format json` writes one object per line in a shape cloud log pipelines read without configuration:

```json
{"timestamp":"2026-10-04T03:52:57.365Z","severity":"INFO","message":"heartbeat 1","target":"modular_agent::patch","patch":"heartbeat","module":"3"}
```

`RUST_LOG` sets the filter; the default is `warn,modular_agent::=info`. Patch and module lifecycle, errors and outputs are logged under `modular_agent::events` (outputs at debug level), and the records of Log nodes placed in a patch under `modular_agent::patch`.

## Container

[Dockerfile](Dockerfile) builds an image running `ma serve` on `/workspace` (a volume) and `0.0.0.0:8080`, with JSON logs. Build it from the repository root; it clones the out-of-tree module crates selected by `ma-config.cloud.toml`:

```bash
docker build -f apps/cli/Dockerfile -t modular-agent .
docker run -e MA_TOKEN=... -e OPENAI_API_KEY=... -p 8080:8080 -v ./my-workspace:/workspace modular-agent
```

`docker stop` sends SIGTERM, which stops the patches gracefully.

## Module Plugins

The default build includes the module crates that live in this repository:

| Crate | Description |
| --- | --- |
| [modular-agent-std](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-std) | Standard (timer, template, file, etc.) |
| [modular-agent-llm](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-llm) | LLM (OpenAI, Claude, Ollama) |

More module packages — web scraping, Slack and Mattermost messaging, SQL databases, screen capture, script modules, and others — live in their own repositories. Clone the ones you want into `custom_modules/` at the repository root and select them with the `ma-config` wizard (see above); [custom_modules/README.md](../../custom_modules/README.md) lists the repositories.

## Development

### Adding Custom Modules

A custom module package describes itself with an `ma-registry.yaml` at the root of its own repository:

```yaml
name: my-custom
description: My custom modules
```

Clone the package into `custom_modules/my-custom` — the directory name has to match `name` — and re-run `ma-config`, which scans `custom_modules/` and offers every clone it finds. `available_features`, `default_features`, `default_for` and `conflicts` are optional; see `custom_modules/README.md` for the full schema and for the module repositories published under `modular-agent`.

## License

Apache-2.0
