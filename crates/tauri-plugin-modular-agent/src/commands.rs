use agent_stream_kit::{
    AgentConfigs, AgentConfigsMap, AgentDefinition, AgentDefinitions, AgentSpec, AgentStreamSpec,
    AgentValue, ChannelSpec,
};
use serde_json::Value;
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

// agent spec

#[tauri::command]
pub(crate) async fn get_agent_spec<R: Runtime>(
    app: AppHandle<R>,
    agent_id: String,
) -> Option<AgentSpec> {
    app.askit().get_agent_spec(&agent_id).await
}

#[tauri::command]
pub(crate) async fn update_agent_spec<R: Runtime>(
    app: AppHandle<R>,
    agent_id: String,
    value: Value,
) -> Result<()> {
    app.askit()
        .update_agent_spec(&agent_id, &value)
        .await
        .map_err(Into::into)
}

// stream

#[tauri::command]
pub(crate) fn get_agent_stream_info<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Option<agent_stream_kit::AgentStreamInfo> {
    app.askit().get_agent_stream_info(&id)
}

#[tauri::command]
pub(crate) fn get_agent_stream_infos<R: Runtime>(
    app: AppHandle<R>,
) -> Vec<agent_stream_kit::AgentStreamInfo> {
    app.askit().get_agent_stream_infos()
}

#[tauri::command]
pub(crate) async fn get_agent_stream_spec<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Option<AgentStreamSpec> {
    app.askit().get_agent_stream_spec(&id).await
}

#[tauri::command]
pub(crate) fn update_agent_stream_spec<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    value: Value,
) -> Result<()> {
    app.askit()
        .update_agent_stream_spec(&id, &value)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn new_agent_stream<R: Runtime>(app: AppHandle<R>, name: String) -> Result<String> {
    app.askit().new_agent_stream(&name).map_err(Into::into)
}

#[tauri::command]
pub(crate) fn rename_agent_stream<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    name: String,
) -> Result<String> {
    app.askit()
        .rename_agent_stream(&id, &name)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn unique_stream_name<R: Runtime>(app: tauri::AppHandle<R>, name: String) -> String {
    app.askit().unique_stream_name(&name)
}

#[tauri::command]
pub(crate) fn add_agent_stream<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    spec: AgentStreamSpec,
) -> Result<String> {
    app.askit().add_agent_stream(name, spec).map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn remove_agent_stream<R: Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
) -> Result<()> {
    app.askit()
        .remove_agent_stream(&id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn add_agents_and_channels<R: Runtime>(
    app: AppHandle<R>,
    stream_id: &str,
    agents: Vec<AgentSpec>,
    channels: Vec<ChannelSpec>,
) -> Result<(Vec<AgentSpec>, Vec<ChannelSpec>)> {
    app.askit()
        .add_agents_and_channels(stream_id, &agents, &channels)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn start_agent_stream<R: Runtime>(app: AppHandle<R>, id: String) -> Result<()> {
    app.askit()
        .start_agent_stream(&id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn stop_agent_stream<R: Runtime>(app: AppHandle<R>, id: String) -> Result<()> {
    app.askit().stop_agent_stream(&id).await.map_err(Into::into)
}

// agent

#[tauri::command]
pub fn new_agent_spec<R: Runtime>(app: AppHandle<R>, def_name: String) -> Result<AgentSpec> {
    app.askit().new_agent_spec(&def_name).map_err(Into::into)
}

#[tauri::command]
pub(crate) fn add_agent<R: Runtime>(
    app: AppHandle<R>,
    stream_id: String,
    spec: AgentSpec,
) -> Result<String> {
    app.askit().add_agent(stream_id, spec).map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn remove_agent<R: Runtime>(
    app: AppHandle<R>,
    stream_id: String,
    agent_id: String,
) -> Result<()> {
    app.askit()
        .remove_agent(&stream_id, &agent_id)
        .await
        .map_err(Into::into)
}

// channel

#[tauri::command]
pub(crate) fn add_channel<R: Runtime>(
    app: AppHandle<R>,
    stream_id: String,
    channel: ChannelSpec,
) -> Result<()> {
    app.askit()
        .add_channel(&stream_id, channel)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn remove_channel<R: Runtime>(
    app: AppHandle<R>,
    stream_id: String,
    channel: ChannelSpec,
) -> Result<()> {
    app.askit()
        .remove_channel(&stream_id, &channel)
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
pub(crate) async fn write_board<R: Runtime>(
    app: AppHandle<R>,
    board: String,
    message: String,
) -> Result<()> {
    app.askit()
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
