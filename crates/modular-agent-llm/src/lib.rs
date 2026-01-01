#![recursion_limit = "256"]

pub mod doc;
pub mod message;
pub mod message_lib;
pub mod tool;

#[cfg(feature = "mcp")]
pub mod mcp;

#[cfg(feature = "ollama")]
pub mod ollama;

#[cfg(feature = "openai")]
pub mod openai;

#[cfg(feature = "sakura")]
pub mod sakura_ai;
