use agent_stream_kit::ASKit;

pub mod ollama;

pub fn register_agents(askit: &ASKit) {
    ollama::register_agents(askit);
}
