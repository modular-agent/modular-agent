use crate::askit::ASKit;
use crate::context::AgentContext;
use crate::error::AgentError;
use crate::value::AgentValue;

#[derive(Clone, Debug)]
pub enum AgentEventMessage {
    AgentOut {
        agent: String,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    },
    BoardOut {
        name: String,
        ctx: AgentContext,
        value: AgentValue,
    },
}

pub async fn send_agent_out(
    askit: &ASKit,
    agent: String,
    ctx: AgentContext,
    pin: String,
    value: AgentValue,
) -> Result<(), AgentError> {
    askit
        .tx()?
        .send(AgentEventMessage::AgentOut {
            agent,
            ctx,
            pin,
            value,
        })
        .await
        .map_err(|_| AgentError::SendMessageFailed("Failed to send AgentOut message".to_string()))
}

pub fn try_send_agent_out(
    askit: &ASKit,
    agent: String,
    ctx: AgentContext,
    pin: String,
    value: AgentValue,
) -> Result<(), AgentError> {
    askit
        .tx()?
        .try_send(AgentEventMessage::AgentOut {
            agent,
            ctx,
            pin,
            value,
        })
        .map_err(|_| {
            AgentError::SendMessageFailed("Failed to try_send AgentOut message".to_string())
        })
}

pub async fn send_board_out(
    askit: &ASKit,
    name: String,
    ctx: AgentContext,
    value: AgentValue,
) -> Result<(), AgentError> {
    askit
        .tx()?
        .send(AgentEventMessage::BoardOut { name, ctx, value })
        .await
        .map_err(|_| {
            AgentError::SendMessageFailed("Failed to try_send BoardOut message".to_string())
        })
}

// Processing AgentOut message
pub async fn agent_out(
    askit: &ASKit,
    source_agent: String,
    ctx: AgentContext,
    pin: String,
    value: AgentValue,
) {
    let targets;
    {
        let env_edges = askit.channels.lock().unwrap();
        targets = env_edges.get(&source_agent).cloned();
    }

    if targets.is_none() {
        return;
    }

    for target in targets.unwrap() {
        let (target_agent, source_pin, target_pin) = target;

        if source_pin != pin {
            // Skip if source_handle does not match with the given port.
            continue;
        }

        {
            let env_agents = askit.agents.lock().unwrap();
            if !env_agents.contains_key(&target_agent) {
                continue;
            }
        }

        askit
            .agent_input(target_agent.clone(), ctx.clone(), target_pin, value.clone())
            .await
            .unwrap_or_else(|e| {
                log::error!("Failed to send message to {}: {}", target_agent, e);
            });
    }
}

pub async fn board_out(askit: &ASKit, name: String, ctx: AgentContext, value: AgentValue) {
    {
        let mut board_value = askit.board_value.lock().unwrap();
        board_value.insert(name.clone(), value.clone());
    }
    let board_nodes;
    {
        let env_board_nodes = askit.board_out_agents.lock().unwrap();
        board_nodes = env_board_nodes.get(&name).cloned();
    }
    if let Some(board_nodes) = board_nodes {
        for node in board_nodes {
            // Perhaps we could process this by send_message_to BoardOutAgent

            let edges;
            {
                let env_edges = askit.channels.lock().unwrap();
                edges = env_edges.get(&node).cloned();
            }
            let Some(edges) = edges else {
                // edges not found
                continue;
            };
            for (target_agent, _source_pin, target_pin) in edges {
                askit
                    .agent_input(target_agent.clone(), ctx.clone(), target_pin, value.clone())
                    .await
                    .unwrap_or_else(|e| {
                        log::error!("Failed to send message to {}: {}", target_agent, e);
                    });
            }
        }
    }

    askit.emit_board(name, value);
}
