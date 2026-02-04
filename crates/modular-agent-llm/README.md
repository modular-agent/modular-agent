# LLM Agents for Modular Agent

LLM integration library providing chat, completion, embeddings, and responses agents for OpenAI and Ollama.

## Features

- **ChatAgent** - Multi-provider chat with streaming and tool support (OpenAI/Ollama)
- **ResponsesAgent** - OpenAI Responses API with server-side conversation state
- **CompletionAgent** - Text completion (OpenAI/Ollama)
- **EmbeddingsAgent** - Vector embeddings (OpenAI/Ollama)
- **Message Agents** - Message accumulation and formatting utilities
- **Doc Agents** - Text processing (NFKC normalization, chunking)

## Installation

```toml
[dependencies]
modular-agent-llm = "0.11.0"
```

## Feature Flags

- `openai` (default) - OpenAI API support
- `ollama` (default) - Ollama local LLM support
- `image` (default) - Image support in messages

## ResponsesAgent (Responses API)

The ResponsesAgent uses OpenAI's new Responses API, which provides:

- Server-side conversation state via `previous_response_id`
- Semantic streaming events
- Better performance with reasoning models (GPT-5, etc.)

### Configuration

| Config | Default | Description |
| ------ | ------- | ----------- |
| model | gpt-5-mini | Model name |
| stream | false | Enable streaming |
| use_conversation_state | true | Use server-side conversation state |
| tools | - | Tool patterns (regex, newline-separated) |
| options | - | Additional request options (JSON) |

### Ports

- **Input**: `message` (Message/array), `reset` (reset conversation state)
- **Output**: `message` (assistant response), `response` (raw API response)

## Environment Variables

| Variable | Purpose |
| -------- | ------- |
| `OPENAI_API_KEY` | OpenAI API key |
| `OPENAI_API_BASE` | Custom OpenAI endpoint |
| `OLLAMA_API_BASE_URL` | Ollama server URL |

## License

Apache-2.0 OR MIT
