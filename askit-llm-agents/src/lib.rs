use agent_stream_kit::ASKit;

pub mod message;
pub mod ollama;
pub mod sakura_ai;

pub fn register_agents(askit: &ASKit) {
    ollama::register_agents(askit);
    sakura_ai::register_agents(askit);
}
