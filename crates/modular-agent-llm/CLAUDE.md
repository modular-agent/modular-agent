# CLAUDE.md

See root CLAUDE.md for common agent development patterns.

## Overview

LLM integration library providing completion, chat, and embeddings agents for OpenAI, Ollama, and Claude (Anthropic).

## Modules

| Module | Purpose |
| ------ | ------- |
| doc.rs | Text processing (NFKC normalization, chunking by chars/tokens) |
| message.rs | Message accumulation and formatting |
| capabilities.rs | Model capability registry (context window / max tokens / cost; built-in table + models.json overlay) |
| chat.rs | Chat agents (OpenAI, Ollama, Claude) |
| claude_client.rs | Claude (Anthropic) Messages API client |
| completion.rs | Text completion agents |
| embeddings.rs | Embeddings agents |
| responses.rs | OpenAI Responses API integration |
| provider.rs | Model prefix routing (`openai/`, `ollama/`, `claude/`) |
| openai_client.rs | OpenAI API client wrapper |
| ollama_client.rs | Ollama API client (direct HTTP/NDJSON) |
| ollama.rs | Ollama-specific agents (list models, show info) |

## Features

- `image` (default) - Image support in messages
- `ollama` (default) - Ollama agents
- `openai` (default) - OpenAI agents
- `claude` (default) - Claude (Anthropic) agents

## Agents

| Agent | Category | Purpose |
| ----- | -------- | ------- |
| ChatAgent | LLM | Chat with streaming, tools (OpenAI/Ollama/Claude) |
| ResponsesAgent | LLM | OpenAI Responses API with conversation state |
| CompletionAgent | LLM | Text completion (OpenAI/Ollama) |
| EmbeddingsAgent | LLM | Vector embeddings (OpenAI/Ollama) |
| OllamaListLocalModelsAgent | LLM/Ollama | List available models |
| OllamaShowModelInfoAgent | LLM/Ollama | Show model details |
| NFKCAgent | LLM/Doc | Unicode normalization |
| SplitTextAgent | LLM/Doc | Character-based chunking |
| SplitTextByTokensAgent | LLM/Doc | Token-based chunking |
| UserMessageAgent | LLM/Message | Append user message |
| AssistantMessageAgent | LLM/Message | Append assistant message |
| SystemMessageAgent | LLM/Message | Prepend system message |
| PreambleAgent | LLM/Message | Add preamble once |
| MessagesAgent | LLM/Message | In-memory message history (SessionStore, `session_id`) |
| FileMessagesAgent | LLM/Message | Message history persisted as JSONL session files (`session_dir`/`session_id`) |
| MessagesForPromptAgent | LLM/Message | Filter messages to fit max_size |

## Model Capabilities

