# CLAUDE.md

See root CLAUDE.md for common module development patterns.

## Overview

LLM integration library providing completion, chat, and embeddings modules for OpenAI, Ollama, and Claude (Anthropic).

## Modules

| Module | Purpose |
| ------ | ------- |
| doc.rs | Text processing (NFKC normalization, chunking by chars/tokens) |
| message.rs | Message accumulation and formatting |
| capabilities.rs | Model capability registry (context window / max tokens / thinking levels / cost; models.json > built-in table > Ollama probe > defaults) |
| models_config.rs | `models.json` (Pi format): providers, endpoints, API keys, per-model entries; two files, read once per process |
| chat.rs | Chat modules (OpenAI, Ollama, Claude) |
| claude_client.rs | Claude (Anthropic) Messages API client |
| completion.rs | Text completion modules |
| embeddings.rs | Embeddings modules |
| responses.rs | OpenAI Responses API integration |
| provider.rs | Model prefix routing (`openai/`, `ollama/`, `claude/`, custom providers), per-module client cache |
| openai_client.rs | OpenAI API client wrapper |
| ollama_client.rs | Ollama API client (direct HTTP/NDJSON) |
| ollama.rs | Ollama-specific modules (list models, show info) |

## Features

- `image` (default) - Image support in messages
- `ollama` (default) - Ollama modules
- `openai` (default) - OpenAI modules
- `claude` (default) - Claude (Anthropic) modules

## Modules

| Module | Category | Purpose |
| ----- | -------- | ------- |
| ChatModule | LLM | Chat with streaming, tools (OpenAI/Ollama/Claude) |
| ResponsesModule | LLM | OpenAI Responses API with conversation state |
| CompletionModule | LLM | Text completion (OpenAI/Ollama) |
| EmbeddingsModule | LLM | Vector embeddings (OpenAI/Ollama) |
| OllamaListLocalModelsModule | LLM/Ollama | List available models |
| OllamaShowModelInfoModule | LLM/Ollama | Show model details |
| NFKCModule | LLM/Doc | Unicode normalization |
| SplitTextModule | LLM/Doc | Character-based chunking |
| SplitTextByTokensModule | LLM/Doc | Token-based chunking |
| UserMessageModule | LLM/Message | Append user message |
| AssistantMessageModule | LLM/Message | Append assistant message |
| SystemMessageModule | LLM/Message | Prepend system message |
| PreambleModule | LLM/Message | Add preamble once |
| MessagesModule | LLM/Message | In-memory message history; emits a prompt-ready window on user/tool arrivals, bounded by `max_context_tokens`/`max_messages`/`max_message_tokens`; `summarize_model` folds evicted history into a rolling LLM summary instead of deleting it |
| FileMessagesModule | LLM/Message | Same, persisted as JSONL session files (`session_dir`); `prune_file` controls whether trimmed history is also deleted from the file |
| MessagesForPromptModule | LLM/Message | Trim a session-less message stream to a max_tokens/max_size budget |

## models.json (connection settings)

