# LLM Modules for Modular Agent

LLM integration library providing chat, completion, embeddings, and responses modules for OpenAI, Ollama, and Claude (Anthropic).

## Features

- **ChatModule** - Multi-provider chat with streaming and tool support (OpenAI/Ollama/Claude)
- **ResponsesModule** - OpenAI Responses API with server-side conversation state
- **CompletionModule** - Text completion (OpenAI/Ollama)
- **EmbeddingsModule** - Vector embeddings (OpenAI/Ollama)
- **Message Modules** - Message accumulation and formatting utilities
- **Doc Modules** - Text processing (NFKC normalization, chunking)

## Installation

```toml
[dependencies]
modular-agent-llm = "0.33"
```

## Model Prefix

Use `provider/model-name` format to select the provider:

- `openai/gpt-5-mini` - OpenAI
- `ollama/llama3.2:1b` - Ollama
- `claude/claude-sonnet-4-5-20250514` - Claude (Anthropic)
- `lmstudio/qwen3-30b-a3b` - a provider defined in `models.json`

A provider prefix is required. Only the first `/` separates it, so
`openai/qwen/qwen3-vl-8b` sends the model name `qwen/qwen3-vl-8b`.

## models.json

Endpoints, API keys and per-model limits are configured in `models.json`, in
the same shape as Pi's `models.json`. Two files are read:

1. `~/.modular_agent/models.json`
2. `models.json` in the working directory — the workspace for the desktop app
   and `ma --workspace` (the same file as 1 for the desktop app)

A provider defined in the second file replaces the one with the same ID in
the first. The files are read once, when a model is first used, so restart
the app (or `ma`) after editing them. If either file is invalid, every model
fails with the file's error; nothing falls back to the default endpoints.

Comments (`//`, `/* */`) and trailing commas are allowed.
[models.json.sample](models.json.sample) is a template with OpenAI, Claude,
Ollama and LM Studio commented out: copy it, uncomment what you use and fill
in the values.

```json
{
  "providers": {
    "openai": { "apiKey": "${OPENAI_API_KEY}" },
    "claude": {
      "apiKey": "${ANTHROPIC_API_KEY}",
      "models": [{ "id": "claude-sonnet-4-6", "cost": { "input": 2.5, "output": 12.5 } }]
    },
    "lmstudio": {
      "baseUrl": "http://localhost:1234/v1",
      "api": "openai-completions",
      "models": [
        {
          "id": "qwen3-30b-a3b",
          "reasoning": true,
          "input": ["text", "image"]
        }
      ]
    }
  }
}
```

### Providers

`openai`, `claude` and `ollama` are built in and work without an entry. Any
other ID defines a custom provider, used as `<id>/<model>`.

| Field | Description |
| ----- | ----------- |
| `baseUrl` | Endpoint. Required for custom providers; built-ins default to `https://api.openai.com/v1`, `https://api.anthropic.com` and `http://localhost:11434`. OpenAI-style URLs include `/v1`, Anthropic and Ollama ones do not |
| `api` | Custom providers only (required): `openai-completions`, `anthropic-messages`, or `ollama` (Ollama's native API) |
| `apiKey` | A literal key or `${VAR}`. Omitted: requests are sent without authentication |
| `name` | Display name (informational) |
| `models` | Model entries, below |

Environment variables are read only through `${VAR}`: `OPENAI_API_KEY`,
`ANTHROPIC_API_KEY` and the like are not picked up on their own.

### Models

An entry applies to the model whose name equals `id` under that provider. For
a model the built-in table knows (current Claude and OpenAI models on the
built-in providers), only the fields you set are overridden.

| Field | Description |
| ----- | ----------- |
| `id` | Model name sent to the API (required) |
| `name` | Display name (informational) |
| `contextWindow` | Context window in tokens. Metadata only: it is not sent to the server and does not change the context length a local server (LM Studio, Ollama) loads the model with |
| `maxTokens` | Output token limit; the `max_tokens` config is clamped to it, and Claude uses it as the default. Not a server setting either |
| `input` | `["text"]` or `["text", "image"]`; images are dropped for text-only models |
| `reasoning` | `true` enables the `thinking_level` config with the API's default levels (minimal–high); `false` disables it |
| `thinkingLevelMap` | Per level (`minimal`, `low`, `medium`, `high`, `xhigh`, `max`): a string is the value sent to the API (OpenAI `reasoning_effort`, Claude `effort`), `null` removes the level. `xhigh` and `max` are available only when mapped |
| `cost` | USD per million tokens: `input`, `output` (required), `cacheRead`, `cacheWrite` |

Unknown fields are errors, so a typo does not go unnoticed. Unlike Pi,
`headers`, `compat`, `modelOverrides` and `!command` keys are not supported,
and an entry for a built-in model overrides it field by field.

## Feature Flags

- `openai` (default) - OpenAI API support
- `ollama` (default) - Ollama local LLM support
- `claude` (default) - Claude (Anthropic) API support
- `image` (default) - Image support in messages

## ResponsesModule (Responses API)

The ResponsesModule uses OpenAI's new Responses API, which provides:

- Server-side conversation state via `previous_response_id`
- Semantic streaming events
- Better performance with reasoning models (GPT-5, etc.)

### Configuration

| Config | Default | Description |
| ------ | ------- | ----------- |
| model | openai/gpt-5-mini | Model name |
| stream | false | Enable streaming |
| use_conversation_state | true | Use server-side conversation state |
| tools | - | Tool patterns (regex, newline-separated) |
| options | - | Additional request options (JSON) |

### Ports

- **Input**: `message` (Message/array), `reset` (reset conversation state)
- **Output**: `message` (assistant response), `response` (raw API response)

## License

Apache-2.0
