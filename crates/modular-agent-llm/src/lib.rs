#![recursion_limit = "256"]

pub mod doc;
pub mod message;
pub mod provider;

pub mod chat;
pub mod completion;
pub mod embeddings;

#[cfg(feature = "openai")]
pub mod responses;

#[cfg(feature = "openai")]
pub(crate) mod openai_client;

#[cfg(feature = "ollama")]
pub(crate) mod ollama_client;

#[cfg(feature = "ollama")]
pub mod ollama;
