use agent_stream_kit::ASKit;

pub mod common;
pub mod message;

#[cfg(feature = "ollama")]
pub mod ollama;

#[cfg(feature = "openai")]
pub mod openai;

#[cfg(feature = "sakura")]
pub mod sakura_ai;

pub fn register_agents(askit: &ASKit) {
    common::register_agents(askit);

    #[cfg(feature = "ollama")]
    ollama::register_agents(askit);

    #[cfg(feature = "openai")]
    openai::register_agents(askit);

    #[cfg(feature = "sakura")]
    sakura_ai::register_agents(askit);
}
