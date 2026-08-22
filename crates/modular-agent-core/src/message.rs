use crate::context::AgentContext;
use crate::error::AgentError;
use crate::modular_agent::ModularAgent;
use crate::value::AgentValue;

/// Internal event messages passed through the agent event loop.
///
/// These messages are used internally by the `ModularAgent` to route values
/// between agents and to/from external inputs/outputs.
#[derive(Clone, Debug)]
pub enum AgentEventMessage {
    /// Output from an agent to be routed to connected agents.
    AgentOut {
        /// ID of the source agent.
        agent: String,
        /// Execution context for tracing.
        ctx: AgentContext,
        /// Output port name.
        port: String,
        /// The value being output.
        value: AgentValue,
    },
    /// Output to an external destination (outside the agent graph).
    ExternalOutput {
        /// Name of the external output.
        name: String,
        /// Execution context for tracing.
        ctx: AgentContext,
        /// The value being output.
        value: AgentValue,
    },
}

/// Sends an agent output message asynchronously.
///
/// This function queues an `AgentOut` message to be processed by the event loop,
/// which will route the value to connected agents.
pub async fn send_agent_out(
    ma: &ModularAgent,
    agent: String,
    ctx: AgentContext,
    port: String,
    value: AgentValue,
) -> Result<(), AgentError> {
    ma.tx()?
        .send(AgentEventMessage::AgentOut {
            agent,
            ctx,
            port,
            value,
        })
        .await
        .map_err(|_| AgentError::SendMessageFailed("Failed to send AgentOut message".to_string()))
}

/// Tries to send an agent output message without blocking.
///
/// Returns immediately even if the channel is full.
/// Use `send_agent_out` if async await is acceptable.
pub fn try_send_agent_out(
    ma: &ModularAgent,
    agent: String,
    ctx: AgentContext,
    port: String,
    value: AgentValue,
) -> Result<(), AgentError> {
    ma.tx()?
        .try_send(AgentEventMessage::AgentOut {
            agent,
            ctx,
            port,
            value,
        })
        .map_err(|_| {
            AgentError::SendMessageFailed("Failed to try_send AgentOut message".to_string())
        })
}

/// Sends an external output message asynchronously.
///
/// This function queues an `ExternalOutput` message to be processed by the event loop,
/// which will emit the value to external listeners.
pub async fn send_external_output(
    ma: &ModularAgent,
    name: String,
    ctx: AgentContext,
    value: AgentValue,
) -> Result<(), AgentError> {
    ma.tx()?
        .send(AgentEventMessage::ExternalOutput { name, ctx, value })
        .await
        .map_err(|_| {
            AgentError::SendMessageFailed("Failed to send ExternalOutput message".to_string())
        })
}

/// Processes an agent output by routing it to connected agents.
///
/// This function looks up all connections from the source agent's output port
/// and forwards the value to each connected agent's input port.
pub async fn agent_out(
    ma: &ModularAgent,
    source_agent: String,
    ctx: AgentContext,
    port: String,
    value: AgentValue,
) {
    let targets;
    {
        let env_edges = ma.connections.lock();
        targets = env_edges.get(&source_agent).cloned();
    }

    if targets.is_none() {
        return;
    }

    for target in targets.unwrap() {
        let (target_agent, source_port, target_port) = target;

        if source_port != port {
            // Skip if source_handle does not match with the given port.
            continue;
        }

        {
            let env_agents = ma.agents.lock();
            if !env_agents.contains_key(&target_agent) {
                continue;
            }
        }

        ma.agent_input(
            target_agent.clone(),
            ctx.clone(),
            target_port,
            value.clone(),
        )
        .await
        .unwrap_or_else(|e| {
            log::error!("Failed to send message to {}: {}", target_agent, e);
        });
    }
}

/// Processes an external input by routing it to connected agents.
///
/// This function:
/// 1. Stores the value in the external values map for later retrieval
/// 2. Finds all external input agents registered for this name
/// 3. Routes the value through their connections to target agents
/// 4. Emits the value as an external output event
pub async fn external_input(ma: &ModularAgent, name: String, ctx: AgentContext, value: AgentValue) {
    {
        let mut external_values = ma.external_values.lock();
        external_values.insert(name.clone(), value.clone());
    }
    let input_nodes;
    {
        let env_input_nodes = ma.external_input_agents.lock();
        input_nodes = env_input_nodes.get(&name).cloned();
    }
    if let Some(input_nodes) = input_nodes {
        for node in input_nodes {
            // Perhaps we could process this by send_message_to ExternalInputAgent

            let edges;
            {
                let env_edges = ma.connections.lock();
                edges = env_edges.get(&node).cloned();
            }
            let Some(edges) = edges else {
                // edges not found
                continue;
            };
            for (target_agent, _source_port, target_port) in edges {
                ma.agent_input(
                    target_agent.clone(),
                    ctx.clone(),
                    target_port,
                    value.clone(),
                )
                .await
                .unwrap_or_else(|e| {
                    log::error!("Failed to send message to {}: {}", target_agent, e);
                });
            }
        }
    }

    ma.emit_external_output(name, value);
}
