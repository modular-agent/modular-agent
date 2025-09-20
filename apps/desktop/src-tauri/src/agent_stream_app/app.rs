use std::path::PathBuf;

use anyhow::{bail, Context as _, Result};
use dirs;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use agent_stream_kit::{
    ASKit, ASKitEvent, ASKitObserver, AgentConfig, AgentConfigs, AgentDefinitions, AgentFlow,
    AgentFlowEdge, AgentFlowNode,
};
use askit_std_agents;

use super::observer::ASAppObserver;

static ASKIT_FLOWS_PATH: &'static str = ".askit/flows";

pub struct ASApp {
    askit: ASKit,
}

impl ASApp {
    pub fn get_agent_definitions(&self) -> AgentDefinitions {
        self.askit.get_agent_definitions()
    }

    // Global Configs
    pub fn set_global_configs(&self, configs: AgentConfigs) {
        self.askit.set_global_configs(configs);
    }

    pub fn get_global_configs(&self) -> AgentConfigs {
        self.askit.get_global_configs()
    }

    pub fn set_global_config(&self, agent_name: String, config: Value) {
        if let Ok(config) = serde_json::from_value::<AgentConfig>(config) {
            self.askit.set_global_config(agent_name, config);
        } else {
            log::error!("Failed to parse agent config for {}", agent_name);
        }
    }

    // Agent

    pub async fn set_agent_config(&self, agent_id: String, config: AgentConfig) -> Result<()> {
        self.askit.set_agent_config(agent_id, config).await?;
        Ok(())
    }

    pub async fn start_agent(&self, agent_id: &str) -> Result<()> {
        self.askit.start_agent(agent_id).await.unwrap_or_else(|e| {
            log::error!("Failed to start agent: {}", e);
        });
        Ok(())
    }

    pub async fn stop_agent(&self, agent_id: &str) -> Result<()> {
        self.askit.stop_agent(agent_id).await.unwrap_or_else(|e| {
            log::error!("Failed to stop agent: {}", e);
        });
        Ok(())
    }

    // AgentFlow

    pub fn new_agent_flow_node(&self, def_name: &str) -> Result<AgentFlowNode> {
        let def = self
            .askit
            .get_agent_definition(def_name)
            .with_context(|| format!("Agent definition '{}' not found", def_name))?;
        let node = AgentFlowNode::new(&def)?;
        Ok(node)
    }

    pub fn add_agent_flow_node(&self, flow_name: &str, node: AgentFlowNode) -> Result<()> {
        self.askit.add_agent_flow_node(flow_name, &node)?;
        Ok(())
    }

    pub async fn remove_agent_flow_node(&self, flow_name: &str, node_id: &str) -> Result<()> {
        self.askit
            .remove_agent_flow_node(flow_name, node_id)
            .await?;
        Ok(())
    }

    pub fn add_agent_flow_edge(&self, flow_name: &str, edge: AgentFlowEdge) -> Result<()> {
        self.askit.add_agent_flow_edge(flow_name, &edge)?;
        Ok(())
    }

    pub fn remove_agent_flow_edge(&self, flow_name: &str, edge_id: &str) -> Result<()> {
        self.askit.remove_agent_flow_edge(flow_name, edge_id)?;
        Ok(())
    }

    pub fn insert_agent_flow(&self, flow: AgentFlow) -> Result<()> {
        self.askit.insert_agent_flow(flow)?;
        Ok(())
    }

    pub async fn remove_agent_flow(&self, name: &str) -> Result<()> {
        self.askit.remove_agent_flow(name).await?;

        let flow_path = self.agent_flow_path(name)?;
        if flow_path.exists() {
            std::fs::remove_file(flow_path).with_context(|| "Failed to remove agent flow file")?;
        }

        Ok(())
    }

    pub fn rename_agent_flow(&self, old_name: &str, new_name: &str) -> Result<String> {
        let new_flow_path = self.agent_flow_path(new_name)?;
        if new_flow_path.exists() {
            bail!("Agent flow file already exists: {:?}", new_flow_path);
        }

        self.askit.rename_agent_flow(old_name, new_name)?;

        let old_flow_path = self.agent_flow_path(old_name)?;
        if old_flow_path.exists() {
            std::fs::rename(old_flow_path, new_flow_path)
                .with_context(|| "Failed to rename old agent flow file")?;
        }

        Ok(new_name.to_string())
    }

    fn agent_flow_path(&self, flow_name: &str) -> Result<PathBuf> {
        let mut flow_path = agent_flows_dir()?;

        let path_components: Vec<&str> = flow_name.split('/').collect();
        for &component in &path_components[..path_components.len()] {
            flow_path = flow_path.join(component);
        }

        flow_path = flow_path.with_extension("json");

        Ok(flow_path)
    }

