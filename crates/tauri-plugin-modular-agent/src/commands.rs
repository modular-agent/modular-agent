use agent_stream_kit::{AgentConfigs, AgentDefinition, AgentDefinitions, AgentFlowEdge, AgentFlowNode, AgentFlows};
use agent_stream_kit::{
    AgentConfig, AgentData, AgentDefaultConfig, AgentFlow,
};
use tauri::{AppHandle, Runtime};

use crate::Result;
use crate::ASKitExt;

// agent definition

#[tauri::command]
pub(crate) fn get_agent_definition<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
) -> Option<AgentDefinition> {
    app.askit().get_agent_definition(&def_name)
}

#[tauri::command]
pub(crate) fn get_agent_definitions<R: Runtime>(app: AppHandle<R>) -> AgentDefinitions {
    app.askit().get_agent_definitions()
}

// flow

#[tauri::command]
pub(crate) fn get_agent_flows<R: Runtime>(app: AppHandle<R>) -> AgentFlows {
    app.askit().get_agent_flows()
}

#[tauri::command]
pub(crate) fn new_agent_flow<R: Runtime>(
    app: AppHandle<R>,
    flow_name: String,
) -> Result<AgentFlow> {
    app.askit().new_agent_flow(&flow_name).map_err(Into::into)
}

#[tauri::command]
pub(crate) fn rename_agent_flow<R: Runtime>(
    app: AppHandle<R>,
    old_name: String,
    new_name: String,
) -> Result<String> {
    app.askit()
        .rename_agent_flow(&old_name, &new_name)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn unique_flow_name<R: Runtime>(
    app: tauri::AppHandle<R>,
    name: String,
 ) -> String {
    app.askit().unique_flow_name(&name)
}

#[tauri::command]
pub(crate) fn add_agent_flow<R: Runtime>(
    app: AppHandle<R>,
    agent_flow: AgentFlow,
) -> Result<()> {
    app.askit()
        .add_agent_flow(&agent_flow)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn remove_agent_flow<R: Runtime>(
    app: tauri::AppHandle<R>,
    flow_name: String,
 ) -> Result<()> {
    app.askit()
        .remove_agent_flow(&flow_name)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn insert_agent_flow<R: Runtime>(
    app: AppHandle<R>,
    agent_flow: AgentFlow,
) -> Result<()> {
    app.askit()
        .insert_agent_flow(agent_flow)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn copy_sub_flow<R: Runtime>(
    app: AppHandle<R>,
    nodes: Vec<AgentFlowNode>,
    edges: Vec<AgentFlowEdge>,
) -> (Vec<AgentFlowNode>, Vec<AgentFlowEdge>) {
    app.askit().copy_sub_flow(&nodes, &edges)
}

#[tauri::command]
pub(crate) async fn start_agent_flow<R: Runtime>(
    app: AppHandle<R>,
    flow_name: String,
) -> Result<()> {
    app.askit()
        .start_agent_flow(&flow_name)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn stop_agent_flow<R: Runtime>(
    app: AppHandle<R>,
    flow_name: String,
) -> Result<()> {
    app.askit()
        .stop_agent_flow(&flow_name)
        .await
        .map_err(Into::into)
}

// node

#[tauri::command]
pub fn new_agent_flow_node<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
) -> Result<AgentFlowNode> {
    app.askit().new_agent_flow_node(&def_name).map_err(Into::into)
}

#[tauri::command]
pub(crate) fn add_agent_flow_node<R: Runtime>(
    app: AppHandle<R>,
    flow_name: String,
    node: AgentFlowNode,
) -> Result<()> {
    app.askit().add_agent_flow_node(&flow_name, &node).map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn remove_agent_flow_node<R: Runtime>(
    app: AppHandle<R>,
    flow_name: String,
    node_id: String,
) -> Result<()> {
    app.askit().remove_agent_flow_node(&flow_name, &node_id).await.map_err(Into::into)
}

// edge

#[tauri::command]
pub(crate) fn add_agent_flow_edge<R: Runtime>(
    app: AppHandle<R>,
    flow_name: String,
    edge: AgentFlowEdge,
) -> Result<()> {
    app.askit().add_agent_flow_edge(&flow_name, &edge).map_err(Into::into)
}

#[tauri::command]
pub(crate) fn remove_agent_flow_edge<R: Runtime>(
    app: AppHandle<R>,
    flow_name: String,
    edge_id: String,
) -> Result<()> {
    app.askit().remove_agent_flow_edge(&flow_name, &edge_id).map_err(Into::into)
}

// agent

#[tauri::command]
pub(crate) async fn start_agent<R: Runtime>(
    app: AppHandle<R>,
    agent_id: String,
) -> Result<()> {
    app.askit()
        .start_agent(&agent_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn stop_agent<R: Runtime>(
    app: AppHandle<R>,
    agent_id: String,
) -> Result<()> {
    app.askit()
        .stop_agent(&agent_id)
        .await
        .map_err(Into::into)
}

// board commands

#[tauri::command]
pub(crate) fn write_board<R: Runtime>(
    app: AppHandle<R>,
    board: String,
    message: String,
) -> Result<()> {
    app.askit().write_board_data(board, AgentData::string(message)).map_err(Into::into)
}

// config

#[tauri::command]
pub(crate) async fn set_agent_config<R: Runtime>(
    app: AppHandle<R>,
    agent_id: String,
    config: AgentConfig,
) -> Result<()> {
    app.askit()
        .set_agent_config(agent_id, config)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn get_global_config<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
) -> Option<AgentConfig> {
    app.askit().get_global_config(&def_name)
}

#[tauri::command]
pub(crate) fn get_global_configs<R: Runtime>(
    app: AppHandle<R>,
) -> AgentConfigs {
    app.askit().get_global_configs()
}

#[tauri::command]
pub(crate) fn set_global_config<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
    config: AgentConfig,
) {
    app.askit().set_global_config(def_name, config);
}

#[tauri::command]
pub(crate) fn set_global_configs<R: Runtime>(
    app: AppHandle<R>,
    configs: AgentConfigs,
) {
    app.askit().set_global_configs(configs)
}

#[tauri::command]
pub(crate) async fn get_agent_default_config<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
) -> Option<AgentDefaultConfig> {
    app.askit().get_agent_default_config(&def_name)
}
