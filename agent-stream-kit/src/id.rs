use std::sync::atomic::AtomicUsize;

use im::Vector;

use crate::{
    FnvIndexMap,
    spec::{AgentSpec, ChannelSpec},
};

static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

pub(crate) fn new_id() -> String {
    return ID_COUNTER
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .to_string();
}

pub(crate) fn update_ids(
    agents: &Vector<AgentSpec>,
    channels: &Vector<ChannelSpec>,
) -> (Vector<AgentSpec>, Vector<ChannelSpec>) {
    let mut new_agents = Vector::new();
    let mut agent_id_map = FnvIndexMap::default();
    for agent in agents {
        let new_id = new_id();
        agent_id_map.insert(agent.id.clone(), new_id.clone());
        let mut new_agent = agent.clone();
        new_agent.id = new_id;
        new_agents.push_back(new_agent);
    }

    let mut new_channels = Vector::new();
    for channel in channels {
        let Some(source) = agent_id_map.get(&channel.source) else {
            continue;
        };
        let Some(target) = agent_id_map.get(&channel.target) else {
            continue;
        };
        let mut new_channel = channel.clone();
        new_channel.source = source.clone();
        new_channel.target = target.clone();
        new_channels.push_back(new_channel);
    }

    (new_agents, new_channels)
}
