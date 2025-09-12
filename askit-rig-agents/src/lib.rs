use agent_stream_kit::ASKit;

pub mod rig;

pub fn register_agents(askit: &ASKit) {
    rig::register_agents(askit);
}
