use std::path::PathBuf;

use anyhow::{bail, Context as _, Result};
use dirs;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use agent_stream_kit::{
    ASKit, ASKitEvent, ASKitObserver, Agent, AgentConfig, AgentDefinitions, AgentFlow,
    AgentFlowEdge, AgentFlowNode,
};
use askit_std_agents;

use super::observer::ASAppObserver;

pub struct ASApp {
    askit: ASKit,
}

impl ASApp {
    pub fn get_agent_definitions(&self) -> AgentDefinitions {
        self.askit.get_agent_definitions()
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
        Ok(())
    }

    pub fn save_agent_flow(&self, agent_flow: AgentFlow) -> Result<()> {
        let home_dir = dirs::home_dir().with_context(|| "Failed to get home directory")?;
        let mut flow_path = home_dir.join(".askit/flows");

        let path_components: Vec<&str> = agent_flow.name().split('/').collect();
        for &component in &path_components[..path_components.len() - 1] {
            flow_path = flow_path.join(component);
        }
        // Ensure the parent directory exists
        if !flow_path.exists() {
            std::fs::create_dir_all(flow_path.clone())?;
        }

        let flow_file = flow_path
            .join(path_components.last().context("no last component")?)
            .with_extension("json");

        let json = agent_flow.to_json()?;
        std::fs::write(flow_file, json).with_context(|| "Failed to write agent flow file")?;

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
    askit_rig_agents::register_agents(&askit);

    askit.new_agent_flow("main")?;

    // let mut sflows = HashMap::new();
    // // read_agent_flows_dir(app, &askit)?;
    // flow::init(&mut sflows)?;

    let asapp = ASApp { askit };
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
    app: State<ASApp>,
    old_name: String,
    new_name: String,
) -> Result<String, String> {
    let askit = &app.askit;
    askit
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
