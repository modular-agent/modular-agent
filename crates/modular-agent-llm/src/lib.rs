#![recursion_limit = "256"]

pub mod doc;
pub mod message;
pub mod tool;

#[cfg(feature = "mcp")]
pub mod mcp;

#[cfg(feature = "ollama")]
pub mod ollama;

#[cfg(feature = "openai")]
pub mod openai;
