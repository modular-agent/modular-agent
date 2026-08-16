use modular_agent_core::{
    AgentConfigs, AgentConfigsMap, AgentDefinition, AgentDefinitions, AgentSpec, AgentValue,
    ConnectionSpec, PatchSpec,
};
use serde_json::Value;
use tauri::{AppHandle, Runtime};

use crate::ModularAgentExt;
use crate::Result;

// Patch management

#[tauri::command]
pub fn new_patch<R: Runtime>(app: AppHandle<R>) -> Result<String> {
    app.ma().new_patch().map_err(Into::into)
}

#[tauri::command]
pub fn add_patch<R: Runtime>(app: AppHandle<R>, spec: PatchSpec) -> Result<String> {
    app.ma().add_patch(spec).map_err(Into::into)
}

#[tauri::command]
pub fn add_patch_with_name<R: Runtime>(
    app: AppHandle<R>,
    spec: PatchSpec,
    name: String,
) -> Result<String> {
    app.ma().add_patch_with_name(spec, name).map_err(Into::into)
}

#[tauri::command]
pub async fn remove_patch<R: Runtime>(app: tauri::AppHandle<R>, id: String) -> Result<()> {
    app.ma().remove_patch(&id).await.map_err(Into::into)
}

#[tauri::command]
pub async fn start_patch<R: Runtime>(app: AppHandle<R>, id: String) -> Result<()> {
    app.ma().start_patch(&id).await.map_err(Into::into)
}

#[tauri::command]
pub async fn stop_patch<R: Runtime>(app: AppHandle<R>, id: String) -> Result<()> {
    app.ma().stop_patch(&id).await.map_err(Into::into)
}

#[tauri::command]
pub async fn open_patch_from_file<R: Runtime>(
    app: AppHandle<R>,
    path: String,
    name: Option<String>,
) -> Result<String> {
    app.ma()
        .open_patch_from_file(&path, name)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn save_patch<R: Runtime>(app: AppHandle<R>, id: String, path: String) -> Result<()> {
    app.ma().save_patch(&id, &path).await.map_err(Into::into)
}

// patch spec

#[tauri::command]
pub async fn get_patch_spec<R: Runtime>(app: AppHandle<R>, id: String) -> Option<PatchSpec> {
    app.ma().get_patch_spec(&id).await
}

#[tauri::command]
pub async fn update_patch_spec<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    value: Value,
) -> Result<()> {
    app.ma()
        .update_patch_spec(&id, &value)
        .await
        .map_err(Into::into)
}

// patch info

#[tauri::command]
pub async fn get_patch_info<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Option<modular_agent_core::PatchInfo> {
    app.ma().get_patch_info(&id).await
}

#[tauri::command]
pub async fn get_patch_infos<R: Runtime>(app: AppHandle<R>) -> Vec<modular_agent_core::PatchInfo> {
    app.ma().get_patch_infos().await
}

// agent management

// agent definition

#[tauri::command]
pub fn get_agent_definition<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
) -> Option<AgentDefinition> {
    app.ma().get_agent_definition(&def_name)
}

#[tauri::command]
pub fn get_agent_definitions<R: Runtime>(app: AppHandle<R>) -> AgentDefinitions {
    app.ma().get_agent_definitions()
}

// agent spec

#[tauri::command]
pub async fn get_agent_spec<R: Runtime>(app: AppHandle<R>, agent_id: String) -> Option<AgentSpec> {
    app.ma().get_agent_spec(&agent_id).await
}

#[tauri::command]
pub async fn update_agent_spec<R: Runtime>(
    app: AppHandle<R>,
    agent_id: String,
    value: Value,
) -> Result<()> {
    app.ma()
        .update_agent_spec(&agent_id, &value)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub fn new_agent_spec<R: Runtime>(app: AppHandle<R>, def_name: String) -> Result<AgentSpec> {
    app.ma().new_agent_spec(&def_name).map_err(Into::into)
}

#[tauri::command]
pub async fn add_agent<R: Runtime>(
    app: AppHandle<R>,
    patch_id: String,
    spec: AgentSpec,
) -> Result<String> {
    app.ma().add_agent(patch_id, spec).await.map_err(Into::into)
}

#[tauri::command]
pub async fn remove_agent<R: Runtime>(
    app: AppHandle<R>,
    patch_id: String,
    agent_id: String,
) -> Result<()> {
    app.ma()
        .remove_agent(&patch_id, &agent_id)
        .await
        .map_err(Into::into)
}

// connection

#[tauri::command]
pub async fn add_connection<R: Runtime>(
    app: AppHandle<R>,
    patch_id: String,
    connection: ConnectionSpec,
) -> Result<()> {
    app.ma()
        .add_connection(&patch_id, connection)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn remove_connection<R: Runtime>(
    app: AppHandle<R>,
    patch_id: String,
    connection: ConnectionSpec,
) -> Result<()> {
    app.ma()
        .remove_connection(&patch_id, &connection)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn add_agents_and_connections<R: Runtime>(
    app: AppHandle<R>,
    patch_id: &str,
    agents: Vec<AgentSpec>,
    connections: Vec<ConnectionSpec>,
) -> Result<(Vec<AgentSpec>, Vec<ConnectionSpec>)> {
    app.ma()
        .add_agents_and_connections(patch_id, &agents, &connections)
        .await
        .map_err(Into::into)
}

// agent

#[tauri::command]
pub async fn start_agent<R: Runtime>(app: AppHandle<R>, agent_id: String) -> Result<()> {
    app.ma().start_agent(&agent_id).await.map_err(Into::into)
}

#[tauri::command]
pub async fn stop_agent<R: Runtime>(app: AppHandle<R>, agent_id: String) -> Result<()> {
    app.ma().stop_agent(&agent_id).await.map_err(Into::into)
}

// config

#[tauri::command]
pub async fn set_agent_configs<R: Runtime>(
    app: AppHandle<R>,
    agent_id: String,
    configs: AgentConfigs,
) -> Result<()> {
    app.ma()
        .set_agent_configs(agent_id, configs)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_global_configs<R: Runtime>(app: AppHandle<R>, def_name: String) -> Option<AgentConfigs> {
    app.ma().get_global_configs(&def_name)
}

#[tauri::command]
pub fn get_global_configs_map<R: Runtime>(app: AppHandle<R>) -> AgentConfigsMap {
    app.ma().get_global_configs_map()
}

#[tauri::command]
pub fn set_global_configs<R: Runtime>(app: AppHandle<R>, def_name: String, configs: AgentConfigs) {
    app.ma().set_global_configs(def_name, configs);
}

#[tauri::command]
pub fn set_global_configs_map<R: Runtime>(app: AppHandle<R>, configs: AgentConfigsMap) {
    app.ma().set_global_configs_map(configs)
}

// external input commands

#[tauri::command]
pub async fn write_external_input<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    message: String,
) -> Result<()> {
    app.ma()
        .write_external_input(name, AgentValue::string(message))
        .await
        .map_err(Into::into)
}
