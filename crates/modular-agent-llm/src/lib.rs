#![recursion_limit = "256"]

pub mod doc;
pub mod message;
pub mod provider;

pub mod chat;
pub mod completion;
pub mod embeddings;

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
