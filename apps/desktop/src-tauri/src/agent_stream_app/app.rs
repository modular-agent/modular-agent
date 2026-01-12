use std::{path::PathBuf, sync::Mutex};

use anyhow::{anyhow, bail, Context as _, Result};
use dirs;
use tauri::{AppHandle, Manager, State};

use agent_stream_kit::{ASKit, AgentStreamSpec};
use tauri_plugin_askit::ASKitExt;

use crate::agent_stream_app::settings::CoreSettings;

use super::observer::ASAppObserver;

static ASKIT_STREAMS_PATH: &'static str = ".askit/streams";

pub struct ASApp {
    askit: ASKit,
}

impl ASApp {
    pub fn new(askit: &ASKit) -> Self {
        Self {
            askit: askit.clone(),
        }
    }

    // AgentStream

    /// Create a new agent stream with the given name.
    /// Update the stream infos list.
    pub fn new_agent_stream(&self, name: &str) -> Result<String> {
        let id = self.askit.new_agent_stream(name)?;
        Ok(id)
    }

    /// Remove an agent stream by its ID, and delete its file.
    /// Update the stream infos list.
    pub async fn remove_agent_stream(&self, stream_id: &str) -> Result<()> {
        let info = self
            .askit
            .get_agent_stream_info(stream_id)
            .ok_or_else(|| anyhow!("Agent stream not found: {}", stream_id))?;
        self.askit.remove_agent_stream(stream_id).await?;
        let stream_path = agent_stream_path(&info.name)?;
        if stream_path.exists() {
            std::fs::remove_file(stream_path)
                .with_context(|| "Failed to remove agent stream file")?;
        }
        Ok(())
    }

    /// Rename an agent stream by its ID, and rename its file.
    /// Update the stream infos list.
    pub fn rename_agent_stream(&self, stream_id: &str, new_name: &str) -> Result<String> {
        let old_info = self
            .askit
            .get_agent_stream_info(stream_id)
            .ok_or_else(|| anyhow!("Agent stream not found: {}", stream_id))?;
        let old_name = old_info.name.clone();
        let new_stream_path = agent_stream_path(new_name)?;
        if new_stream_path.exists() {
            bail!("Agent stream file already exists: {:?}", new_stream_path);
        }

        self.askit.rename_agent_stream(stream_id, new_name)?;

        let old_stream_path = agent_stream_path(&old_name)?;
        if old_stream_path.exists() {
            std::fs::rename(old_stream_path, new_stream_path)
                .with_context(|| "Failed to rename old agent stream file")?;
        }

        Ok(new_name.to_string())
    }

    pub fn save_agent_stream(&self, name: String, spec: AgentStreamSpec) -> Result<()> {
        let stream_path = agent_stream_path(&name)?;

        // Ensure the parent directory exists
        let parent_path = stream_path.parent().context("no parent path")?;
        if !parent_path.exists() {
            std::fs::create_dir_all(parent_path)?;
        }

        let json = spec.to_json()?;
        std::fs::write(stream_path, json).with_context(|| "Failed to write agent stream file")?;

        Ok(())
    }

    pub fn import_agent_stream(&self, path: String) -> Result<String> {
        let path = PathBuf::from(path);
        let name = path
            .file_stem()
            .context("Failed to get file stem")?
            .to_string_lossy()
            .to_string();

        let spec = self.read_agent_stream(path)?;

        let name = self.askit.unique_stream_name(&name);

        let id = self
            .askit
            .add_agent_stream(name.clone(), spec.clone())
            .context("Failed to add agent stream")?;

        self.save_agent_stream(name, spec)?;

        Ok(id)
    }

    pub async fn start_agent_stream(&self, stream_id: &str) -> Result<()> {
        self.askit.start_agent_stream(stream_id).await?;
        Ok(())
    }

    pub async fn stop_agent_stream(&self, stream_id: &str) -> Result<()> {
        self.askit.stop_agent_stream(stream_id).await?;
        Ok(())
    }

    fn read_agent_streams_dir(&mut self) -> Result<()> {
        let streams_dir = agent_streams_dir()?;
        if !streams_dir.exists() {
            std::fs::create_dir_all(&streams_dir)
                .with_context(|| "Failed to create streams directory")?;
            return Ok(());
        }
        self.read_agent_streams_dir_recursive(&streams_dir, "")?;
        Ok(())
    }

