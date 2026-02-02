#![recursion_limit = "256"]
//! Modular Agent Core - A crate for building modular multi-agents intelligent systems.
//!
//! This crate provides a set of tools and abstractions to create, configure, and run agents
//! in a stream-based architecture. It includes support for defining agent behaviors, managing
//! agent flows, handling agent input and output.

mod agent;
mod board_agent;
mod config;
mod context;
mod definition;
mod error;
mod id;
mod modular_agent;
mod message;
mod output;
mod preset;
mod registry;
mod runtime;
mod spec;
mod value;

#[cfg(feature = "llm")]
pub mod llm;
pub mod tool;

#[cfg(feature = "mcp")]
pub mod mcp;

#[cfg(feature = "test-utils")]
pub mod test_utils;

// re-export async_trait
pub use async_trait::async_trait;

// re-export photon_rs
#[cfg(feature = "image")]
pub use photon_rs::{self, PhotonImage};

// re-export im
pub use im;

// re-export inventory
pub use inventory;

// re-export FnvIndexMap
pub use fnv;
pub use indexmap;
pub type FnvIndexMap<K, V> = indexmap::IndexMap<K, V, fnv::FnvBuildHasher>;
pub type FnvIndexSet<T> = indexmap::IndexSet<T, fnv::FnvBuildHasher>;

// Re-export the crate under its canonical name for proc-macros.
pub extern crate self as modular_agent_core;

// Re-exports modular_agent_macros
pub use modular_agent_macros::modular_agent;

pub use agent::{Agent, AgentData, AgentStatus, AsAgent, HasAgentData, agent_new, new_agent_boxed};
pub use config::{AgentConfigs, AgentConfigsMap};
pub use context::AgentContext;
pub use definition::{AgentConfigSpec, AgentConfigSpecs, AgentDefinition, AgentDefinitions};
pub use error::AgentError;
pub use llm::{Message, ToolCall, ToolCallFunction};
pub use modular_agent::{ModularAgent, ModularAgentEvent};
pub use output::AgentOutput;
pub use preset::{Preset, PresetInfo};
pub use registry::AgentRegistration;
pub use spec::{AgentSpec, ConnectionSpec, PresetSpec, PresetSpecs};
pub use value::{AgentValue, AgentValueMap};
