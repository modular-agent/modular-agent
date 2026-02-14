use std::collections::HashMap;
use std::sync::Arc;
use std::{path::PathBuf, sync::Mutex};

use anyhow::{anyhow, bail, Context as _, Result};
use dirs;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use modular_agent_core::mcp::register_tools_from_mcp_json;
use modular_agent_core::{ModularAgent, PresetSpec};

use tauri_plugin_modular_agent::ModularAgentExt;

use crate::modular_agent_desktop::{
    observer::start_modular_agent_observer, settings::CoreSettings,
};

static MODULAR_AGENT_PATH: &'static str = ".modular_agent";
static MODULAR_AGENT_PRESETS_PATH: &'static str = "presets";

const EMIT_PRESET_LIST_CHANGED: &str = "ma:preset_list_changed";
const EMIT_PRESET_RENAMED: &str = "ma:preset_renamed";

#[derive(Clone, Serialize)]
struct PresetListChangedPayload {
    path: String,
}

#[derive(Clone, Serialize)]
struct PresetRenamedPayload {
    id: String,
    #[serde(rename = "newName")]
    new_name: String,
}

/// Extract parent directory path from a preset name.
/// e.g., "Category/MyPreset" -> "Category", "MyPreset" -> ""
fn parent_preset_path(name: &str) -> String {
    match name.rfind('/') {
        Some(i) => name[..i].to_string(),
        None => String::new(),
    }
}

pub struct ModularAgentApp {
    ma: ModularAgent,

    /// Map of preset name to preset ID.
    presets: Arc<Mutex<HashMap<String, String>>>,
}

impl ModularAgentApp {
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
        // If the preset is opened, remove it from ModularAgent core.
        if let Some(preset_id) = self.get_preset_id(name) {
            self.ma.remove_preset(&preset_id).await?;
        }

        // Remove from the presets HashMap
        {
            let mut presets = self.presets.lock().unwrap();
            presets.remove(name);
        }

        // Delete the file from disk
        let preset_path = preset_path(name)?;
        if preset_path.exists() {
            std::fs::remove_file(preset_path).with_context(|| "Failed to remove preset file")?;
        }

