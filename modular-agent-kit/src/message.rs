use crate::mak::MAK;
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
    BoardOut {
        name: String,
        ctx: AgentContext,
        value: AgentValue,
    },
}

pub async fn send_agent_out(
    mak: &MAK,
    agent: String,
    ctx: AgentContext,
    port: String,
    value: AgentValue,
) -> Result<(), AgentError> {
    mak
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
    mak: &MAK,
    agent: String,
    ctx: AgentContext,
    port: String,
    value: AgentValue,
) -> Result<(), AgentError> {
    mak
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

pub async fn send_board_out(
    mak: &MAK,
    name: String,
    ctx: AgentContext,
    value: AgentValue,
) -> Result<(), AgentError> {
    mak
        .tx()?
        .send(AgentEventMessage::BoardOut { name, ctx, value })
        .await
        .map_err(|_| {
            AgentError::SendMessageFailed("Failed to try_send BoardOut message".to_string())
        })
}

// Processing AgentOut message
pub async fn agent_out(
    mak: &MAK,
    source_agent: String,
    ctx: AgentContext,
    port: String,
    value: AgentValue,
) {
    let targets;
    {
        let env_edges = mak.connections.lock().unwrap();
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
            let env_agents = mak.agents.lock().unwrap();
            if !env_agents.contains_key(&target_agent) {
                continue;
            }
        }

        mak
            .agent_input(target_agent.clone(), ctx.clone(), target_port, value.clone())
            .await
            .unwrap_or_else(|e| {
                log::error!("Failed to send message to {}: {}", target_agent, e);
            });
    }
}

pub async fn board_out(mak: &MAK, name: String, ctx: AgentContext, value: AgentValue) {
    {
        let mut board_value = mak.board_value.lock().unwrap();
        board_value.insert(name.clone(), value.clone());
    }
    let board_nodes;
    {
        let env_board_nodes = mak.board_out_agents.lock().unwrap();
        board_nodes = env_board_nodes.get(&name).cloned();
    }
    if let Some(board_nodes) = board_nodes {
        for node in board_nodes {
            // Perhaps we could process this by send_message_to BoardOutAgent

            let edges;
            {
                let env_edges = mak.connections.lock().unwrap();
                edges = env_edges.get(&node).cloned();
            }
            let Some(edges) = edges else {
                // edges not found
                continue;
            };
            for (target_agent, _source_port, target_port) in edges {
                mak
                    .agent_input(target_agent.clone(), ctx.clone(), target_port, value.clone())
                    .await
                    .unwrap_or_else(|e| {
                        log::error!("Failed to send message to {}: {}", target_agent, e);
                    });
            }
        }
    }

    mak.emit_board(name, value);
}