All provider connection settings live in `models.json` (user-facing format in
README.md). There are no global configs and no implicit environment
variables: the environment is read only where `apiKey` says `${VAR}`
(expanded by core's `expand_vars`, the same syntax as workspace settings).

- **Loading** (`models_config.rs`): `~/.modular_agent/models.json`, then
  `models.json` in the working directory (hosts make the workspace their cwd;
  llm is ma-config-selectable, so app code cannot call into it). Provider-level
  replacement, local wins. JSONC: `strip_jsonc` blanks comments and trailing
  commas in place (byte positions kept, so serde_json errors point into the
  file); `models.json.sample` is the commented template, and a test loads it
  both as shipped and uncommented. Read once on first use (`LazyLock`) and kept for
  the process; edits need a restart. Any invalid file makes `current()` an error, and
  `ModelIdentifier::parse` fails for *every* provider — never fall back to the
  default endpoints, a broken edit would silently reroute requests.
- **Routing**: `ModelIdentifier { provider: ProviderKind, model_name, config:
  Arc<Provider> }`. `ProviderKind` is the API spoken; custom providers pick it
  with `api` (`openai-completions` / `anthropic-messages` / `ollama`).
- **Clients**: managers take `&Provider` and cache one client per provider ID
  in `ClientCache` (a module's `model` can move between providers). An absent
  key sends no auth header. ResponsesModule drops `previous_response_id` when
  the provider changes, since response IDs are server-local.
- **Tests** never read the user's files: under `cfg(test)` `current()` returns
  the built-ins plus `test_support::register_test_provider` entries (unique ID
  per test; the registry is process-global). Capability tests build
  `ModelIdentifier` directly from `test_support::config_from_json` (bypassing
  `parse`'s feature check, so they run in every feature combination); routing
  tests use `ModelIdentifier::parse_with`.

## Model Capabilities

`capabilities.rs` resolves per-model metadata from four layers: the model's
`models.json` entry (exact ID, within its provider) > built-in static table
(prefix match; built-in providers only) > measured Ollama `/api/show` (keyed by
`provider/model`) > conservative defaults (8192). The chat / responses /
completion modules use it to pick each model's `max_tokens` default and to
clamp user-configured values to the model limit.

Thinking levels follow Pi's vocabulary minus "off" (`minimal` … `max`).
`reasoning: true` adds the API's defaults (minimal–high; OpenAI with the level
name as `reasoning_effort`, Claude/Ollama with no parameter), then
`thinkingLevelMap` sets (string) or removes (`null`) individual levels.

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

## ResponsesModule (Responses API)

OpenAI Responses API を使用するModule。Chat Completions APIとの違い:
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

All LLM modules use a `provider/model-name` prefix to route to the correct provider:

| Prefix | Provider | Example |
| ------ | -------- | ------- |
| `openai/` | OpenAI | `openai/gpt-5-mini` |
| `ollama/` | Ollama | `ollama/llama3.2:1b` |
| `claude/` | Claude (Anthropic) | `claude/claude-sonnet-4-5-20250514` |
| `<id>/` | Custom provider from `models.json` | `lmstudio/qwen3-30b-a3b` |

Provider prefix is mandatory. Model names without a prefix will produce an error.

## Common LLM Configs (detail)

ChatModule, CompletionModule, ResponsesModule share these configs (`detail = true`, sidebar only):

| Config | Type | Default | Description |
| ------ | ---- | ------- | ----------- |
| max_tokens | integer | 0 | Max output tokens. 0 = use API default. Provider mapping: `max_tokens` (OpenAI Chat, Claude), `max_output_tokens` (OpenAI Responses), `num_predict` (Ollama) |
| temperature | number | -1 | Sampling temperature. -1 = use API default. Valid range: 0.0-2.0 |
| top_p | number | -1 | Nucleus sampling. -1 = use API default. Valid range: 0.0-1.0 |

Priority: These configs override values set in `options` JSON. Sentinel values (-1, 0) mean "not set".

## Claude Integration

ChatModule supports Claude (Anthropic) Messages API via the `claude` feature flag.

- Uses `reqwest` + `eventsource-stream` for direct HTTP/SSE communication
- Supports streaming and non-streaming modes
- Supports tool use (function calling)
- Supports extended thinking via options: `{"thinking": {"type": "enabled", "budget_tokens": 10000}}`
- Default `max_tokens`: registry-resolved model cap for streaming requests; 8192 for non-streaming requests and registry-unknown models (overridable via `max_tokens` config — clamped to the known model limit — or options)
- CompletionModule and EmbeddingsModule return errors for Claude (unsupported by Anthropic API)

## Ollama Integration

All three providers (OpenAI, Ollama, Claude) now use direct `reqwest` HTTP calls — no provider-specific SDK crates.

- Ollama uses NDJSON streaming (not SSE); `ollama_client::post_ndjson_stream` handles line-buffered parsing via `futures::stream::unfold`
- Ollama `options` (temperature, top_p, etc.) must be nested under the `"options"` key in requests, unlike OpenAI's flat merge — use `ollama_client::merge_options`
- `GenerationContext` is `Vec<i64>` (not `i32`) to support large-vocabulary models

## Key Dependencies

- `reqwest` (0.13) - HTTP client for all three providers (OpenAI, Ollama, Claude)
- `eventsource-stream` (0.2) - SSE streaming (openai, claude features)
- `futures` - Stream combinators, NDJSON streaming (ollama, openai, claude features)
- `tokenizers` (0.23) - Hugging Face tokenizers
- `text-splitter` (0.33) - Text chunking
