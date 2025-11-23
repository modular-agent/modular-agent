use std::path::PathBuf;

use anyhow::{bail, Context as _, Result};
use dirs;
use tauri::{AppHandle, Manager, State};

use agent_stream_kit::{ASKit, AgentFlow};
use askit_std_agents;
use tauri_plugin_askit::ASKitExt;

use super::observer::ASAppObserver;

static ASKIT_FLOWS_PATH: &'static str = ".askit/flows";

pub struct ASApp {
    askit: ASKit,
}

impl ASApp {
    // AgentFlow

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
        let (nodes, edges) = self.askit.copy_sub_flow(flow.nodes(), flow.edges());
        flow.set_nodes(nodes);
        flow.set_edges(edges);

        Ok(flow)
    }
}

pub fn init(app: &AppHandle) -> Result<()> {
    let askit = app.askit();
    askit_std_agents::register_agents(&askit);
    askit_rhai_agents::register_agents(&askit);
    askit_cozodb_agents::register_agents(&askit);
    askit_lifelog_agents::register_agents(&askit);
    askit_llm_agents::register_agents(&askit);

    let asapp = ASApp {
        askit: askit.clone(),
    };
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
    let observer = ASAppObserver { app: app.clone() };
    askit.subscribe(Box::new(observer));
    Ok(())
}

pub fn quit(_app: &AppHandle) {}

fn agent_flows_dir() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().with_context(|| "Failed to get home directory")?;
    let flows_dir = home_dir.join(ASKIT_FLOWS_PATH);
    Ok(flows_dir)
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
pub fn save_agent_flow_cmd(asapp: State<ASApp>, agent_flow: AgentFlow) -> Result<(), String> {
    asapp.save_agent_flow(agent_flow).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_agent_flow_cmd(asapp: State<ASApp>, path: String) -> Result<AgentFlow, String> {
    asapp.import_agent_flow(path).map_err(|e| e.to_string())
}
