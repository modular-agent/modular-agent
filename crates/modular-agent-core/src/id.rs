use std::sync::atomic::AtomicUsize;

use crate::{
    FnvIndexMap,
    spec::{AgentSpec, ConnectionSpec},
};

static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

pub(crate) fn new_id() -> String {
    ID_COUNTER
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .to_string()
}

pub(crate) fn update_ids(
    agents: &Vec<AgentSpec>,
    connections: &Vec<ConnectionSpec>,
) -> (Vec<AgentSpec>, Vec<ConnectionSpec>) {
    let mut new_agents = Vec::new();
    let mut agent_id_map = FnvIndexMap::default();
    for agent in agents {
        let new_id = new_id();
        agent_id_map.insert(agent.id.clone(), new_id.clone());
        let mut new_agent = agent.clone();
        new_agent.id = new_id;
        new_agents.push(new_agent);
    }

    let mut new_connections = Vec::new();
    for connection in connections {
        let source = agent_id_map
            .get(&connection.source)
            .cloned()
            .unwrap_or_else(|| connection.source.clone());
        let target = agent_id_map
            .get(&connection.target)
            .cloned()
            .unwrap_or_else(|| connection.target.clone());
        let mut new_connection = connection.clone();
        new_connection.source = source.clone();
        new_connection.target = target.clone();
        new_connections.push(new_connection);
    }

    (new_agents, new_connections)
}
