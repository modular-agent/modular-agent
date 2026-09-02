#![recursion_limit = "256"]

pub mod doc;
pub mod message;
pub mod provider;

// The registry itself is feature-independent, but its pub(crate) helpers
// (resolve_entry, clamp_max_tokens, warm_ollama_context) are only consumed by
// the provider request builders; silence dead-code when none are enabled.
#[cfg_attr(
    not(any(feature = "openai", feature = "claude", feature = "ollama")),
    allow(dead_code)
)]
pub mod capabilities;

// Provider-cross message normalization applied by ChatAgent/ResponsesAgent
// right before provider-specific conversion (P-02).
pub(crate) mod prepare;

pub mod chat;

// Summarization machinery (prompt builder, provider-routed request) behind
// the Messages agents' rolling summary. The module is feature-independent so
// message.rs compiles unconditionally; without any provider feature the
// request path degenerates to an error, parts of it go dead, and the code
// after the always-diverging provider match becomes unreachable.
#[cfg_attr(
    not(any(feature = "openai", feature = "claude", feature = "ollama")),
    allow(dead_code, unreachable_code)
)]
pub(crate) mod summarize;

pub mod completion;
pub mod embeddings;
pub mod usage;

// With no provider features enabled, only `RetryPolicy::from_configs` is
// reachable; the retry machinery itself is dead. Keep it compiled (so all
// feature combinations type-check it) but silence the dead-code lint.
#[cfg_attr(
    not(any(feature = "openai", feature = "claude", feature = "ollama")),
    allow(dead_code)
)]
pub(crate) mod retry;

#[cfg(feature = "openai")]
pub mod responses;

#[cfg(any(feature = "openai", feature = "claude", feature = "ollama"))]
pub(crate) mod http_error;

// MessageContent assembly shared by the provider response converters, plus
// the string-only flattening fallback also used by feature-independent code
// (prepare, summarize), so the module is not gated on provider features.
pub(crate) mod content;

// Only the providers that transport tool arguments as strings need the
// repair parser; Ollama sends them as already-parsed JSON.
#[cfg(any(feature = "openai", feature = "claude"))]
pub(crate) mod json_repair;

#[cfg(feature = "openai")]
pub(crate) mod openai_client;

#[cfg(feature = "claude")]
pub(crate) mod claude_client;

#[cfg(feature = "ollama")]
pub(crate) mod ollama_client;

#[cfg(feature = "ollama")]
pub mod ollama;
