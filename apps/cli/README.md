# modular-agent-cli

CLI runner for [Modular Agent](https://github.com/modular-agent) presets. Loads a preset JSON file and provides stdin/stdout communication with the agent network.

## Build

```bash
cargo build
```

### Custom Build with ma-build

`ma-build` is a TUI wizard that lets you select which agent crates to include, configure their sources (local path or Git repository), and build a custom `ma` binary.

```bash
# Run the wizard
cd tools/ma-build && cargo run

# Use a specific config file
cargo run -- server.toml

# Build in release mode (skip build-mode prompt)
cargo run -- --release
```

The wizard generates `Cargo.toml` dependencies and `src/agents.rs` based on your selections. Configuration is saved to `ma-build.toml` for subsequent rebuilds.

## Usage

```
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
| [modular-agent-audio](https://github.com/modular-agent/modular-agent-audio) | Audio capture/transcription | |
| [modular-agent-cozodb](https://github.com/modular-agent/modular-agent-cozodb) | CozoDB logic database | |
| [modular-agent-duckdb](https://github.com/modular-agent/modular-agent-duckdb) | DuckDB analytics | |
| [modular-agent-lancedb](https://github.com/modular-agent/modular-agent-lancedb) | LanceDB vector database | |
| [modular-agent-lifelog](https://github.com/modular-agent/modular-agent-lifelog) | Screen capture, window tracking | |
| [modular-agent-llm](https://github.com/modular-agent/modular-agent-llm) | LLM (OpenAI, Anthropic, Google, etc.) | Yes |
| [modular-agent-mongodb](https://github.com/modular-agent/modular-agent-mongodb) | MongoDB CRUD | |
| [modular-agent-monty](https://github.com/modular-agent/modular-agent-monty) | Monty | |
| [modular-agent-slack](https://github.com/modular-agent/modular-agent-slack) | Slack messaging | Yes |
| [modular-agent-sqlx](https://github.com/modular-agent/modular-agent-sqlx) | SQL database (PostgreSQL, MySQL, SQLite) | Yes |
| [modular-agent-std](https://github.com/modular-agent/modular-agent-std) | Standard (timer, template, file, etc.) | Yes |
| [modular-agent-surrealdb](https://github.com/modular-agent/modular-agent-surrealdb) | SurrealDB graph database | |
| [modular-agent-voicevox](https://github.com/modular-agent/modular-agent-voicevox) | VOICEVOX text-to-speech | |
| [modular-agent-web](https://github.com/modular-agent/modular-agent-web) | Web/HTTP, scraping, YouTube | Yes |

Agent selection and features are managed by the `ma-build` wizard.

## Development

### Adding Custom Agents

To add a custom agent package, edit `tools/ma-build/registry.yaml`:

```yaml
  - name: my-custom
    description: My custom agents
    git_url: https://github.com/your-repo/my-custom.git
```

Then re-run `ma-build` to include it in the build.

## License

Apache-2.0 OR MIT
