use agent_stream_kit::ASKit;

pub mod agents;

pub fn register_agents(askit: &ASKit) {
    agents::register_agents(askit);
}
