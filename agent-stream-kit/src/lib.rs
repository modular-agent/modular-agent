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
mod flow;
mod message;
mod output;
mod registry;
mod runtime;
pub mod test_utils;
mod value;

// re-export async_trait
pub use async_trait::async_trait;

// re-export photon_rs
#[cfg(feature = "image")]
pub use photon_rs::{self, PhotonImage};

// Re-export the crate under its canonical name for proc-macros.
pub extern crate self as agent_stream_kit;
pub use inventory;

pub use agent::{
    Agent, AgentData, AgentStatus, AsAgent, HasAgentData, agent_new, downcast_agent_ref,
    new_agent_boxed,
};
pub use askit::{ASKit, ASKitEvent, ASKitObserver};
pub use config::{AgentConfigs, AgentConfigsMap};
pub use context::AgentContext;
pub use definition::{
    AgentConfigEntry, AgentDefaultConfigs, AgentDefinition, AgentDefinitions,
    AgentDisplayConfigEntry,
};
pub use error::AgentError;
pub use flow::{AgentFlow, AgentFlowEdge, AgentFlowNode, AgentFlows};
pub use output::AgentOutput;
pub use registry::AgentRegistration;
pub use value::{AgentValue, AgentValueMap};
