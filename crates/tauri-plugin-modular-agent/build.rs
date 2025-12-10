const COMMANDS: &[&str] = &[
    "get_agent_definition",
    "get_agent_definitions",
    "get_agent_spec",
    "get_agent_flows",
    "new_agent_flow",
    "rename_agent_flow",
    "unique_flow_name",
    "add_agent_flow",
    "remove_agent_flow",
    "insert_agent_flow",
    "copy_sub_flow",
    "start_agent_flow",
    "stop_agent_flow",
    "new_agent_flow_node",
    "add_agent_flow_node",
    "remove_agent_flow_node",
    "add_agent_flow_edge",
    "remove_agent_flow_edge",
    "start_agent",
    "stop_agent",
    "write_board",
    "set_agent_configs",
    "get_global_configs",
    "get_global_configs_map",
    "set_global_configs",
    "set_global_configs_map",
    "get_agent_default_configs",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