`capabilities.rs` resolves per-model metadata (context window, max output
tokens, cost, reasoning) from four layers: `models.json` > built-in static
table > measured Ollama `/api/show` > conservative defaults (8192). The chat /
responses / completion agents use it to pick each model's `max_tokens` default
and to clamp user-configured values to the model limit — this is why Claude's
output is no longer capped at a hardcoded 8192. Loading `models.json` is the
caller's responsibility: CLI and desktop call `load_model_capabilities_json`
once at startup (counterpart of core's `register_tools_from_mcp_json`); without
it only the built-in table and defaults apply.

## Environment Variables

| Variable | Purpose |
| -------- | ------- |
| `OPENAI_API_KEY` | OpenAI API key |
| `OPENAI_API_BASE` | Custom OpenAI endpoint |
| `OLLAMA_API_KEY` | Ollama API key (Bearer token) |
| `OLLAMA_API_BASE_URL` | Ollama server URL |
| `OLLAMA_HOST` | Alternative Ollama host |
| `CLAUDE_API_KEY` | Claude (Anthropic) API key |
| `ANTHROPIC_API_KEY` | Claude API key (fallback) |
| `CLAUDE_API_BASE` | Custom Claude API endpoint |
| `ANTHROPIC_API_BASE` | Claude API endpoint (fallback) |

Default Ollama URL: `http://localhost:11434`
Default Claude API Base: `https://api.anthropic.com`

## Build Commands

```bash
cargo build                        # Default features (openai, ollama, claude, image)
cargo build --all-features         # All features
cargo build --features="ollama"    # Ollama only
cargo build --features="openai"    # OpenAI only
cargo build --features="claude"    # Claude only
cargo test --all-features          # Run tests
```

## Message Type Structure

```rust
Message {
    id: Option<String>,
    role: String,           // "user", "assistant", "system", "tool"
    content: String,
    thinking: Option<String>,
    tool_calls: Option<Vector<ToolCall>>,
    image: Option<PhotonImage>,  // feature-gated
}
```

## ResponsesAgent (Responses API)

OpenAI Responses API を使用するAgent。Chat Completions APIとの違い:
- サーバーサイド会話状態管理 (`previous_response_id`)
- セマンティックなストリーミングイベント
- 推論モデル(GPT-5等)でのパフォーマンス向上

### Ports

| Port | Direction | Purpose |
| ---- | --------- | ------- |
| message | input | Message or array of messages |
| reset | input | Reset conversation state (any value) |
| message | output | Assistant's response message |
| response | output | Raw API response |

### Configuration

| Config | Type | Default | Description |
| ------ | ---- | ------- | ----------- |
| model | string | openai/gpt-5-mini | Model name |
| stream | boolean | false | Enable streaming |
| use_conversation_state | boolean | true | Use server-side conversation state |
| tools | text | - | Tool patterns (regex, newline-separated) |
| options | object | - | Additional request options (JSON) |
| max_tokens | integer | 0 | Max output tokens (0: API default). Maps to `max_output_tokens` |
| temperature | number | -1 | Sampling temperature (-1: API default, 0.0-2.0) |
| top_p | number | -1 | Nucleus sampling (-1: API default, 0.0-1.0) |

### Future: Built-in Tools

将来的にビルトインツールをサポート予定 (web_search, file_search, code_interpreter)。
現在は `options` configでJSON形式で指定可能:
```json
{
  "tools": [
    { "type": "web_search" },
    { "type": "code_interpreter" }
  ]
}
```

## Model Prefix Format

All LLM agents use a `provider/model-name` prefix to route to the correct provider:

| Prefix | Provider | Example |
| ------ | -------- | ------- |
| `openai/` | OpenAI | `openai/gpt-5-mini` |
| `ollama/` | Ollama | `ollama/llama3.2:1b` |
| `claude/` | Claude (Anthropic) | `claude/claude-sonnet-4-5-20250514` |

Provider prefix is mandatory. Model names without a prefix will produce an error.

## Common LLM Configs (detail)

ChatAgent, CompletionAgent, ResponsesAgent share these configs (`detail = true`, sidebar only):

| Config | Type | Default | Description |
| ------ | ---- | ------- | ----------- |
| max_tokens | integer | 0 | Max output tokens. 0 = use API default. Provider mapping: `max_tokens` (OpenAI Chat, Claude), `max_output_tokens` (OpenAI Responses), `num_predict` (Ollama) |
| temperature | number | -1 | Sampling temperature. -1 = use API default. Valid range: 0.0-2.0 |
| top_p | number | -1 | Nucleus sampling. -1 = use API default. Valid range: 0.0-1.0 |

Priority: These configs override values set in `options` JSON. Sentinel values (-1, 0) mean "not set".

## Claude Integration

ChatAgent supports Claude (Anthropic) Messages API via the `claude` feature flag.

- Uses `reqwest` + `eventsource-stream` for direct HTTP/SSE communication
- Supports streaming and non-streaming modes
- Supports tool use (function calling)
- Supports extended thinking via options: `{"thinking": {"type": "enabled", "budget_tokens": 10000}}`
- Default `max_tokens`: registry-resolved model cap for streaming requests; 8192 for non-streaming requests and registry-unknown models (overridable via `max_tokens` config — clamped to the known model limit — or options)
- CompletionAgent and EmbeddingsAgent return errors for Claude (unsupported by Anthropic API)

### Global Configs (Claude)

| Config | Purpose |
| ------ | ------- |
| `claude_api_key` | Claude API key (overrides env var) |
| `claude_api_base` | Custom API base URL |

## Ollama Integration

All three providers (OpenAI, Ollama, Claude) now use direct `reqwest` HTTP calls — no provider-specific SDK crates.

### Global Configs (Ollama)

| Config | Purpose |
| ------ | ------- |
| `ollama_api_key` | Ollama API key / Bearer token (overrides env var) |
| `ollama_url` | Ollama server URL (overrides env var) |

- Ollama uses NDJSON streaming (not SSE); `ollama_client::post_ndjson_stream` handles line-buffered parsing via `futures::stream::unfold`
- Ollama `options` (temperature, top_p, etc.) must be nested under the `"options"` key in requests, unlike OpenAI's flat merge — use `ollama_client::merge_options`
- `GenerationContext` is `Vec<i64>` (not `i32`) to support large-vocabulary models

## Key Dependencies

- `reqwest` (0.12) - HTTP client for all three providers (OpenAI, Ollama, Claude)
- `eventsource-stream` (0.2) - SSE streaming (openai, claude features)
- `futures` - Stream combinators, NDJSON streaming (ollama, openai, claude features)
- `tokenizers` (0.22.2) - Hugging Face tokenizers
- `text-splitter` (0.29.3) - Text chunking
