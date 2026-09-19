#![recursion_limit = "256"]
//! # Modular Agent Core
//!
//! A Rust framework that composes modules into an agent with stream-based message orchestration.
//!
//! This crate provides tools and abstractions to create, configure, and run modules
//! in a stream-based architecture. It supports defining module behaviors, managing
//! module flows, and handling module input/output through a channel-based messaging system.
//!
//! ## Core Concepts
//!
//! ### ModularAgent
//!
//! [`ModularAgent`] is the central orchestrator that manages module lifecycle, connections,
//! and message routing. It maintains module instances, connection maps, and handles events.
//!
//! ### Modules
//!
//! Modules are processing units that receive messages via channels and process them
//! asynchronously. Implement the [`AsModule`] trait to create custom modules, or use the
//! `#[modular_agent]` macro for declarative module definitions.
//!
//! ### Patches
//!
//! Patches are collections of modules and their connections, defined in JSON format.
//! They can be loaded from files and managed via [`ModularAgent`] methods.
//!
//! ## Quick Start
//!
//! See the [CLI example](https://github.com/modular-agent/modular-agent/blob/main/crates/modular-agent-core/examples/cli.rs)
//! for a complete working example of loading a patch and running modules from the command line.
//!
//! ## Feature Flags
//!
//! - `file` - File handling support (enabled by default)
//! - `image` - Image processing with photon-rs (enabled by default)
//! - `llm` - LLM integration with Message/ToolCall types (enabled by default)
//! - `mcp` - Model Context Protocol integration (enabled by default)
//! - `mcp-server` - Built-in MCP server exposing flow-editing tools over streamable HTTP
//! - `test-utils` - Testing utilities including TestProbeModule

mod config;
mod context;
mod definition;
mod error;
mod external_module;
mod id;
mod message;
mod modular_agent;
mod module;
mod output;
mod patch;
mod registry;
mod runtime;
mod spec;
mod value;

#[cfg(feature = "llm")]
pub mod llm;
#[cfg(feature = "llm")]
pub mod session;
pub mod tool;

#[cfg(feature = "mcp")]
pub mod mcp;

#[cfg(feature = "mcp-server")]
pub mod mcp_server;

#[cfg(feature = "test-utils")]
pub mod test_utils;

// re-export async_trait
pub use async_trait::async_trait;

// re-export photon_rs
#[cfg(feature = "image")]
pub use photon_rs::{self, PhotonImage};

// re-export im
pub use im;

// re-export CancellationToken (used by ModuleContext and ModularAgent cancellation APIs)
pub use tokio_util::sync::CancellationToken;

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

pub use config::{ModuleConfigs, ModuleConfigsMap};
pub use context::ModuleContext;
pub use definition::{ModuleConfigSpec, ModuleConfigSpecs, ModuleDefinition, ModuleDefinitions};
pub use error::{Error, Result};
#[cfg(feature = "llm")]
pub use llm::{
    ContentBlock, Message, MessageContent, MessageEvent, ToolCall, ToolCallFunction, Usage,
    estimate_context_tokens, estimate_message_tokens,
};
pub use modular_agent::{EventEnvelope, ModularAgent, ModularAgentEvent, SharedModule};
pub use module::{AsModule, HasModuleData, Module, ModuleData, ModuleStatus, new_module_boxed};
pub use output::ModuleOutput;
pub use patch::{Patch, PatchInfo};
pub use registry::ModuleRegistration;
#[cfg(feature = "llm")]
pub use session::{
    InMemorySessionStore, JsonlSessionStore, SessionEntry, SessionMeta, SessionStore,
    build_context, build_context_with_ids,
};
pub use spec::{ConnectionSpec, ModuleSpec, PatchSpec, PatchSpecs};
pub use value::{Value, ValueMap, parse_index};
