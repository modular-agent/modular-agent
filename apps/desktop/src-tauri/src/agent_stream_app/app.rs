use std::path::PathBuf;

use anyhow::{anyhow, bail, Context as _, Result};
use dirs;
use tauri::{AppHandle, Manager, State};

use agent_stream_kit::{ASKit, AgentStreamSpec};
use tauri_plugin_askit::ASKitExt;

use super::observer::ASAppObserver;

static ASKIT_STREAMS_PATH: &'static str = ".askit/streams";

pub struct ASApp {
    askit: ASKit,
}

impl ASApp {
    // AgentStream

    pub async fn remove_agent_stream(&self, stream_id: &str) -> Result<()> {
        let spec = self
            .askit
            .get_agent_stream(stream_id)
            .ok_or_else(|| anyhow!("Agent stream not found: {}", stream_id))?;
        self.askit.remove_agent_stream(stream_id).await?;
        let stream_path = self.agent_stream_path(&spec.name)?;
        if stream_path.exists() {
            std::fs::remove_file(stream_path)
                .with_context(|| "Failed to remove agent stream file")?;
        }
        Ok(())
    }

    pub fn rename_agent_stream(&self, stream_id: &str, new_name: &str) -> Result<String> {
        let old_spec = self
            .askit
            .get_agent_stream(stream_id)
            .ok_or_else(|| anyhow!("Agent stream not found: {}", stream_id))?;
        let old_name = old_spec.name.clone();

        let new_stream_path = self.agent_stream_path(new_name)?;
        if new_stream_path.exists() {
            bail!("Agent stream file already exists: {:?}", new_stream_path);
        }

        self.askit.rename_agent_stream(stream_id, new_name)?;
        let old_stream_path = self.agent_stream_path(&old_name)?;
        if old_stream_path.exists() {
            std::fs::rename(old_stream_path, new_stream_path)
                .with_context(|| "Failed to rename old agent stream file")?;
        }

        Ok(new_name.to_string())
    }

    fn agent_stream_path(&self, stream_name: &str) -> Result<PathBuf> {
        let mut stream_path = agent_streams_dir()?;

        let path_components: Vec<&str> = stream_name.split('/').collect();
        for &component in &path_components[..path_components.len()] {
            stream_path = stream_path.join(component);
        }

        stream_path = stream_path.with_extension("json");

        Ok(stream_path)
    }

    pub fn save_agent_stream(&self, agent_stream: AgentStreamSpec) -> Result<()> {
        let stream_path = self.agent_stream_path(&agent_stream.name)?;

        // Ensure the parent directory exists
        let parent_path = stream_path.parent().context("no parent path")?;
        if !parent_path.exists() {
            std::fs::create_dir_all(parent_path)?;
        }

        let json = agent_stream.to_json()?;
        std::fs::write(stream_path, json).with_context(|| "Failed to write agent stream file")?;

        Ok(())
    }

    fn read_agent_streams_dir(&self) -> Result<()> {
        let streams_dir = agent_streams_dir()?;
        if !streams_dir.exists() {
            std::fs::create_dir_all(&streams_dir)
                .with_context(|| "Failed to create streams directory")?;
            return Ok(());
        }

        self.read_agent_streams_dir_recursive(&streams_dir, "")?;
        Ok(())
    }

    fn read_agent_streams_dir_recursive(&self, dir: &PathBuf, name_prefix: &str) -> Result<()> {
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
                self.read_agent_streams_dir_recursive(&path, &new_prefix)?;
            } else if path.is_file() && path.extension().unwrap_or_default() == "json" {
                match self.read_agent_stream(path) {
                    Ok(stream) => {
                        if name_prefix.is_empty() {
                            self.askit.add_agent_stream(stream)?;
                        } else {
                            let mut stream = stream;
                            let full_name = format!("{}/{}", name_prefix, stream.name);
                            stream.name = full_name;
                            self.askit.add_agent_stream(stream)?;
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to read agent stream: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn import_agent_stream(&self, path: String) -> Result<String> {
        let path = PathBuf::from(path);
        let mut spec = self.read_agent_stream(path)?;
        spec.run_on_start = false;

        let name = self.askit.unique_stream_name(&spec.name);
        spec.name = name;

        let id = self
            .askit
            .add_agent_stream(spec)
            .context("Failed to add agent stream")?;

        Ok(id)
    }

    fn read_agent_stream(&self, path: PathBuf) -> Result<AgentStreamSpec> {
        if !path.is_file() || path.extension().unwrap_or_default() != "json" {
            bail!("Invalid file extension");
        }

        let content = std::fs::read_to_string(&path)?;
        // let mut stream = AgentStream::from_json(&content)?;

        let mut spec = AgentStreamSpec::from_json(&content)?;

        // Get the base name from the file name
        let base_name = path
            .file_stem()
            .context("Failed to get file stem")?
            .to_string_lossy()
            .trim()
            .to_string();
        if base_name.is_empty() {
            bail!("Agent stream name is empty");
        }
        spec.name = base_name.clone();

        // Rename IDs in the stream
        let agents_vec: Vec<_> = spec.agents.iter().cloned().collect();
        let channels_vec: Vec<_> = spec.channels.iter().cloned().collect();
        let (agents, edges) = self.askit.copy_sub_stream(&agents_vec, &channels_vec);
        spec.agents = agents.into();
        spec.channels = edges.into();

        Ok(spec)
    }
}

pub fn init(app: &AppHandle) -> Result<()> {
    let askit = app.askit();

    let asapp = ASApp {
        askit: askit.clone(),
    };
    asapp.read_agent_streams_dir().unwrap_or_else(|e| {
        log::error!("Failed to read agent streams: {}", e);
    });

    // if asapp.askit.get_agent_streams().get("main").is_none() {
    //     if let Err(e) = asapp.askit.new_agent_stream("main") {
    //         log::error!("Failed to create main agent stream: {}", e);
    //     };
    // }

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

fn agent_streams_dir() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().with_context(|| "Failed to get home directory")?;
    let streams_dir = home_dir.join(ASKIT_STREAMS_PATH);
    Ok(streams_dir)
}

#[tauri::command]
pub fn rename_agent_stream_cmd(
    asapp: State<'_, ASApp>,
    stream_id: String,
    new_name: String,
) -> Result<String, String> {
    asapp
        .rename_agent_stream(&stream_id, &new_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_agent_stream_cmd(
    asapp: State<'_, ASApp>,
    stream_id: String,
) -> Result<(), String> {
    asapp
        .remove_agent_stream(&stream_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_agent_stream_cmd(asapp: State<ASApp>, stream: AgentStreamSpec) -> Result<(), String> {
    asapp.save_agent_stream(stream).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_agent_stream_cmd(asapp: State<ASApp>, path: String) -> Result<String, String> {
    asapp.import_agent_stream(path).map_err(|e| e.to_string())
}