    pub fn save_agent_flow(&self, agent_flow: AgentFlow) -> Result<()> {
        let flow_path = self.agent_flow_path(agent_flow.name())?;

        // Ensure the parent directory exists
        let parent_path = flow_path.parent().context("no parent path")?;
        if !parent_path.exists() {
            std::fs::create_dir_all(parent_path)?;
        }

        let json = agent_flow.to_json()?;
        std::fs::write(flow_path, json).with_context(|| "Failed to write agent flow file")?;

        Ok(())
    }

    fn read_agent_flows_dir(&self) -> Result<()> {
        let flows_dir = agent_flows_dir()?;
        if !flows_dir.exists() {
            std::fs::create_dir_all(&flows_dir)
                .with_context(|| "Failed to create flows directory")?;
            return Ok(());
        }

        self.read_agent_flows_dir_recursive(&flows_dir, "")?;

        Ok(())
    }

    fn read_agent_flows_dir_recursive(&self, dir: &PathBuf, name_prefix: &str) -> Result<()> {
        if !dir.exists() || !dir.is_dir() {
            return Ok(());
        }

        let entries = std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {:?}", dir))?;

        for entry in entries {
            let entry = entry.with_context(|| "Failed to read directory entry")?;
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path
                    .file_name()
                    .context("Failed to get directory name")?
                    .to_string_lossy();
                let new_prefix = if name_prefix.is_empty() {
                    dir_name.to_string()
                } else {
                    format!("{}/{}", name_prefix, dir_name)
                };
                self.read_agent_flows_dir_recursive(&path, &new_prefix)?;
            } else if path.is_file() && path.extension().unwrap_or_default() == "json" {
                match self.read_agent_flow(path) {
                    Ok(flow) => {
                        if name_prefix.is_empty() {
                            self.askit.add_agent_flow(&flow)?;
                        } else {
                            let mut flow = flow;
                            let full_name = format!("{}/{}", name_prefix, flow.name());
                            flow.set_name(full_name);
                            self.askit.add_agent_flow(&flow)?;
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to read agent flow: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn import_agent_flow(&self, path: String) -> Result<AgentFlow> {
        let path = PathBuf::from(path);
        let mut flow = self.read_agent_flow(path)?;

        let name = self.askit.unique_flow_name(flow.name());
        flow.set_name(name);
        flow.disable_all_nodes();

        self.askit
            .add_agent_flow(&flow)
            .context("Failed to add agent flow")?;

        Ok(flow)
    }

    fn read_agent_flow(&self, path: PathBuf) -> Result<AgentFlow> {
        if !path.is_file() || path.extension().unwrap_or_default() != "json" {
            bail!("Invalid file extension");
        }

        let content = std::fs::read_to_string(&path)?;
        let mut flow = AgentFlow::from_json(&content)?;

        // Get the base name from the file name
        let base_name = path
            .file_stem()
            .context("Failed to get file stem")?
            .to_string_lossy()
            .trim()
            .to_string();
        if base_name.is_empty() {
            bail!("Agent flow name is empty");
        }
        flow.set_name(base_name.clone());

        // Rename IDs in the flow
        let (nodes, edges) = self.copy_sub_flow(flow.nodes(), flow.edges());
        flow.set_nodes(nodes);
        flow.set_edges(edges);

        Ok(flow)
    }

    fn copy_sub_flow(
        &self,
        nodes: &Vec<AgentFlowNode>,
        edges: &Vec<AgentFlowEdge>,
    ) -> (Vec<AgentFlowNode>, Vec<AgentFlowEdge>) {
        self.askit.copy_sub_flow(nodes, edges)
    }
}

impl ASKitObserver for ASApp {
    fn notify(&self, event: ASKitEvent) {
        match event {
            ASKitEvent::AgentIn(agent_id, channel) => {
                log::info!("Agent input: {} - {:?}", agent_id, channel);
            }
            ASKitEvent::AgentDisplay(agent_id, key, data) => {
                log::info!("Agent display: {} - {}: {:?}", agent_id, key, data);
            }
            ASKitEvent::AgentError(agent_id, message) => {
                log::error!("Agent error: {} - {}", agent_id, message);
            }
        }
    }
}

pub fn init(app: &AppHandle) -> Result<()> {
    let askit = ASKit::init()?;
    askit_std_agents::register_agents(&askit);
    askit_llm_agents::register_agents(&askit);
    askit_rig_agents::register_agents(&askit);

    let asapp = ASApp { askit };
    asapp.read_agent_flows_dir().unwrap_or_else(|e| {
        log::error!("Failed to read agent flows: {}", e);
    });

    if asapp.askit.get_agent_flows().get("main").is_none() {
        if let Err(e) = asapp.askit.new_agent_flow("main") {
            log::error!("Failed to create main agent flow: {}", e);
        };
    }

    app.manage(asapp);

    Ok(())
}

pub async fn ready(app: &AppHandle) -> Result<()> {
    let asapp = app.state::<ASApp>();
    let askit = &asapp.askit;
    askit.ready().await?;
    let observer = ASAppObserver { app: app.clone() };
    askit.subscribe(Box::new(observer));
    Ok(())
}

pub fn quit(app: &AppHandle) {
    let asapp = app.state::<ASApp>();
    let askit = &asapp.askit;
    askit.quit();
}

fn agent_flows_dir() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().with_context(|| "Failed to get home directory")?;
    let flows_dir = home_dir.join(ASKIT_FLOWS_PATH);
    Ok(flows_dir)
}

// Tauri Commands

#[tauri::command]
pub fn get_agent_defs_cmd(asapp: State<'_, ASApp>) -> Result<Value, String> {
    let defs = asapp.get_agent_definitions();
    serde_json::to_value(defs).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_agent_config_cmd(
    asapp: State<'_, ASApp>,
    agent_id: String,
    config: AgentConfig,
) -> Result<(), String> {
    asapp
        .set_agent_config(agent_id, config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_agent_cmd(asapp: State<'_, ASApp>, agent_id: String) -> Result<(), String> {
    asapp
        .start_agent(&agent_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_agent_cmd(asapp: State<'_, ASApp>, agent_id: String) -> Result<(), String> {
    asapp.stop_agent(&agent_id).await.map_err(|e| e.to_string())
}

// flow commands

#[tauri::command]
pub fn get_agent_flows_cmd(asapp: State<'_, ASApp>) -> Result<Value, String> {
    let askit = &asapp.askit;
    let flows = askit.get_agent_flows();
    let value = serde_json::to_value(&flows).map_err(|e| e.to_string())?;
    Ok(value)
}

#[tauri::command]
pub fn new_agent_flow_cmd(asapp: State<ASApp>, name: String) -> Result<AgentFlow, String> {
    let askit = &asapp.askit;
    let flow = askit.new_agent_flow(&name).map_err(|e| e.to_string())?;
    Ok(flow)
}

#[tauri::command]
pub fn rename_agent_flow_cmd(
    asapp: State<'_, ASApp>,
    old_name: String,
    new_name: String,
) -> Result<String, String> {
    asapp
        .rename_agent_flow(&old_name, &new_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_agent_flow_cmd(asapp: State<'_, ASApp>, name: String) -> Result<(), String> {
    asapp
        .remove_agent_flow(&name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn insert_agent_flow_cmd(asapp: State<'_, ASApp>, agent_flow: AgentFlow) -> Result<(), String> {
    asapp
        .insert_agent_flow(agent_flow)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_agent_flow_cmd(asapp: State<ASApp>, agent_flow: AgentFlow) -> Result<(), String> {
    asapp.save_agent_flow(agent_flow).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_agent_flow_cmd(asapp: State<ASApp>, path: String) -> Result<AgentFlow, String> {
    asapp.import_agent_flow(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn new_agent_flow_node_cmd(
    asapp: State<'_, ASApp>,
    def_name: String,
) -> Result<AgentFlowNode, String> {
    asapp
        .new_agent_flow_node(&def_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_agent_flow_node_cmd(
    asapp: State<'_, ASApp>,
    flow_name: String,
    node: AgentFlowNode,
) -> Result<(), String> {
    asapp
        .add_agent_flow_node(&flow_name, node)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_agent_flow_node_cmd(
    asapp: State<'_, ASApp>,
    flow_name: String,
    node_id: String,
) -> Result<(), String> {
    asapp
        .remove_agent_flow_node(&flow_name, &node_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_agent_flow_edge_cmd(
    asapp: State<'_, ASApp>,
    flow_name: String,
    edge: AgentFlowEdge,
) -> Result<(), String> {
    asapp
        .add_agent_flow_edge(&flow_name, edge)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_agent_flow_edge_cmd(
    asapp: State<'_, ASApp>,
    flow_name: String,
    edge_id: String,
) -> Result<(), String> {
    asapp
        .remove_agent_flow_edge(&flow_name, &edge_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn copy_sub_flow_cmd(
    asapp: State<ASApp>,
    nodes: Vec<AgentFlowNode>,
    edges: Vec<AgentFlowEdge>,
) -> (Vec<AgentFlowNode>, Vec<AgentFlowEdge>) {
    asapp.copy_sub_flow(&nodes, &edges)
}
