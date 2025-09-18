use agent_stream_kit::ASKit;

pub mod ollama;
pub mod openai;
pub mod prompt;

pub fn register_agents(askit: &ASKit) {
    ollama::register_agents(askit);
    openai::register_agents(askit);
    prompt::register_agents(askit);
}
