use crate::modular_agent::ModularAgent;
use crate::context::AgentContext;
use crate::error::AgentError;
use crate::value::AgentValue;

#[derive(Clone, Debug)]
pub enum AgentEventMessage {
    AgentOut {
        agent: String,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    },
    ExternalOutput {
        name: String,
        ctx: AgentContext,
        value: AgentValue,
    },
}

pub async fn send_agent_out(
    ma: &ModularAgent,
    agent: String,
    ctx: AgentContext,
    port: String,
    value: AgentValue,
) -> Result<(), AgentError> {
    ma
        .tx()?
        .send(AgentEventMessage::AgentOut {
            agent,
            ctx,
            port,
            value,
        })
        .await
        .map_err(|_| AgentError::SendMessageFailed("Failed to send AgentOut message".to_string()))
}

pub fn try_send_agent_out(
    ma: &ModularAgent,
    agent: String,
    ctx: AgentContext,
    port: String,
    value: AgentValue,
) -> Result<(), AgentError> {
    ma
        .tx()?
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

pub async fn send_external_output(
    ma: &ModularAgent,
    name: String,
    ctx: AgentContext,
    value: AgentValue,
) -> Result<(), AgentError> {
    ma
        .tx()?
        .send(AgentEventMessage::ExternalOutput { name, ctx, value })
        .await
        .map_err(|_| {
            AgentError::SendMessageFailed("Failed to send ExternalOutput message".to_string())
        })
}

// Processing AgentOut message
pub async fn agent_out(
    ma: &ModularAgent,
    source_agent: String,
    ctx: AgentContext,
    port: String,
    value: AgentValue,
) {
    let targets;
    {
        let env_edges = ma.connections.lock().unwrap();
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
            let env_agents = ma.agents.lock().unwrap();
            if !env_agents.contains_key(&target_agent) {
                continue;
            }
        }

        ma
            .agent_input(target_agent.clone(), ctx.clone(), target_port, value.clone())
            .await
            .unwrap_or_else(|e| {
                log::error!("Failed to send message to {}: {}", target_agent, e);
            });
    }
}

pub async fn external_input(ma: &ModularAgent, name: String, ctx: AgentContext, value: AgentValue) {
    {
        let mut external_values = ma.external_values.lock().unwrap();
        external_values.insert(name.clone(), value.clone());
    }
    let input_nodes;
    {
        let env_input_nodes = ma.external_input_agents.lock().unwrap();
        input_nodes = env_input_nodes.get(&name).cloned();
    }
    if let Some(input_nodes) = input_nodes {
        for node in input_nodes {
            // Perhaps we could process this by send_message_to ExternalInputAgent

            let edges;
            {
                let env_edges = ma.connections.lock().unwrap();
                edges = env_edges.get(&node).cloned();
            }
            let Some(edges) = edges else {
                // edges not found
                continue;
            };
            for (target_agent, _source_port, target_port) in edges {
                ma
                    .agent_input(target_agent.clone(), ctx.clone(), target_port, value.clone())
                    .await
                    .unwrap_or_else(|e| {
                        log::error!("Failed to send message to {}: {}", target_agent, e);
                    });
            }
        }
    }

    ma.emit_external_output(name, value);
}