    fn read_agent_streams_dir_recursive(&mut self, dir: &PathBuf, name_prefix: &str) -> Result<()> {
        if !dir.exists() || !dir.is_dir() {
            return Ok(());
        }

        let entries = std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {:?}", dir))?;

        for entry in entries {
            if let Err(err) = entry {
                log::warn!("Failed to read directory entry: {}", err);
                continue;
            };
            let path = entry?.path();
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
                // Get the base name from the file name
                let base_name = path
                    .file_stem()
                    .context("Failed to get file stem")?
                    .to_string_lossy()
                    .trim()
                    .to_string();
                let name = if name_prefix.is_empty() {
                    base_name
                } else {
                    format!("{}/{}", name_prefix, base_name)
                };
                let spec = match self.read_agent_stream(path) {
                    Ok(spec) => spec,
                    Err(e) => {
                        log::error!("Failed to read agent stream: {}", e);
                        continue;
                    }
                };
                if let Err(e) = self.askit.add_agent_stream(name, spec) {
                    log::error!("Failed to add agent stream: {}", e);
                    continue;
                }
            }
        }

        Ok(())
    }

    fn read_agent_stream(&self, path: PathBuf) -> Result<AgentStreamSpec> {
        if !path.is_file() || path.extension().unwrap_or_default() != "json" {
            bail!("Invalid file extension");
        }
        let content = std::fs::read_to_string(&path)?;
        let spec = AgentStreamSpec::from_json(&content)?;
        Ok(spec)
    }
}

pub fn init(app: &AppHandle) -> Result<()> {
    let askit = app.askit();
    let mut asapp = ASApp::new(askit);
    asapp.read_agent_streams_dir()?;
    app.manage(asapp);
    Ok(())
}

pub async fn ready(app: &AppHandle) -> Result<()> {
    let asapp = app.state::<ASApp>();
    let askit = &asapp.askit;
    let observer = ASAppObserver { app: app.clone() };
    askit.subscribe(Box::new(observer));

    run_auto_start_streams(app).await;

    Ok(())
}

async fn run_auto_start_streams(app: &AppHandle) {
    let auto_start_streams = {
        let core_settings = app.state::<Mutex<CoreSettings>>();
        let guard = core_settings.lock().unwrap();
        guard.auto_start_streams.clone()
    };

    let asapp = app.state::<ASApp>();
    let stream_infos = asapp.askit.get_agent_stream_infos();

    for info in stream_infos {
        if auto_start_streams.contains(&info.name) {
            log::info!("Auto-starting agent stream: {}", &info.name);
            if let Err(e) = asapp.start_agent_stream(&info.id).await {
                log::error!("Failed to auto-start agent stream '{}': {}", &info.name, e);
            }
        }
    }
}

pub fn quit(_app: &AppHandle) {}

fn agent_streams_dir() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().with_context(|| "Failed to get home directory")?;
    let streams_dir = home_dir.join(ASKIT_STREAMS_PATH);
    Ok(streams_dir)
}

// Get the file path for an agent stream based on its name.
// '/' in the name indicates subdirectories.
fn agent_stream_path(stream_name: &str) -> Result<PathBuf> {
    let mut stream_path = agent_streams_dir()?;

    let path_components: Vec<&str> = stream_name.split('/').collect();
    for &component in &path_components[..path_components.len()] {
        stream_path = stream_path.join(component);
    }

    stream_path = stream_path.with_extension("json");

    Ok(stream_path)
}

#[tauri::command]
pub fn new_agent_stream_cmd(asapp: State<'_, ASApp>, name: String) -> Result<String, String> {
    asapp.new_agent_stream(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_agent_stream_cmd(
    asapp: State<'_, ASApp>,
    id: String,
    name: String,
) -> Result<String, String> {
    asapp
        .rename_agent_stream(&id, &name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_agent_stream_cmd(asapp: State<'_, ASApp>, id: String) -> Result<(), String> {
    asapp
        .remove_agent_stream(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_agent_stream_cmd(
    asapp: State<'_, ASApp>,
    name: String,
    spec: AgentStreamSpec,
) -> Result<(), String> {
    asapp
        .save_agent_stream(name, spec)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_agent_stream_cmd(asapp: State<'_, ASApp>, path: String) -> Result<String, String> {
    asapp.import_agent_stream(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_agent_stream_cmd(asapp: State<'_, ASApp>, id: String) -> Result<(), String> {
    asapp
        .start_agent_stream(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_agent_stream_cmd(asapp: State<'_, ASApp>, id: String) -> Result<(), String> {
    asapp
        .stop_agent_stream(&id)
        .await
        .map_err(|e| e.to_string())
}
