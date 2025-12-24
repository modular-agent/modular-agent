use agent_stream_kit::{
    AgentConfigSpecs, AgentConfigs, AgentConfigsMap, AgentDefinition, AgentDefinitions, AgentSpec,
    AgentStream, AgentStreams, AgentValue, ChannelSpec,
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

// agent spec

#[tauri::command]
pub(crate) fn get_agent_spec<R: Runtime>(app: AppHandle<R>, agent_id: String) -> Option<AgentSpec> {
    app.askit().get_agent_spec(&agent_id)
}

// stream

#[tauri::command]
pub(crate) fn get_agent_streams<R: Runtime>(app: AppHandle<R>) -> AgentStreams {
    app.askit().get_agent_streams()
}

#[tauri::command]
pub(crate) fn get_running_agent_streams<R: Runtime>(app: AppHandle<R>) -> Vec<String> {
    app.askit().get_running_agent_streams()
}

#[tauri::command]
pub(crate) fn new_agent_stream<R: Runtime>(
    app: AppHandle<R>,
    stream_name: String,
) -> Result<AgentStream> {
    app.askit()
        .new_agent_stream(&stream_name)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn rename_agent_stream<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    new_name: String,
) -> Result<String> {
    app.askit()
        .rename_agent_stream(&id, &new_name)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn unique_stream_name<R: Runtime>(app: tauri::AppHandle<R>, name: String) -> String {
    app.askit().unique_stream_name(&name)
}

#[tauri::command]
pub(crate) fn add_agent_stream<R: Runtime>(
    app: AppHandle<R>,
    agent_stream: AgentStream,
) -> Result<()> {
    app.askit()
        .add_agent_stream(&agent_stream)
        .map_err(Into::into)
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
pub(crate) fn insert_agent_stream<R: Runtime>(
    app: AppHandle<R>,
    agent_stream: AgentStream,
) -> Result<()> {
    app.askit()
        .insert_agent_stream(agent_stream)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn copy_sub_stream<R: Runtime>(
    app: AppHandle<R>,
    agents: Vec<AgentSpec>,
    channels: Vec<ChannelSpec>,
) -> (Vec<AgentSpec>, Vec<ChannelSpec>) {
    app.askit().copy_sub_stream(&agents, &channels)
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
) -> Result<()> {
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
    channel_id: String,
) -> Result<()> {
    app.askit()
        .remove_channel(&stream_id, &channel_id)
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
pub(crate) async fn get_agent_config_specs<R: Runtime>(
    app: AppHandle<R>,
    def_name: String,
) -> Option<AgentConfigSpecs> {
    app.askit().get_agent_config_specs(&def_name)
}
