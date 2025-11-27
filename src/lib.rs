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
mod runtime;
mod value;

pub use agent::{Agent, AgentStatus, AsAgent, AsAgentData, agent_new, new_agent_boxed};
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
pub use value::{AgentValue, AgentValueMap};

// re-export async_trait
pub use async_trait::async_trait;
