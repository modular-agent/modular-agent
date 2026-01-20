use modular_agent_kit::{
    AgentConfigs, AgentConfigsMap, AgentDefinition, AgentDefinitions, AgentSpec, PresetSpec,
    AgentValue, ConnectionSpec,
};
use serde_json::Value;
use tauri::{AppHandle, Runtime};

use crate::MAKExt;
use crate::Result;

// agent definition

#[tauri::command]
pub(crate) fn get_agent_definition<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
) -> Option<AgentDefinition> {
    app.mak().get_agent_definition(&def_name)
}

#[tauri::command]
pub(crate) fn get_agent_definitions<R: Runtime>(app: AppHandle<R>) -> AgentDefinitions {
    app.mak().get_agent_definitions()
}

// agent spec

#[tauri::command]
pub(crate) async fn get_agent_spec<R: Runtime>(
    app: AppHandle<R>,
    agent_id: String,
) -> Option<AgentSpec> {
    app.mak().get_agent_spec(&agent_id).await
}

#[tauri::command]
pub(crate) async fn update_agent_spec<R: Runtime>(
    app: AppHandle<R>,
    agent_id: String,
    value: Value,
) -> Result<()> {
    app.mak()
        .update_agent_spec(&agent_id, &value)
        .await
        .map_err(Into::into)
}

// preset

#[tauri::command]
pub(crate) fn get_preset_info<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Option<modular_agent_kit::PresetInfo> {
    app.mak().get_preset_info(&id)
}

#[tauri::command]
pub(crate) fn get_preset_infos<R: Runtime>(
    app: AppHandle<R>,
) -> Vec<modular_agent_kit::PresetInfo> {
    app.mak().get_preset_infos()
}

#[tauri::command]
pub(crate) async fn get_preset_spec<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Option<PresetSpec> {
    app.mak().get_preset_spec(&id).await
}

#[tauri::command]
pub(crate) fn update_preset_spec<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    value: Value,
) -> Result<()> {
    app.mak()
        .update_preset_spec(&id, &value)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn new_preset<R: Runtime>(app: AppHandle<R>, name: String) -> Result<String> {
    app.mak().new_preset(&name).map_err(Into::into)
}

#[tauri::command]
pub(crate) fn rename_preset<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    name: String,
) -> Result<String> {
    app.mak()
        .rename_preset(&id, &name)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn unique_preset_name<R: Runtime>(app: tauri::AppHandle<R>, name: String) -> String {
    app.mak().unique_preset_name(&name)
}

#[tauri::command]
pub(crate) fn add_preset<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    spec: PresetSpec,
) -> Result<String> {
    app.mak().add_preset(name, spec).map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn remove_preset<R: Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
) -> Result<()> {
    app.mak()
        .remove_preset(&id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn add_agents_and_connections<R: Runtime>(
    app: AppHandle<R>,
    preset_id: &str,
    agents: Vec<AgentSpec>,
    connections: Vec<ConnectionSpec>,
) -> Result<(Vec<AgentSpec>, Vec<ConnectionSpec>)> {
    app.mak()
        .add_agents_and_connections(preset_id, &agents, &connections)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn start_preset<R: Runtime>(app: AppHandle<R>, id: String) -> Result<()> {
    app.mak()
        .start_preset(&id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn stop_preset<R: Runtime>(app: AppHandle<R>, id: String) -> Result<()> {
    app.mak().stop_preset(&id).await.map_err(Into::into)
}

// agent

#[tauri::command]
pub fn new_agent_spec<R: Runtime>(app: AppHandle<R>, def_name: String) -> Result<AgentSpec> {
    app.mak().new_agent_spec(&def_name).map_err(Into::into)
}

#[tauri::command]
pub(crate) fn add_agent<R: Runtime>(
    app: AppHandle<R>,
    preset_id: String,
    spec: AgentSpec,
) -> Result<String> {
    app.mak().add_agent(preset_id, spec).map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn remove_agent<R: Runtime>(
    app: AppHandle<R>,
    preset_id: String,
    agent_id: String,
) -> Result<()> {
    app.mak()
        .remove_agent(&preset_id, &agent_id)
        .await
        .map_err(Into::into)
}

// connection

#[tauri::command]
pub(crate) fn add_connection<R: Runtime>(
    app: AppHandle<R>,
    preset_id: String,
    connection: ConnectionSpec,
) -> Result<()> {
    app.mak()
        .add_connection(&preset_id, connection)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn remove_connection<R: Runtime>(
    app: AppHandle<R>,
    preset_id: String,
    connection: ConnectionSpec,
) -> Result<()> {
    app.mak()
        .remove_connection(&preset_id, &connection)
        .map_err(Into::into)
}

// agent

#[tauri::command]
pub(crate) async fn start_agent<R: Runtime>(app: AppHandle<R>, agent_id: String) -> Result<()> {
    app.mak().start_agent(&agent_id).await.map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn stop_agent<R: Runtime>(app: AppHandle<R>, agent_id: String) -> Result<()> {
    app.mak().stop_agent(&agent_id).await.map_err(Into::into)
}

// board commands

#[tauri::command]
pub(crate) async fn write_board<R: Runtime>(
    app: AppHandle<R>,
    board: String,
    message: String,
) -> Result<()> {
    app.mak()
        .write_board_value(board, AgentValue::string(message))
        .await
        .map_err(Into::into)
}

// config

#[tauri::command]
pub(crate) async fn set_agent_configs<R: Runtime>(
    app: AppHandle<R>,
    agent_id: String,
    configs: AgentConfigs,
) -> Result<()> {
    app.mak()
        .set_agent_configs(agent_id, configs)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn get_global_configs<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
) -> Option<AgentConfigs> {
    app.mak().get_global_configs(&def_name)
}

#[tauri::command]
pub(crate) fn get_global_configs_map<R: Runtime>(app: AppHandle<R>) -> AgentConfigsMap {
    app.mak().get_global_configs_map()
}

#[tauri::command]
pub(crate) fn set_global_configs<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
    configs: AgentConfigs,
) {
    app.mak().set_global_configs(def_name, configs);
}

#[tauri::command]
pub(crate) fn set_global_configs_map<R: Runtime>(app: AppHandle<R>, configs: AgentConfigsMap) {
    app.mak().set_global_configs_map(configs)
}
