use std::collections::HashMap;
use std::sync::Arc;
use std::{path::PathBuf, sync::Mutex};

use anyhow::{anyhow, bail, Context as _, Result};
use dirs;
use tauri::{AppHandle, Manager, State};

use modular_agent_core::mcp::register_tools_from_mcp_json;
use modular_agent_core::{ModularAgent, PresetSpec};

use tauri_plugin_modular_agent::MAKExt;

use crate::modular_agent_desktop::{observer::start_mak_observer, settings::CoreSettings};

static MAK_PATH: &'static str = ".modular_agent";
static MAK_PRESETS_PATH: &'static str = "presets";

pub struct MakApp {
    ma: ModularAgent,

    /// Map of preset name to preset ID.
    presets: Arc<Mutex<HashMap<String, String>>>,
}

impl MakApp {
    pub fn new(ma: &ModularAgent) -> Self {
        Self {
            ma: ma.clone(),
            presets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // Preset

    /// Create a new preset.
    pub fn new_preset_with_name(&self, name: String) -> Result<String> {
        if !is_valid_preset_name(&name) {
            return Err(anyhow!("Invalid preset name: {}", name));
        }
        let id = self.ma.new_preset_with_name(name.clone())?;
        let mut presets = self.presets.lock().unwrap();
        presets.insert(name, id.clone());
        Ok(id)
    }

    pub async fn open_preset(&self, name: String) -> Result<String> {
        if !is_valid_preset_name(&name) {
            return Err(anyhow!("Invalid preset name: {}", name));
        }

        // Check if the preset already exists
        if let Some(id) = self.get_preset_id(&name) {
            return Ok(id);
        }

        // open the preset file
        let path = preset_path(&name)?;
        let id = self
            .ma
            .open_preset_from_file(path.to_string_lossy().as_ref(), Some(name.clone()))
            .await?;

        // Store into the presets map
        {
            let mut presets = self.presets.lock().unwrap();
            presets.insert(name, id.clone());
        }

        Ok(id)
    }

    /// Delete a preset by the given name, and delete its file.
    pub async fn delete_preset(&self, name: &str) -> Result<()> {
        let preset_id = self
            .get_preset_id(name)
            .ok_or_else(|| anyhow!("Preset not found: {}", name))?;

        self.ma.remove_preset(&preset_id).await?;

        let preset_path = preset_path(name)?;
        if preset_path.exists() {
            std::fs::remove_file(preset_path).with_context(|| "Failed to remove preset file")?;
        }
        Ok(())
    }

    // /// Rename an preset by its ID, and rename its file.
    // pub fn rename_preset(&self, old_name: &str, new_name: &str) -> Result<String> {
    //     let old_id = self
    //         .get_preset_id(old_name)
    //         .ok_or_else(|| anyhow!("Preset not found: {}", old_name))?;

    //     if !is_valid_preset_name(new_name) {
    //         return Err(anyhow!("Invalid preset name: {}", new_name));
    //     }

    //     Err(anyhow!("Not implemented"))
    // }

    pub fn save_preset(&self, name: String, spec: PresetSpec) -> Result<()> {
        let preset_path = preset_path(&name)?;

        // Ensure the parent directory exists
        let parent_path = preset_path.parent().context("no parent path")?;
        if !parent_path.exists() {
            std::fs::create_dir_all(parent_path)?;
        }

        let json = spec.to_json()?;
        std::fs::write(preset_path, json).with_context(|| "Failed to write preset file")?;
        Ok(())
    }

    pub fn import_preset(&self, path: String) -> Result<String> {
        // let path = PathBuf::from(path);
        // let name = path
        //     .file_stem()
        //     .context("Failed to get file stem")?
        //     .to_string_lossy()
        //     .to_string();

        // let spec = self.read_preset(path)?;

        // let name = self.ma.unique_preset_name(&name);

        // let id = self
        //     .ma
        //     .add_preset(name.clone(), spec.clone())
        //     .context("Failed to add preset")?;

        // self.save_preset(name, spec)?;

        // Ok(id)
        Err(anyhow!("Not implemented"))
    }

    pub async fn start_preset(&self, preset_id: &str) -> Result<()> {
        self.ma.start_preset(preset_id).await?;
        Ok(())
    }

    pub async fn stop_preset(&self, preset_id: &str) -> Result<()> {
        self.ma.stop_preset(preset_id).await?;
        Ok(())
    }

    fn get_preset_id(&self, name: &str) -> Option<String> {
        let presets = self.presets.lock().unwrap();
        presets.get(name).cloned()
    }
}

pub fn init(app: &AppHandle) -> Result<()> {
    let ma = app.ma();
    let asapp = MakApp::new(ma);
    app.manage(asapp);
    Ok(())
}

pub async fn ready(app: &AppHandle) -> Result<()> {
    let asapp = app.state::<MakApp>();
    let ma = &asapp.ma;
    start_mak_observer(&ma, app.clone());

    start_mcp_services().await?;

    run_auto_start_presets(app).await;

    Ok(())
}

async fn start_mcp_services() -> Result<()> {
    let mak_dir = mak_dir()?;
    let mcp_path = mak_dir.join("mcp.json");
    if !mcp_path.exists() {
        return Ok(());
    }

    let tools = register_tools_from_mcp_json(mcp_path).await?;
    log::info!("Registered {} tools:", tools.len());
    for tool in tools {
        log::info!("  - {}", tool);
    }

    Ok(())
}

async fn run_auto_start_presets(app: &AppHandle) {
    let auto_start_presets = {
        let core_settings = app.state::<Mutex<CoreSettings>>();
        let guard = core_settings.lock().unwrap();
        guard.auto_start_presets.clone()
    };

    let asapp = app.state::<MakApp>();
    for name in auto_start_presets {
        log::info!("Auto-starting preset: {}", name);
        match asapp.open_preset(name.clone()).await {
            Ok(id) => {
                if let Err(e) = asapp.start_preset(&id).await {
                    log::error!("Failed to start preset {}: {}", name, e);
                }
            }
            Err(e) => {
                log::error!("Failed to open preset {}: {}", name, e);
                continue;
            }
        }
    }
}

pub fn quit(_app: &AppHandle) {}

fn mak_dir() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().with_context(|| "Failed to get home directory")?;
    let mak_dir = home_dir.join(MAK_PATH);
    Ok(mak_dir)
}

fn presets_dir() -> Result<PathBuf> {
    let mak_dir = mak_dir()?;
    let presets_dir = mak_dir.join(MAK_PRESETS_PATH);
    Ok(presets_dir)
}

// Get the file path for an preset based on its name.
// '/' in the name indicates subdirectories.
fn preset_path(preset_name: &str) -> Result<PathBuf> {
    let mut preset_path = presets_dir()?;

    let path_components: Vec<&str> = preset_name.split('/').collect();
    for &component in &path_components[..path_components.len()] {
        preset_path = preset_path.join(component);
    }

    preset_path = preset_path.with_extension("json");

    Ok(preset_path)
}

fn get_dir_entries(path: &str) -> Result<Vec<String>> {
    if path.starts_with("/") || path.contains("..") {
        bail!("Invalid path: {}", path);
    }
    let mut entries = Vec::new();
    let preset_dir = presets_dir()?;
    let dir = preset_dir.join(path);
    if !dir.exists() || !dir.is_dir() {
        return Ok(entries);
    }

    let dir_entries =
        std::fs::read_dir(&dir).with_context(|| format!("Failed to read directory: {:?}", dir))?;

    for entry in dir_entries {
        let path = entry?.path();
        if path.is_dir() {
            let dir_name = path
                .file_name()
                .context("Failed to get directory name")?
                .to_string_lossy();
            entries.push(format!("{}/", dir_name));
        } else if path.is_file() && path.extension().unwrap_or_default() == "json" {
            // Get the base name from the file name
            let base_name = path
                .file_stem()
                .context("Failed to get file stem")?
                .to_string_lossy()
                .trim()
                .to_string();
            entries.push(base_name);
        }
    }

    Ok(entries)
}

fn is_valid_preset_name(new_name: &str) -> bool {
    // Check if the name is empty
    if new_name.trim().is_empty() {
        return false;
    }

    // Checks for path-like names:
    if new_name.contains('/') {
        // Disallow leading, trailing, or consecutive slashes
        if new_name.starts_with('/') || new_name.ends_with('/') || new_name.contains("//") {
            return false;
        }
        // Disallow segments that are "." or ".."
        if new_name
            .split('/')
            .any(|segment| segment == "." || segment == "..")
        {
            return false;
        }
    }

    // Check if the name contains invalid characters
    let invalid_chars = ['\\', ':', '*', '?', '"', '<', '>', '|'];
    for c in invalid_chars {
        if new_name.contains(c) {
            return false;
        }
    }

    true
}

// fn is_valid_preset_path(path: &str) -> bool {
//     let path = std::path::Path::new(path);

//     // Check for valid file extension
//     match path.extension().and_then(|s| s.to_str()) {
//         Some("json") => {}
//         _ => return false,
//     }

//     // Check for valid file name
//     if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
//         if !is_valid_preset_name(file_stem) {
//             return false;
//         }
//     } else {
//         return false;
//     }

//     true
// }

#[tauri::command]
pub fn new_preset_with_name_cmd(asapp: State<'_, MakApp>, name: String) -> Result<String, String> {
    asapp.new_preset_with_name(name).map_err(|e| e.to_string())
}

// #[tauri::command]
// pub fn rename_preset_cmd(
//     asapp: State<'_, MakApp>,
//     old_name: String,
//     new_name: String,
// ) -> Result<String, String> {
//     asapp
//         .rename_preset(&old_name, &new_name)
//         .map_err(|e| e.to_string())
// }

#[tauri::command]
pub async fn delete_preset_cmd(asapp: State<'_, MakApp>, name: String) -> Result<(), String> {
    asapp.delete_preset(&name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_preset_cmd(
    asapp: State<'_, MakApp>,
    name: String,
    spec: PresetSpec,
) -> Result<(), String> {
    asapp.save_preset(name, spec).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_preset_cmd(asapp: State<'_, MakApp>, path: String) -> Result<String, String> {
    asapp.import_preset(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_preset_cmd(asapp: State<'_, MakApp>, id: String) -> Result<(), String> {
    asapp.start_preset(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_preset_cmd(asapp: State<'_, MakApp>, id: String) -> Result<(), String> {
    asapp.stop_preset(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_dir_entries_cmd(path: String) -> Result<Vec<String>, String> {
    get_dir_entries(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_preset_cmd(asapp: State<'_, MakApp>, name: String) -> Result<String, String> {
    asapp.open_preset(name).await.map_err(|e| e.to_string())
}
