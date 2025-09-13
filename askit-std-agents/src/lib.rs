use agent_stream_kit::ASKit;

pub mod counter;
pub mod data;
pub mod display;
pub mod input;
pub mod string;
pub mod task;
pub mod time;

pub fn register_agents(askit: &ASKit) {
    counter::register_agents(askit);
    data::register_agents(askit);
    display::register_agents(askit);
    input::register_agents(askit);
    task::register_agents(askit);
    string::register_agents(askit);
    time::register_agents(askit);
}
