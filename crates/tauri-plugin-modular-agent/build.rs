const COMMANDS: &[&str] = &[
    "get_agent_definition",
    "get_agent_definitions",
    "get_agent_spec",
    "get_agent_streams",
    "new_agent_stream",
    "rename_agent_stream",
    "unique_stream_name",
    "add_agent_stream",
    "remove_agent_stream",
    "insert_agent_stream",
    "copy_sub_stream",
    "start_agent_stream",
    "stop_agent_stream",
    "new_agent_stream_node",
    "add_agent_stream_node",
    "remove_agent_stream_node",
    "add_agent_stream_edge",
    "remove_agent_stream_edge",
    "start_agent",
    "stop_agent",
    "write_board",
    "set_agent_configs",
    "get_global_configs",
    "get_global_configs_map",
    "set_global_configs",
    "set_global_configs_map",
    "get_agent_config_specs",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
