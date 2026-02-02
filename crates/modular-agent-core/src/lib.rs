#![recursion_limit = "256"]
//! # Modular Agent Core
//!
//! A Rust framework for building modular multi-agent orchestration systems.
//!
//! This crate provides tools and abstractions to create, configure, and run agents
//! in a stream-based architecture. It supports defining agent behaviors, managing
//! agent flows, and handling agent input/output through a channel-based messaging system.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use modular_agent_core::{AgentError, AgentValue, ModularAgent, ModularAgentEvent};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), AgentError> {
//!     // 1. Initialize and prepare ModularAgent
//!     let ma = ModularAgent::init()?;
//!     ma.ready().await?;
//!
//!     // 2. Subscribe to output events BEFORE starting preset (avoid race condition)
//!     let output_name = "output".to_string();
//!     let mut output_rx = ma.subscribe_to_event(move |event| {
//!         if let ModularAgentEvent::ExternalOutput(name, value) = event {
//!             if name == output_name {
//!                 return Some(value);
//!             }
//!         }
//!         None
//!     });
//!
//!     // 3. Load and start a preset from file
//!     let preset_id = ma.open_preset_from_file("preset.json", None).await?;
//!     ma.start_preset(&preset_id).await?;
//!
//!     // 4. Send input to the agent network via external input
//!     ma.write_external_input("input".to_string(), AgentValue::string("Hello")).await?;
//!
//!     // 5. Receive output from the agent network
//!     if let Some(value) = output_rx.recv().await {
//!         println!("Output: {:?}", value);
//!     }
//!
//!     // 6. Cleanup
//!     ma.stop_preset(&preset_id).await?;
//!     ma.quit();
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Core Concepts
//!
//! ### ModularAgent
//!
//! [`ModularAgent`] is the central orchestrator that manages agent lifecycle, connections,
//! and message routing. It maintains agent instances, connection maps, and handles events.
//!
//! ### Agents
//!
//! Agents are processing units that receive messages via channels and process them
//! asynchronously. Implement the [`AsAgent`] trait to create custom agents, or use the
//! `#[modular_agent]` macro for declarative agent definitions.
//!
//! ### External I/O
//!
//! External agents provide I/O to the agent network:
//!
//! - **ExternalInputAgent** (`ExtIn->`): Entry point for external input. When you call
//!   [`ModularAgent::write_external_input`], the value is delivered to all `ExternalInputAgent`
//!   instances listening to that name, which then forward it to connected agents.
//!
//! - **ExternalOutputAgent** (`->ExtOut`): Exit point for external output. When an agent sends
//!   a value to an `ExternalOutputAgent`, it broadcasts to that channel, triggering a
//!   [`ModularAgentEvent::ExternalOutput`] event.
//!
//! ### Presets
//!
//! Presets are collections of agents and their connections, defined in JSON format.
//! They can be loaded from files and managed via [`ModularAgent`] methods.
//!
//! ## Feature Flags
//!
//! - `file` - File handling support (enabled by default)
//! - `image` - Image processing with photon-rs (enabled by default)
//! - `llm` - LLM integration with Message/ToolCall types (enabled by default)
//! - `mcp` - Model Context Protocol integration (enabled by default)
//! - `test-utils` - Testing utilities including TestProbeAgent

mod agent;
mod external_agent;
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
