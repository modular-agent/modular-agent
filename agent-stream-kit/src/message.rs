use super::askit::ASKit;
use super::context::AgentContext;
use super::data::AgentData;
use super::error::AgentError;

#[derive(Clone, Debug)]
pub enum AgentEventMessage {
    AgentOut {
        agent: String,
        ctx: AgentContext,
        data: AgentData,
    },
    BoardOut {
        name: String,
        ctx: AgentContext,
        data: AgentData,
    },
}

pub async fn send_agent_out(
    askit: &ASKit,
    agent: String,
    ctx: AgentContext,
    data: AgentData,
) -> Result<(), AgentError> {
    askit
        .tx()?
        .send(AgentEventMessage::AgentOut { agent, ctx, data })
        .await
        .map_err(|_| AgentError::SendMessageFailed("Failed to send AgentOut message".to_string()))
}

pub fn try_send_agent_out(
    askit: &ASKit,
    agent: String,
    ctx: AgentContext,
    data: AgentData,
) -> Result<(), AgentError> {
    askit
        .tx()?
        .try_send(AgentEventMessage::AgentOut { agent, ctx, data })
        .map_err(|_| {
            AgentError::SendMessageFailed("Failed to try_send AgentOut message".to_string())
        })
}

pub fn try_send_board_out(
    askit: &ASKit,
    name: String,
    ctx: AgentContext,
    data: AgentData,
) -> Result<(), AgentError> {
    askit
        .tx()?
        .try_send(AgentEventMessage::BoardOut { name, ctx, data })
        .map_err(|_| {
            AgentError::SendMessageFailed("Failed to try_send BoardOut message".to_string())
        })
}

// Processing AgentOut message
pub async fn agent_out(env: &ASKit, source_agent: String, ctx: AgentContext, data: AgentData) {
    let targets;
    {
        let env_edges = env.edges.lock().unwrap();
        targets = env_edges.get(&source_agent).cloned();
    }

    if targets.is_none() {
        return;
    }

    for target in targets.unwrap() {
        let (target_agent, source_handle, target_handle) = target;

        if source_handle != ctx.port() && source_handle != "*" {
            // Skip if source_handle does not match with the given port.
            // "*" is a wildcard, and outputs messages of all ports.
            continue;
        }

        {
            let env_agents = env.agents.lock().unwrap();
            if !env_agents.contains_key(&target_agent) {
                continue;
            }
        }

        let target_port = if target_handle == "*" {
            // If target_handle is "*", use the port specified by the source agent
            ctx.port().to_string()
        } else {
            target_handle.clone()
        };

        let target_ctx = ctx.with_port(target_port);

        env.agent_input(target_agent.clone(), target_ctx, data.clone())
            .await
            .unwrap_or_else(|e| {
                log::error!("Failed to send message to {}: {}", target_agent, e);
            });
    }
}

pub async fn board_out(env: &ASKit, name: String, ctx: AgentContext, data: AgentData) {
    let board_nodes;
    {
        let env_board_nodes = env.board_out_agents.lock().unwrap();
        board_nodes = env_board_nodes.get(&name).cloned();
    }
    if let Some(board_nodes) = board_nodes {
        for node in board_nodes {
            // Perhaps we could process this by send_message_to BoardOutAgent

            let edges;
            {
                let env_edges = env.edges.lock().unwrap();
                edges = env_edges.get(&node).cloned();
            }
            let Some(edges) = edges else {
                // edges not found
                continue;
            };
            for (target_agent, _source_handle, target_handle) in edges {
                let target_port = if target_handle == "*" {
                    // If target_handle is "*", use the board name
                    name.clone()
                } else {
                    target_handle.clone()
                };
                let target_ctx = ctx.with_port(target_port);
                env.agent_input(target_agent.clone(), target_ctx, data.clone())
                    .await
                    .unwrap_or_else(|e| {
                        log::error!("Failed to send message to {}: {}", target_agent, e);
                    });
            }
        }
    }

    env.emit_board(name, data);
}