        Ok(())
    }

    /// Move a preset file to a different directory.
    pub async fn move_preset(&self, app: &AppHandle, name: &str, target_dir: &str) -> Result<()> {
        if name.contains("..") || name.contains('\\') {
            bail!("Invalid preset name");
        }

        let basename = name.rsplit('/').next().unwrap_or(name);
        let new_name = if target_dir.is_empty() {
            basename.to_string()
        } else {
            format!("{}/{}", target_dir, basename)
        };

        if name == new_name {
            return Ok(());
        }

        if !is_valid_preset_name(&new_name) {
            bail!("Invalid preset name: {}", new_name);
        }

        // Block moving running presets
        if let Some(id) = self.get_preset_id(name) {
            let infos = self.ma.get_preset_infos().await;
            if infos.iter().any(|p| p.id == id && p.running) {
                bail!("Cannot move a running preset. Stop it first.");
            }
        }

        let old_path = preset_path(name)?;
        let new_path = preset_path(&new_name)?;

        if !old_path.exists() {
            bail!("Preset file not found: {}", name);
        }
        if new_path.exists() {
            bail!("A preset with this name already exists: {}", new_name);
        }

        // Ensure target directory exists
        if let Some(parent) = new_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Move file — source and target are always under ~/.modular_agent/presets/
        std::fs::rename(&old_path, &new_path)
            .with_context(|| format!("Failed to move preset: {} -> {}", name, new_name))?;

        // Update in-memory state if preset is open
        if let Some(id) = self.get_preset_id(name) {
            {
                let mut presets = self.presets.lock().unwrap();
                presets.remove(name);
                presets.insert(new_name.clone(), id.clone());
            }
            // Update core Preset.name
            if let Err(e) = self.ma.rename_preset(&id, new_name.clone()).await {
                log::warn!("move_preset: rename_preset({}) failed: {}", id, e);
            }
            let _ = app.emit(EMIT_PRESET_RENAMED, PresetRenamedPayload {
                id,
                new_name: new_name.clone(),
            });
        }

        // Update auto_start_presets
        update_auto_start_presets(app, name, &new_name);

        // Emit list changed for both old and new parent directories
        let old_parent = parent_preset_path(name);
        let new_parent = parent_preset_path(&new_name);
        let _ = app.emit(EMIT_PRESET_LIST_CHANGED, PresetListChangedPayload { path: old_parent.clone() });
        let _ = app.emit(EMIT_PRESET_LIST_CHANGED, PresetListChangedPayload { path: new_parent });

        // Clean up empty ancestor directories
        if let Some(parent) = old_path.parent() {
            if let Ok(root) = presets_dir() {
                cleanup_empty_ancestors(app, parent, &root);
            }
        }

        Ok(())
    }

    /// Move a folder (and all its contents) to a different directory.
    pub async fn move_folder(&self, app: &AppHandle, path: &str, target_dir: &str) -> Result<()> {
        // Validate paths to prevent path traversal
        if !path.is_empty() && (path.contains("..") || path.contains('\\') || path.starts_with('/')) {
            bail!("Invalid folder path");
        }
        if !target_dir.is_empty() && (target_dir.contains("..") || target_dir.contains('\\') || target_dir.starts_with('/')) {
            bail!("Invalid target directory");
        }

        let basename = path.rsplit('/').next().unwrap_or(path);
        let new_path_str = if target_dir.is_empty() {
            basename.to_string()
        } else {
            format!("{}/{}", target_dir, basename)
        };

        if path == new_path_str {
            return Ok(());
        }

        // Prevent moving into self
        let self_prefix = format!("{}/", path);
        if target_dir.starts_with(&self_prefix) || target_dir == path {
            bail!("Cannot move a folder into itself");
        }

        let presets_root = presets_dir()?;
        let old_dir = presets_root.join(path);
        let new_dir = presets_root.join(&new_path_str);

        if !old_dir.exists() || !old_dir.is_dir() {
            bail!("Folder not found: {}", path);
        }
        if new_dir.exists() {
            bail!("A folder with this name already exists: {}", new_path_str);
        }

        // Block if any preset inside the folder is running
        let infos_needed: Vec<String> = {
            let presets = self.presets.lock().unwrap();
            presets
                .iter()
                .filter(|(name, _)| name.starts_with(&self_prefix))
                .map(|(_, id)| id.clone())
                .collect()
        };
        if !infos_needed.is_empty() {
            let infos = self.ma.get_preset_infos().await;
            for id in &infos_needed {
                if infos.iter().any(|p| &p.id == id && p.running) {
                    bail!("Cannot move folder: a preset inside it is running. Stop it first.");
                }
            }
        }

        // Ensure target parent directory exists
        if let Some(parent) = new_dir.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Move directory
        std::fs::rename(&old_dir, &new_dir)
            .with_context(|| format!("Failed to move folder: {} -> {}", path, new_path_str))?;

        // Update all open presets that were inside the moved folder
        let old_prefix = format!("{}/", path);
        let new_prefix = format!("{}/", new_path_str);
        let affected: Vec<(String, String)> = {
            let presets = self.presets.lock().unwrap();
            presets
                .iter()
                .filter(|(name, _)| name.starts_with(&old_prefix))
                .map(|(name, id)| (name.clone(), id.clone()))
                .collect()
        };

        for (old_name, id) in &affected {
            let new_name = format!("{}{}", new_prefix, &old_name[old_prefix.len()..]);
            {
                let mut presets = self.presets.lock().unwrap();
                presets.remove(old_name);
                presets.insert(new_name.clone(), id.clone());
            }
            if let Err(e) = self.ma.rename_preset(id, new_name.clone()).await {
                log::warn!("move_folder: rename_preset({}) failed: {}", id, e);
            }
            let _ = app.emit(EMIT_PRESET_RENAMED, PresetRenamedPayload {
                id: id.clone(),
                new_name,
            });
        }

        // Update auto_start_presets for all affected entries
        update_auto_start_presets_prefix(app, &old_prefix, &new_prefix);

        // Emit list changed for both old and new parent directories
        let old_parent = parent_preset_path(path);
        let new_parent = parent_preset_path(&new_path_str);
        let _ = app.emit(EMIT_PRESET_LIST_CHANGED, PresetListChangedPayload { path: old_parent.clone() });
        let _ = app.emit(EMIT_PRESET_LIST_CHANGED, PresetListChangedPayload { path: new_parent });

        // Clean up empty ancestor directories
        if let Some(parent) = old_dir.parent() {
            cleanup_empty_ancestors(app, parent, &presets_root);
        }

        Ok(())
    }

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

    pub async fn import_preset(&self, path: String, target_dir: String) -> Result<String> {
        let path_buf = PathBuf::from(&path);
        let file_stem = path_buf
            .file_stem()
            .context("Failed to get file stem")?
            .to_string_lossy()
            .to_string();

        let base_name = if target_dir.is_empty() {
            file_stem
        } else {
            format!("{}/{}", target_dir, file_stem)
        };

        // Validate before any file I/O (prevents path traversal via target_dir)
        if !is_valid_preset_name(&base_name) {
            return Err(anyhow!("Invalid preset name: {}", base_name));
        }

        let name = unique_preset_name(&base_name);

        // Read and validate the imported file
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file: {}", path))?;
        let spec = PresetSpec::from_json(&content)
            .map_err(|e| anyhow!("Failed to parse preset: {}", e))?;

        // Save to local presets directory
        self.save_preset(name.clone(), spec)?;

        // Open the preset; clean up orphaned file on failure
        match self.open_preset(name.clone()).await {
            Ok(id) => Ok(id),
            Err(e) => {
                if let Ok(p) = preset_path(&name) {
                    let _ = std::fs::remove_file(p);
                }
                Err(e)
            }
        }
    }

    pub async fn start_preset(&self, preset_id: &str) -> Result<()> {
        self.ma.start_preset(preset_id).await?;
        Ok(())
    }

    pub async fn stop_preset(&self, preset_id: &str) -> Result<()> {
        self.ma.stop_preset(preset_id).await?;
        Ok(())
    }

    /// Close a preset by ID (unload from memory, does NOT delete file).
    /// Only unloads if the preset is not running.
    /// Returns Ok(true) if unloaded, Ok(false) if still running.
    pub async fn close_preset(&self, preset_id: &str) -> Result<bool> {
        // Check if running — if so, keep it loaded
        let infos = self.ma.get_preset_infos().await;
        if infos.iter().any(|p| p.id == preset_id && p.running) {
            return Ok(false);
        }

        // Remove from core (stops agents, removes from core's presets map).
        // Ignore "not found" errors — preset may have already been removed.
        if let Err(e) = self.ma.remove_preset(preset_id).await {
            log::warn!(
                "close_preset: remove_preset({}) failed: {}",
                preset_id,
                e
            );
        }

        // Remove from our name→ID HashMap (reverse lookup by value)
        {
            let mut presets = self.presets.lock().unwrap();
            presets.retain(|_, v| v != preset_id);
        }

        Ok(true)
    }

    fn get_preset_id(&self, name: &str) -> Option<String> {
        let presets = self.presets.lock().unwrap();
        presets.get(name).cloned()
    }
}

