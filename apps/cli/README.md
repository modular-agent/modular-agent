# modular-agent-cli

CLI runner for [Modular Agent](https://github.com/modular-agent) presets. Loads a preset JSON file and provides stdin/stdout communication with the agent network.

## Build

```bash
# Default features (std, llm, web, slack, sqlx)
cargo build

# Specific features
cargo build --no-default-features --features "std,llm"
```

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

## Features

All agent crates are optional, controlled by Cargo features.

| Feature | Crate | Default | Description |
| --- | --- | --- | --- |
| `std` | modular-agent-std | Yes | Array, string, file, time, template agents |
| `llm` | modular-agent-llm | Yes | OpenAI, Ollama chat/completion/embeddings |
| `web` | modular-agent-web | Yes | HTTP fetch, HTML scraper, YouTube transcript |
| `slack` | modular-agent-slack | Yes | Slack messaging |
| `sqlx` | modular-agent-sqlx | Yes | SQLite, MySQL, PostgreSQL |
| `mongodb` | modular-agent-mongodb | No | MongoDB CRUD operations |
| `lifelog` | modular-agent-lifelog | No | Screen capture, window tracking |
| `surrealdb` | modular-agent-surrealdb | No | SurrealDB graph database |
| `lancedb` | modular-agent-lancedb | No | Vector database |
| `all` | All of the above | No | |

## License

Apache-2.0 OR MIT
