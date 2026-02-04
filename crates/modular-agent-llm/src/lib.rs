#![recursion_limit = "256"]

pub mod doc;
pub mod message;
pub mod provider;

// Unified agents
pub mod chat;
pub mod completion;
pub mod embeddings;

// Provider-specific client modules (internal)
#[cfg(feature = "openai")]
pub(crate) mod openai_client;

#[cfg(feature = "ollama")]
pub(crate) mod ollama_client;

// Ollama-specific agents (ListLocalModels, ShowModelInfo)
#[cfg(feature = "ollama")]
pub mod ollama;
