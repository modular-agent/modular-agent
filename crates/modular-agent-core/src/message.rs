use crate::context::ModuleContext;
use crate::error::{Error, Result};
use crate::modular_agent::ModularAgent;
use crate::value::Value;

/// Internal event messages passed through the module event loop.
///
/// These messages are used internally by the `ModularAgent` to route values
/// between modules and to/from external inputs/outputs.
#[derive(Clone, Debug)]
pub enum ModuleEventMessage {
    /// Output from a module to be routed to connected modules.
    ModuleOut {
        /// ID of the source module.
        module: String,
        /// Execution context for tracing.
        ctx: ModuleContext,
        /// Output port name.
        port: String,
        /// The value being output.
        value: Value,
    },
    /// Output to an external destination (outside the module graph).
    ExternalOutput {
        /// Name of the external output.
        name: String,
        /// Execution context for tracing.
        ctx: ModuleContext,
        /// The value being output.
        value: Value,
    },
}

/// Sends a module output message asynchronously.
///
/// This function queues a `ModuleOut` message to be processed by the event loop,
/// which will route the value to connected modules.
pub async fn send_module_out(
    ma: &ModularAgent,
    module: String,
    ctx: ModuleContext,
    port: String,
    value: Value,
) -> Result<()> {
    ma.tx()?
        .send(ModuleEventMessage::ModuleOut {
            module,
            ctx,
            port,
            value,
        })
        .map_err(|_| Error::SendMessageFailed("Failed to send ModuleOut message".to_string()))
}

/// Sends a module output message from a synchronous context.
///
/// The queue is unbounded, so enqueueing always succeeds immediately;
/// this fails only when the message loop has shut down.
pub fn try_send_module_out(
    ma: &ModularAgent,
    module: String,
    ctx: ModuleContext,
    port: String,
    value: Value,
) -> Result<()> {
    ma.tx()?
        .send(ModuleEventMessage::ModuleOut {
            module,
            ctx,
            port,
            value,
        })
        .map_err(|_| Error::SendMessageFailed("Failed to try_send ModuleOut message".to_string()))
}

/// Sends an external output message asynchronously.
///
/// This function queues an `ExternalOutput` message to be processed by the event loop,
/// which will emit the value to external listeners.
pub async fn send_external_output(
    ma: &ModularAgent,
    name: String,
    ctx: ModuleContext,
    value: Value,
) -> Result<()> {
    ma.tx()?
        .send(ModuleEventMessage::ExternalOutput { name, ctx, value })
        .map_err(|_| Error::SendMessageFailed("Failed to send ExternalOutput message".to_string()))
}

/// Processes a module output by routing it to connected modules.
///
/// This function looks up all connections from the source module's output port
/// and forwards the value to each connected module's input port.
pub async fn module_out(
    ma: &ModularAgent,
    source_module: String,
    ctx: ModuleContext,
    port: String,
    value: Value,
) {
    let targets;
    {
        let env_edges = ma.connections.lock();
        targets = env_edges.get(&source_module).cloned();
    }

    if targets.is_none() {
        return;
    }

    for target in targets.unwrap() {
        let (target_module, source_port, target_port) = target;

        if source_port != port {
            // Skip if source_handle does not match with the given port.
            continue;
        }

        {
            let env_modules = ma.modules.lock();
            if !env_modules.contains_key(&target_module) {
                continue;
            }
        }

        ma.module_input(
            target_module.clone(),
            ctx.clone(),
            target_port,
            value.clone(),
        )
        .await
        .unwrap_or_else(|e| {
            log::error!("Failed to send message to {}: {}", target_module, e);
        });
    }
}

/// Processes an external input by routing it to connected modules.
///
/// This function:
/// 1. Stores the value in the external values map for later retrieval
/// 2. Finds all external input modules registered for this name
/// 3. Routes the value through their connections to target modules
/// 4. Emits the value as an external output event
pub async fn external_input(ma: &ModularAgent, name: String, ctx: ModuleContext, value: Value) {
    {
        let mut external_values = ma.external_values.lock();
        external_values.insert(name.clone(), value.clone());
    }
    let input_nodes;
    {
        let env_input_nodes = ma.external_input_modules.lock();
        input_nodes = env_input_nodes.get(&name).cloned();
    }
    if let Some(input_nodes) = input_nodes {
        for node in input_nodes {
            // Perhaps we could process this by send_message_to ExternalInputModule

            let edges;
            {
                let env_edges = ma.connections.lock();
                edges = env_edges.get(&node).cloned();
            }
            let Some(edges) = edges else {
                // edges not found
                continue;
            };
            for (target_module, _source_port, target_port) in edges {
                ma.module_input(
                    target_module.clone(),
                    ctx.clone(),
                    target_port,
                    value.clone(),
                )
                .await
                .unwrap_or_else(|e| {
                    log::error!("Failed to send message to {}: {}", target_module, e);
                });
            }
        }
    }

    ma.emit_external_output(name, value);
}
