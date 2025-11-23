use agent_stream_kit::ASKit;

pub mod counter;
pub mod data;
pub mod display;
pub mod file;
pub mod image;
pub mod input;
pub mod stream;
pub mod string;
pub mod time;

#[cfg(feature = "yaml")]
pub mod yaml;

pub fn register_agents(askit: &ASKit) {
    counter::register_agents(askit);
    data::register_agents(askit);
    display::register_agents(askit);
    file::register_agents(askit);
    image::register_agents(askit);
    input::register_agents(askit);
    stream::register_agents(askit);
    string::register_agents(askit);
    time::register_agents(askit);

    #[cfg(feature = "yaml")]
    yaml::register_agents(askit);
}
