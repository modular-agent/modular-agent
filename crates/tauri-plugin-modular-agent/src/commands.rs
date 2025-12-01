use agent_stream_kit::{
    AgentConfigs, AgentConfigsMap, AgentValue, AgentDefaultConfigs, AgentDefinition, AgentDefinitions,
    AgentFlow, AgentFlowEdge, AgentFlowNode, AgentFlows,
};
use tauri::{AppHandle, Runtime};

use crate::ASKitExt;
use crate::Result;

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
    id: String,
    new_name: String,
) -> Result<String> {
    app.askit()
        .rename_agent_flow(&id, &new_name)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn unique_flow_name<R: Runtime>(app: tauri::AppHandle<R>, name: String) -> String {
    app.askit().unique_flow_name(&name)
}

#[tauri::command]
pub(crate) fn add_agent_flow<R: Runtime>(app: AppHandle<R>, agent_flow: AgentFlow) -> Result<()> {
    app.askit().add_agent_flow(&agent_flow).map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn remove_agent_flow<R: Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
) -> Result<()> {
    app.askit()
        .remove_agent_flow(&id)
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
    id: String,
) -> Result<()> {
    app.askit()
        .start_agent_flow(&id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn stop_agent_flow<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<()> {
    app.askit()
        .stop_agent_flow(&id)
        .await
        .map_err(Into::into)
}

// node

#[tauri::command]
pub fn new_agent_flow_node<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
) -> Result<AgentFlowNode> {
    app.askit()
        .new_agent_flow_node(&def_name)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn add_agent_flow_node<R: Runtime>(
    app: AppHandle<R>,
    flow_id: String,
    node: AgentFlowNode,
) -> Result<()> {
    app.askit()
        .add_agent_flow_node(&flow_id, &node)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn remove_agent_flow_node<R: Runtime>(
    app: AppHandle<R>,
    flow_id: String,
    node_id: String,
) -> Result<()> {
    app.askit()
        .remove_agent_flow_node(&flow_id, &node_id)
        .await
        .map_err(Into::into)
}

// edge

#[tauri::command]
pub(crate) fn add_agent_flow_edge<R: Runtime>(
    app: AppHandle<R>,
    flow_id: String,
    edge: AgentFlowEdge,
) -> Result<()> {
    app.askit()
        .add_agent_flow_edge(&flow_id, &edge)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn remove_agent_flow_edge<R: Runtime>(
    app: AppHandle<R>,
    flow_id: String,
    edge_id: String,
) -> Result<()> {
    app.askit()
        .remove_agent_flow_edge(&flow_id, &edge_id)
        .map_err(Into::into)
}

// agent

#[tauri::command]
pub(crate) async fn start_agent<R: Runtime>(app: AppHandle<R>, agent_id: String) -> Result<()> {
    app.askit().start_agent(&agent_id).await.map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn stop_agent<R: Runtime>(app: AppHandle<R>, agent_id: String) -> Result<()> {
    app.askit().stop_agent(&agent_id).await.map_err(Into::into)
}

// board commands

#[tauri::command]
pub(crate) fn write_board<R: Runtime>(
    app: AppHandle<R>,
    board: String,
    message: String,
) -> Result<()> {
    app.askit()
        .write_board_value(board, AgentValue::string(message))
        .map_err(Into::into)
}

// config

#[tauri::command]
pub(crate) async fn set_agent_configs<R: Runtime>(
    app: AppHandle<R>,
    agent_id: String,
    configs: AgentConfigs,
) -> Result<()> {
    app.askit()
        .set_agent_configs(agent_id, configs)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn get_global_configs<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
) -> Option<AgentConfigs> {
    app.askit().get_global_configs(&def_name)
}

#[tauri::command]
pub(crate) fn get_global_configs_map<R: Runtime>(app: AppHandle<R>) -> AgentConfigsMap {
    app.askit().get_global_configs_map()
}

#[tauri::command]
pub(crate) fn set_global_configs<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
    configs: AgentConfigs,
) {
    app.askit().set_global_configs(def_name, configs);
}

#[tauri::command]
pub(crate) fn set_global_configs_map<R: Runtime>(app: AppHandle<R>, configs: AgentConfigsMap) {
    app.askit().set_global_configs_map(configs)
}

#[tauri::command]
pub(crate) async fn get_agent_default_configs<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
) -> Option<AgentDefaultConfigs> {
    app.askit().get_agent_default_configs(&def_name)
}
