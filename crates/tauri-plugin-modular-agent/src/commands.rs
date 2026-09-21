use std::collections::HashMap;

use modular_agent_core::{
    ConnectionSpec, ModuleConfigs, ModuleConfigsMap, ModuleDefinition, ModuleDefinitions,
    ModuleSpec, ModuleStatus, PatchSpec, Value,
};
use serde_json::Value as JsonValue;
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
    value: JsonValue,
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

// module management

// module definition

#[tauri::command]
pub fn get_module_definition<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
) -> Option<ModuleDefinition> {
    app.ma().get_module_definition(&def_name)
}

#[tauri::command]
pub fn get_module_definitions<R: Runtime>(app: AppHandle<R>) -> ModuleDefinitions {
    app.ma().get_module_definitions()
}

// module spec

#[tauri::command]
pub async fn get_module_spec<R: Runtime>(
    app: AppHandle<R>,
    module_id: String,
) -> Option<ModuleSpec> {
    app.ma().get_module_spec(&module_id).await
}

#[tauri::command]
pub async fn update_module_spec<R: Runtime>(
    app: AppHandle<R>,
    module_id: String,
    value: JsonValue,
) -> Result<()> {
    app.ma()
        .update_module_spec(&module_id, &value)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub fn new_module_spec<R: Runtime>(app: AppHandle<R>, def_name: String) -> Result<ModuleSpec> {
    app.ma().new_module_spec(&def_name).map_err(Into::into)
}

#[tauri::command]
pub async fn add_module<R: Runtime>(
    app: AppHandle<R>,
    patch_id: String,
    spec: ModuleSpec,
) -> Result<String> {
    app.ma()
        .add_module(patch_id, spec)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn remove_module<R: Runtime>(
    app: AppHandle<R>,
    patch_id: String,
    module_id: String,
) -> Result<()> {
    app.ma()
        .remove_module(&patch_id, &module_id)
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
pub async fn add_modules_and_connections<R: Runtime>(
    app: AppHandle<R>,
    patch_id: &str,
    modules: Vec<ModuleSpec>,
    connections: Vec<ConnectionSpec>,
) -> Result<(Vec<ModuleSpec>, Vec<ConnectionSpec>)> {
    app.ma()
        .add_modules_and_connections(patch_id, &modules, &connections)
        .await
        .map_err(Into::into)
}

// module

#[tauri::command]
pub async fn start_module<R: Runtime>(app: AppHandle<R>, module_id: String) -> Result<()> {
    app.ma().start_module(&module_id).await.map_err(Into::into)
}

#[tauri::command]
pub async fn stop_module<R: Runtime>(app: AppHandle<R>, module_id: String) -> Result<()> {
    app.ma().stop_module(&module_id).await.map_err(Into::into)
}

#[tauri::command]
pub async fn get_module_statuses<R: Runtime>(
    app: AppHandle<R>,
    patch_id: String,
) -> Result<HashMap<String, ModuleStatus>> {
    app.ma()
        .get_module_statuses(&patch_id)
        .await
        .map_err(Into::into)
}

// config

#[tauri::command]
pub async fn set_module_configs<R: Runtime>(
    app: AppHandle<R>,
    module_id: String,
    configs: ModuleConfigs,
) -> Result<()> {
    app.ma()
        .set_module_configs(module_id, configs)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_global_configs<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
) -> Option<ModuleConfigs> {
    app.ma().get_global_configs(&def_name)
}

#[tauri::command]
pub fn get_global_configs_map<R: Runtime>(app: AppHandle<R>) -> ModuleConfigsMap {
    app.ma().get_global_configs_map()
}

#[tauri::command]
pub fn set_global_configs<R: Runtime>(app: AppHandle<R>, def_name: String, configs: ModuleConfigs) {
    app.ma().set_global_configs(def_name, configs);
}

#[tauri::command]
pub fn set_global_configs_map<R: Runtime>(app: AppHandle<R>, configs: ModuleConfigsMap) {
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
        .write_external_input(name, Value::string(message))
        .await
        .map_err(Into::into)
}