pub fn init(app: &AppHandle) -> Result<()> {
    let ma = app.ma();
    let asapp = ModularAgentApp::new(ma);
    app.manage(asapp);
    Ok(())
}

pub async fn ready(app: &AppHandle) -> Result<()> {
    let asapp = app.state::<ModularAgentApp>();
    let ma = &asapp.ma;
    start_modular_agent_observer(&ma, app.clone());

    start_mcp_services().await?;

    run_auto_start_presets(app).await;

    Ok(())
}

async fn start_mcp_services() -> Result<()> {
    let modular_agent_dir = modular_agent_dir()?;
    let mcp_path = modular_agent_dir.join("mcp.json");
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

    let asapp = app.state::<ModularAgentApp>();
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

fn modular_agent_dir() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().with_context(|| "Failed to get home directory")?;
    let modular_agent_dir = home_dir.join(MODULAR_AGENT_PATH);
    Ok(modular_agent_dir)
}

fn presets_dir() -> Result<PathBuf> {
    let modular_agent_dir = modular_agent_dir()?;
    let presets_dir = modular_agent_dir.join(MODULAR_AGENT_PRESETS_PATH);
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

fn preset_path_exists(name: &str) -> bool {
    preset_path(name).map(|p| p.exists()).unwrap_or(false)
}

fn unique_preset_name(base_name: &str) -> String {
    if !preset_path_exists(base_name) {
        return base_name.to_string();
    }
    let copy_name = format!("{} copy", base_name);
    if !preset_path_exists(&copy_name) {
        return copy_name;
    }
    for i in 2.. {
        let name = format!("{} copy {}", base_name, i);
        if !preset_path_exists(&name) {
            return name;
        }
    }
    unreachable!()
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

/// Remove empty directories walking up from `start_dir` toward `presets_root`.
fn cleanup_empty_ancestors(app: &AppHandle, start_dir: &std::path::Path, presets_root: &std::path::Path) {
    let mut dir = start_dir.to_path_buf();
    while dir != *presets_root && dir.starts_with(presets_root) {
        let is_empty = dir
            .read_dir()
            .map(|mut d| d.next().is_none())
            .unwrap_or(false);
        if !is_empty {
            break;
        }
        if let Err(e) = std::fs::remove_dir(&dir) {
            log::warn!("Failed to remove empty directory {:?}: {}", dir, e);
            break;
        }
        // Emit list changed for the parent of the deleted directory
        if let Ok(rel) = dir.strip_prefix(presets_root) {
            let parent_path = rel
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let _ = app.emit(
                EMIT_PRESET_LIST_CHANGED,
                PresetListChangedPayload { path: parent_path },
            );
        }
        dir = match dir.parent() {
            Some(p) => p.to_path_buf(),
            None => break,
        };
    }
}

/// Update auto_start_presets: replace exact match of old_name with new_name.
fn update_auto_start_presets(app: &AppHandle, old_name: &str, new_name: &str) {
    let core_settings = app.state::<Mutex<CoreSettings>>();
    let mut settings = core_settings.lock().unwrap();
    let mut changed = false;
    for entry in settings.auto_start_presets.iter_mut() {
        if entry == old_name {
            *entry = new_name.to_string();
            changed = true;
        }
    }
    drop(settings);
    if changed {
        let _ = crate::modular_agent_desktop::settings::save(app);
    }
}

/// Update auto_start_presets: replace old prefix with new prefix for folder moves.
fn update_auto_start_presets_prefix(app: &AppHandle, old_prefix: &str, new_prefix: &str) {
    let core_settings = app.state::<Mutex<CoreSettings>>();
    let mut settings = core_settings.lock().unwrap();
    let mut changed = false;
    for entry in settings.auto_start_presets.iter_mut() {
        if entry.starts_with(old_prefix) {
            *entry = format!("{}{}", new_prefix, &entry[old_prefix.len()..]);
            changed = true;
        }
    }
    drop(settings);
    if changed {
        let _ = crate::modular_agent_desktop::settings::save(app);
    }
}

#[tauri::command]
pub fn new_preset_with_name_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    name: String,
) -> Result<String, String> {
    let parent_dir = parent_preset_path(&name);
    let parent_existed =
        parent_dir.is_empty() || presets_dir().map(|d| d.join(&parent_dir).exists()).unwrap_or(true);
    let id = asapp
        .new_preset_with_name(name.clone())
        .map_err(|e| e.to_string())?;
    // Save empty preset to disk immediately so it appears in the sidebar
    asapp
        .save_preset(name.clone(), PresetSpec::default())
        .map_err(|e| e.to_string())?;
    let _ = app.emit(
        EMIT_PRESET_LIST_CHANGED,
        PresetListChangedPayload {
            path: parent_dir.clone(),
        },
    );
    if !parent_existed {
        let _ = app.emit(
            EMIT_PRESET_LIST_CHANGED,
            PresetListChangedPayload {
                path: parent_preset_path(&parent_dir),
            },
        );
    }
    Ok(id)
}

#[tauri::command]
pub async fn move_preset_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    name: String,
    target_dir: String,
) -> Result<(), String> {
    asapp
        .move_preset(&app, &name, &target_dir)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn move_folder_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    path: String,
    target_dir: String,
) -> Result<(), String> {
    asapp
        .move_folder(&app, &path, &target_dir)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_preset_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    name: String,
) -> Result<(), String> {
    asapp
        .delete_preset(&name)
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit(
        EMIT_PRESET_LIST_CHANGED,
        PresetListChangedPayload {
            path: parent_preset_path(&name),
        },
    );
    Ok(())
}

