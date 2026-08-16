# modular-agent-cli

CLI runner for [Modular Agent](https://github.com/modular-agent) patches. Loads a patch JSON file and provides stdin/stdout communication with the agent network.

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
ma <patch> [-i <input>] [-o <output>] [-v]
```

| Argument | Default | Description |
| --- | --- | --- |
| `patch` | (required) | Path to patch JSON file |
| `-i, --input` | `input` | External input channel name |
| `-o, --output` | `output` | External output channel name |
| `-v, --verbose` | off | Enable logging |

## Examples

```bash
# Interactive mode
ma ./patch.json

# Single input via pipe
echo "Hello" | ma ./patch.json

# Input from file
ma ./patch.json < input.txt

# Output to file
echo "Hello" | ma ./patch.json > output.txt

# Custom input/output channels
echo "Hello" | ma ./patch.json -i "query" -o "result"

# Chain with other tools
cat data.txt | ma ./patch.json | jq '.result'
```

Input is read line-by-line from stdin. String output is printed as-is; other types are printed as JSON.

## Agent Plugins

The default build includes the agent crates that live in this repository:

| Crate | Description |
| --- | --- |
| [modular-agent-std](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-std) | Standard (timer, template, file, etc.) |
| [modular-agent-llm](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-llm) | LLM (OpenAI, Anthropic, Google, etc.) |

More agent packages — web scraping, Slack and Mattermost messaging, SQL databases, screen capture, script agents, and others — live in their own repositories. Clone the ones you want into `custom_agents/` at the repository root and select them with the `ma-config` wizard (see above); [custom_agents/README.md](../../custom_agents/README.md) lists the repositories.

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
