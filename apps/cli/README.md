# modular-agent-cli

CLI runner for [Modular Agent](https://github.com/modular-agent) presets. Loads a preset JSON file and provides stdin/stdout communication with the agent network.

## Build

From a checkout of the monorepo:

```bash
cargo build -p modular-agent-cli
```

The `ma` binary lands in the workspace-level `target/` directory at the repository root.

### Custom Configuration with ma-config

`ma-config` is a TUI wizard that lets you select which agent crates to include and which Cargo features they build with. Agent crates from outside this repository are linked from `custom_agents/<name>` at the repository root, and only clones that are already there are offered — see `custom_agents/README.md` for the list of agent repositories and the `git clone` commands.

```bash
cargo run --manifest-path tools/ma-config/Cargo.toml -- cli
```

The wizard generates `apps/cli/Cargo.toml` dependencies and `apps/cli/src/agents.rs` based on your selections. Configuration is saved to `apps/cli/ma-config.toml` for subsequent runs.

## Usage

```bash
ma <preset> [-i <input>] [-o <output>] [-v]
```

| Argument | Default | Description |
| --- | --- | --- |
| `preset` | (required) | Path to preset JSON file |
| `-i, --input` | `input` | External input channel name |
| `-o, --output` | `output` | External output channel name |
| `-v, --verbose` | off | Enable logging |

## Examples

```bash
# Interactive mode
ma ./preset.json

# Single input via pipe
echo "Hello" | ma ./preset.json

# Input from file
ma ./preset.json < input.txt

# Output to file
echo "Hello" | ma ./preset.json > output.txt

# Custom input/output channels
echo "Hello" | ma ./preset.json -i "query" -o "result"

# Chain with other tools
cat data.txt | ma ./preset.json | jq '.result'
```

Input is read line-by-line from stdin. String output is printed as-is; other types are printed as JSON.

## Official Agents

| Package | Description | Default |
| --- | --- | --- |
| [modular-agent-cozodb](https://github.com/modular-agent/modular-agent-cozodb) | CozoDB logic database | |
| [modular-agent-duckdb](https://github.com/modular-agent/modular-agent-duckdb) | DuckDB analytics | |
| [modular-agent-lancedb](https://github.com/modular-agent/modular-agent-lancedb) | LanceDB vector database | |
| [modular-agent-lifelog](https://github.com/modular-agent/modular-agent-lifelog) | Screen capture, window tracking | |
| [modular-agent-llm](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-llm) | LLM (OpenAI, Anthropic, Google, etc.) | Yes |
| [modular-agent-mongodb](https://github.com/modular-agent/modular-agent-mongodb) | MongoDB CRUD | |
| [modular-agent-monty](https://github.com/modular-agent/modular-agent-monty) | Monty | |
| [modular-agent-slack](https://github.com/modular-agent/modular-agent-slack) | Slack messaging | Yes |
| [modular-agent-sqlx](https://github.com/modular-agent/modular-agent-sqlx) | SQL database (PostgreSQL, MySQL, SQLite) | Yes |
| [modular-agent-std](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-std) | Standard (timer, template, file, etc.) | Yes |
| [modular-agent-surrealdb](https://github.com/modular-agent/modular-agent-surrealdb) | SurrealDB graph database | |
| [modular-agent-web](https://github.com/modular-agent/modular-agent-web) | Web/HTTP, scraping, search, YouTube | Yes |

Agent selection and features are managed by the `ma-config` wizard.

## Development

### Adding Custom Agents

A custom agent package describes itself with an `ma-registry.yaml` at the root of its own repository:

```yaml
name: my-custom
description: My custom agents
```

Clone the package into `custom_agents/my-custom` — the directory name has to match `name` — and re-run `ma-config`, which scans `custom_agents/` and offers every clone it finds. `available_features`, `default_features`, `default_for` and `conflicts` are optional; see `custom_agents/README.md` for the full schema and for the agent repositories published under `modular-agent`.

## License

Apache-2.0
