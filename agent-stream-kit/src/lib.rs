#![recursion_limit = "256"]
//! Agent Stream Kit - A framework for building and managing agents in Rust
//!
//! This crate provides a set of tools and abstractions to create, configure, and run agents
//! in a stream-based architecture. It includes support for defining agent behaviors, managing
//! agent flows, handling agent input and output.

mod agent;
mod askit;
mod board_agent;
mod config;
mod context;
mod definition;
mod error;
mod id;
mod llm;
mod message;
mod output;
mod registry;
mod runtime;
mod spec;
mod stream;
pub mod tool;
mod value;

#[cfg(feature = "mcp")]
mod mcp;

#[cfg(feature = "test-utils")]
pub mod test_utils;

// re-export async_trait
pub use async_trait::async_trait;

// re-export photon_rs
#[cfg(feature = "image")]
pub use photon_rs::{self, PhotonImage};

// Re-export the crate under its canonical name for proc-macros.
pub extern crate self as agent_stream_kit;
pub use inventory;

// re-export FnvIndexMap
pub use fnv;
pub use indexmap;
pub type FnvIndexMap<K, V> = indexmap::IndexMap<K, V, fnv::FnvBuildHasher>;
pub type FnvIndexSet<T> = indexmap::IndexSet<T, fnv::FnvBuildHasher>;

// Re-exports askit_macros
pub use askit_macros::askit_agent;

pub use agent::{Agent, AgentData, AgentStatus, AsAgent, HasAgentData, agent_new, new_agent_boxed};
pub use askit::{ASKit, ASKitEvent};
pub use config::{AgentConfigs, AgentConfigsMap};
pub use context::AgentContext;
pub use definition::{AgentConfigSpec, AgentConfigSpecs, AgentDefinition, AgentDefinitions};
pub use error::AgentError;
pub use llm::{Message, ToolCall, ToolCallFunction};
pub use output::AgentOutput;
pub use registry::AgentRegistration;
pub use spec::{AgentSpec, AgentStreamSpec, AgentStreamSpecs, ChannelSpec};
pub use stream::{AgentStream, AgentStreamInfo, AgentStreams};
pub use value::{AgentValue, AgentValueMap};