#[tauri::command]
pub fn save_preset_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    name: String,
    spec: PresetSpec,
) -> Result<(), String> {
    let is_new = !preset_path_exists(&name);
    let parent_dir = parent_preset_path(&name);
    let parent_existed =
        parent_dir.is_empty() || presets_dir().map(|d| d.join(&parent_dir).exists()).unwrap_or(true);
    asapp
        .save_preset(name.clone(), spec)
        .map_err(|e| e.to_string())?;
    if is_new {
        let _ = app.emit(
            EMIT_PRESET_LIST_CHANGED,
            PresetListChangedPayload {
                path: parent_dir.clone(),
            },
        );
        if !parent_existed {
            let _ = app.emit(
                EMIT_PRESET_LIST_CHANGED,
                PresetListChangedPayload {
                    path: parent_preset_path(&parent_dir),
                },
            );
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn import_preset_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    path: String,
    target_dir: String,
) -> Result<String, String> {
    let id = asapp
        .import_preset(path, target_dir.clone())
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit(
        EMIT_PRESET_LIST_CHANGED,
        PresetListChangedPayload {
            path: target_dir,
        },
    );
    Ok(id)
}

#[tauri::command]
pub async fn start_preset_cmd(asapp: State<'_, ModularAgentApp>, id: String) -> Result<(), String> {
    asapp.start_preset(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_preset_cmd(asapp: State<'_, ModularAgentApp>, id: String) -> Result<(), String> {
    asapp.stop_preset(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn close_preset_cmd(
    asapp: State<'_, ModularAgentApp>,
    id: String,
) -> Result<bool, String> {
    asapp.close_preset(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_dir_entries_cmd(path: String) -> Result<Vec<String>, String> {
    get_dir_entries(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_preset_cmd(
    asapp: State<'_, ModularAgentApp>,
    name: String,
) -> Result<String, String> {
    asapp.open_preset(name).await.map_err(|e| e.to_string())
}
