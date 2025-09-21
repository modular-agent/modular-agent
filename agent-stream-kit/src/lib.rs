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
mod data;
mod definition;
mod error;
mod flow;
mod message;
mod output;
mod runtime;

pub use agent::{Agent, AgentStatus, AsAgent, AsAgentData, new_agent_boxed};
pub use askit::{ASKit, ASKitEvent, ASKitObserver};
pub use config::{AgentConfig, AgentConfigs};
pub use context::AgentContext;
pub use data::{AgentData, AgentValue, AgentValueMap};
pub use definition::{
    AgentConfigEntry, AgentDefinition, AgentDefinitions, AgentDisplayConfigEntry,
};
pub use error::AgentError;
pub use flow::{AgentFlow, AgentFlowEdge, AgentFlowNode, AgentFlows};
pub use output::AgentOutput;

// re-export async_trait
pub use async_trait::async_trait;
