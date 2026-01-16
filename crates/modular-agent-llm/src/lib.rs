#![recursion_limit = "256"]

pub mod doc;
pub mod message;

#[cfg(feature = "ollama")]
pub mod ollama;

#[cfg(feature = "openai")]
pub mod openai;
