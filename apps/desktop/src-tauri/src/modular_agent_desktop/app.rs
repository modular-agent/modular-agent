use std::{path::PathBuf, sync::Mutex};

use anyhow::{Context as _, Result, anyhow, bail};
use dirs;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use modular_agent_core::mcp::register_tools_from_mcp_json;
use modular_agent_core::{ModularAgent, PatchSpec};

use tauri_plugin_modular_agent::ModularAgentExt;

use crate::modular_agent_desktop::{
    observer::start_modular_agent_observer, settings::CoreSettings,
};

static MODULAR_AGENT_PATH: &str = ".modular_agent";
static MODULAR_AGENT_PATCHES_PATH: &str = "patches";

const EMIT_PATCH_LIST_CHANGED: &str = "ma:patch_list_changed";

#[derive(Clone, Serialize)]
struct PatchListChangedPayload {
    path: String,
}

/// Extract parent directory path from a patch name.
/// e.g., "Category/MyPatch" -> "Category", "MyPatch" -> ""
pub(crate) fn parent_patch_path(name: &str) -> String {
    match name.rfind('/') {
        Some(i) => name[..i].to_string(),
        None => String::new(),
    }
}

pub struct ModularAgentApp {
    ma: ModularAgent,
}

impl ModularAgentApp {
    pub fn new(ma: &ModularAgent) -> Self {
        Self { ma: ma.clone() }
    }

    // Patch

    /// Create a new patch.
    pub fn new_patch_with_name(&self, name: String) -> Result<String> {
        if !is_valid_patch_name(&name) {
            return Err(anyhow!("Invalid patch name: {}", name));
        }
        let id = self.ma.new_patch_with_name(name)?;
        Ok(id)
    }

    /// Create a new patch with the given spec content.
    pub fn add_patch_with_name(&self, spec: PatchSpec, name: String) -> Result<String> {
        if !is_valid_patch_name(&name) {
            return Err(anyhow!("Invalid patch name: {}", name));
        }
        let id = self.ma.add_patch_with_name(spec, name)?;
        Ok(id)
    }

    pub async fn open_patch(&self, name: String) -> Result<String> {
        if !is_valid_patch_name(&name) {
            return Err(anyhow!("Invalid patch name: {}", name));
        }

        // Return the live instance if the patch is already loaded in core
        // (regardless of who created it).
        if let Some(id) = self.ma.find_patch_id_by_name(&name) {
            return Ok(id);
        }

        // open the patch file
        let path = patch_path(&name)?;
        let id = self
            .ma
            .open_patch_from_file(path.to_string_lossy().as_ref(), Some(name))
            .await?;

        Ok(id)
    }

    /// Delete a patch by the given name, and delete its file.
    pub async fn delete_patch(&self, app: &AppHandle, name: &str) -> Result<()> {
        // If the patch is loaded in core, remove it first (core emits the
        // removal event so the UI can close any open tab).
        if let Some(patch_id) = self.ma.find_patch_id_by_name(name) {
            let infos = self.ma.get_patch_infos().await;
            if infos.iter().any(|p| p.id == patch_id && p.running) {
                bail!("Cannot delete patch: it is running. Stop it first.");
            }
            self.ma.remove_patch(&patch_id).await?;
        }

        // Delete the file from disk
        let patch_path = patch_path(name)?;
        if patch_path.exists() {
            std::fs::remove_file(patch_path).with_context(|| "Failed to remove patch file")?;
        }

        remove_auto_start_patches(app, |entry| entry == name);

        Ok(())
    }

    /// Rename a patch file (also used internally by move_patch).
    pub async fn rename_patch(&self, app: &AppHandle, name: &str, new_name: &str) -> Result<()> {
        if !is_valid_patch_name(name) {
            bail!("Invalid patch name: {}", name);
        }
        if !is_valid_patch_name(new_name) {
            bail!("Invalid patch name: {}", new_name);
        }

        if name == new_name {
            return Ok(());
        }

        // Block renaming running patches
        let live_id = self.ma.find_patch_id_by_name(name);
        if let Some(id) = &live_id {
            let infos = self.ma.get_patch_infos().await;
            if infos.iter().any(|p| &p.id == id && p.running) {
                bail!("Cannot rename a running patch. Stop it first.");
            }
        }

        let old_path = patch_path(name)?;
        let new_path = patch_path(new_name)?;

        if !old_path.exists() {
            bail!("Patch file not found: {}", name);
        }
        if new_path.exists() {
            bail!("A patch with this name already exists: {}", new_name);
        }

        // Rename in core first: on a name conflict this fails before any
        // file has been touched. Core emits the rename event for the UI.
        if let Some(id) = &live_id {
            self.ma.rename_patch(id, new_name.to_string()).await?;
        }

        // Move the file. If this fails, roll back the core rename so the live
        // patch does not diverge from its backing file (a diverged name would
        // let open/delete by the old name spawn a duplicate or orphan the live
        // instance).
        let fs_result = (|| -> Result<()> {
            if let Some(parent) = new_path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            // Source and target are always under ~/.modular_agent/patches/
            std::fs::rename(&old_path, &new_path)
                .with_context(|| format!("Failed to rename patch: {} -> {}", name, new_name))
        })();
        if let Err(e) = fs_result {
            if let Some(id) = &live_id {
                if let Err(re) = self.ma.rename_patch(id, name.to_string()).await {
                    log::error!(
                        "Failed to roll back core rename of patch {}: {}",
                        new_name,
                        re
                    );
                }
            }
            return Err(e);
        }

        // Update auto_start_patches
        update_auto_start_patches(app, name, new_name);

        // Emit list changed for both old and new parent directories
        let old_parent = parent_patch_path(name);
        let new_parent = parent_patch_path(new_name);
        let _ = app.emit(
            EMIT_PATCH_LIST_CHANGED,
            PatchListChangedPayload {
                path: old_parent.clone(),
            },
        );
        if new_parent != old_parent {
            let _ = app.emit(
                EMIT_PATCH_LIST_CHANGED,
                PatchListChangedPayload { path: new_parent },
            );
        }

        // Clean up empty ancestor directories
        if let Some(parent) = old_path.parent() {
            if let Ok(root) = patches_dir() {
                cleanup_empty_ancestors(app, parent, &root);
            }
        }

        Ok(())
    }

    /// Move a patch file to a different directory.
    pub async fn move_patch(&self, app: &AppHandle, name: &str, target_dir: &str) -> Result<()> {
        let basename = name.rsplit('/').next().unwrap_or(name);
        let new_name = if target_dir.is_empty() {
            basename.to_string()
        } else {
            format!("{}/{}", target_dir, basename)
        };
        self.rename_patch(app, name, &new_name).await
    }

    /// Rename a folder (and all its contents). Also used internally by move_folder.
    pub async fn rename_folder(
        &self,
        app: &AppHandle,
        path: &str,
        new_path_str: &str,
    ) -> Result<()> {
        // Validate paths to prevent path traversal
        if !path.is_empty() && (path.contains("..") || path.contains('\\') || path.starts_with('/'))
        {
            bail!("Invalid folder path");
        }
        if !new_path_str.is_empty()
            && (new_path_str.contains("..")
                || new_path_str.contains('\\')
                || new_path_str.starts_with('/'))
        {
            bail!("Invalid folder path: {}", new_path_str);
        }

        if path == new_path_str {
            return Ok(());
        }

        // Prevent renaming into self
        let self_prefix = format!("{}/", path);
        if new_path_str.starts_with(&self_prefix) {
            bail!("Cannot rename a folder into itself");
        }

        let patches_root = patches_dir()?;
        let old_dir = patches_root.join(path);
        let new_dir = patches_root.join(new_path_str);

        if !old_dir.exists() || !old_dir.is_dir() {
            bail!("Folder not found: {}", path);
        }
        if new_dir.exists() {
            bail!("A folder with this name already exists: {}", new_path_str);
        }

        // Collect live patches inside the folder; block the rename while any
        // of them is running.
        let mut affected: Vec<(String, String)> = Vec::new();
        for info in self.ma.get_patch_infos().await {
            let Some(patch_name) = info.name.as_deref() else {
                continue;
            };
            if !patch_name.starts_with(&self_prefix) {
                continue;
            }
            if info.running {
                bail!("Cannot rename folder: a patch inside it is running. Stop it first.");
            }
            affected.push((patch_name.to_string(), info.id));
        }

        // Ensure target parent directory exists
        if let Some(parent) = new_dir.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Rename directory
        std::fs::rename(&old_dir, &new_dir)
            .with_context(|| format!("Failed to rename folder: {} -> {}", path, new_path_str))?;

        // Rename all live patches that were inside the renamed folder.
        // Core emits the rename events for the UI.
        let old_prefix = self_prefix;
        let new_prefix = format!("{}/", new_path_str);
        for (old_name, id) in &affected {
            let new_name = format!("{}{}", new_prefix, &old_name[old_prefix.len()..]);
            if let Err(e) = self.ma.rename_patch(id, new_name).await {
                log::warn!("rename_folder: rename_patch({}) failed: {}", id, e);
            }
        }

        // Update auto_start_patches for all affected entries
        update_auto_start_patches_prefix(app, &old_prefix, &new_prefix);

        // Emit list changed for both old and new parent directories
        let old_parent = parent_patch_path(path);
        let new_parent = parent_patch_path(new_path_str);
        let _ = app.emit(
            EMIT_PATCH_LIST_CHANGED,
            PatchListChangedPayload {
                path: old_parent.clone(),
            },
        );
        if new_parent != old_parent {
            let _ = app.emit(
                EMIT_PATCH_LIST_CHANGED,
                PatchListChangedPayload { path: new_parent },
            );
        }

        // Clean up empty ancestor directories
        if let Some(parent) = old_dir.parent() {
            cleanup_empty_ancestors(app, parent, &patches_root);
        }

        Ok(())
    }

    /// Delete an empty folder. Refuses to delete a folder that still has
    /// anything in it — a right-click can easily land on the wrong row, and
    /// wiping a whole subtree is not recoverable.
    pub fn delete_folder(&self, app: &AppHandle, path: &str) -> Result<()> {
        // Validate the path to prevent path traversal. An empty path would
        // resolve to the patches root itself.
        if path.is_empty() || path.contains("..") || path.contains('\\') || path.starts_with('/') {
            bail!("Invalid folder path");
        }

        let patches_root = patches_dir()?;
        let dir = patches_root.join(path);
        if !dir.exists() || !dir.is_dir() {
            bail!("Folder not found: {}", path);
        }

        let is_empty = dir
            .read_dir()
            .with_context(|| format!("Failed to read directory: {:?}", dir))?
            .next()
            .is_none();
        if !is_empty {
            bail!("Cannot delete folder: it is not empty. Delete its contents first.");
        }

        std::fs::remove_dir(&dir).with_context(|| format!("Failed to remove folder: {}", path))?;

        let _ = app.emit(
            EMIT_PATCH_LIST_CHANGED,
            PatchListChangedPayload {
                path: parent_patch_path(path),
            },
        );

        Ok(())
    }

    /// Move a folder (and all its contents) to a different directory.
    pub async fn move_folder(&self, app: &AppHandle, path: &str, target_dir: &str) -> Result<()> {
        let basename = path.rsplit('/').next().unwrap_or(path);
        let new_path_str = if target_dir.is_empty() {
            basename.to_string()
        } else {
            format!("{}/{}", target_dir, basename)
        };
        self.rename_folder(app, path, &new_path_str).await
    }

    pub fn save_patch(&self, name: String, spec: PatchSpec) -> Result<()> {
        let patch_path = patch_path(&name)?;

        // Ensure the parent directory exists
        let parent_path = patch_path.parent().context("no parent path")?;
        if !parent_path.exists() {
            std::fs::create_dir_all(parent_path)?;
        }

        let json = spec.to_json()?;
        std::fs::write(patch_path, json).with_context(|| "Failed to write patch file")?;
        Ok(())
    }

    pub async fn import_patch(&self, path: String, target_dir: String) -> Result<String> {
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
        if !is_valid_patch_name(&base_name) {
            return Err(anyhow!("Invalid patch name: {}", base_name));
        }

        let name = unique_patch_name(&base_name);

        // Read and validate the imported file
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file: {}", path))?;
        let spec =
            PatchSpec::from_json(&content).map_err(|e| anyhow!("Failed to parse patch: {}", e))?;

        // Save to local patches directory
        self.save_patch(name.clone(), spec)?;

        // Open the patch; clean up orphaned file on failure
        match self.open_patch(name.clone()).await {
            Ok(id) => Ok(id),
            Err(e) => {
                if let Ok(p) = patch_path(&name) {
                    let _ = std::fs::remove_file(p);
                }
                Err(e)
            }
        }
    }

    pub async fn start_patch(&self, patch_id: &str) -> Result<()> {
        self.ma.start_patch(patch_id).await?;
        Ok(())
    }

    pub async fn stop_patch(&self, patch_id: &str) -> Result<()> {
        self.ma.stop_patch(patch_id).await?;
        Ok(())
    }

    /// Close a patch by ID (unload from memory, does NOT delete file).
    /// Only unloads if the patch is not running.
    /// Returns Ok(true) if unloaded, Ok(false) if still running.
    pub async fn close_patch(&self, patch_id: &str) -> Result<bool> {
        // Check if running — if so, keep it loaded
        let infos = self.ma.get_patch_infos().await;
        if infos.iter().any(|p| p.id == patch_id && p.running) {
            return Ok(false);
        }

        // Remove from core (stops agents, removes from core's patches map).
        // Ignore "not found" errors — patch may have already been removed.
        if let Err(e) = self.ma.remove_patch(patch_id).await {
            log::warn!("close_patch: remove_patch({}) failed: {}", patch_id, e);
        }

        Ok(true)
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
    start_modular_agent_observer(ma, app.clone());

    start_mcp_services().await?;

    run_auto_start_patches(app).await;

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

async fn run_auto_start_patches(app: &AppHandle) {
    let auto_start_patches = {
        let core_settings = app.state::<Mutex<CoreSettings>>();
        let guard = core_settings.lock().unwrap();
        guard.auto_start_patches.clone()
    };

    let asapp = app.state::<ModularAgentApp>();
    for name in auto_start_patches {
        log::info!("Auto-starting patch: {}", name);
        match asapp.open_patch(name.clone()).await {
            Ok(id) => {
                if let Err(e) = asapp.start_patch(&id).await {
                    log::error!("Failed to start patch {}: {}", name, e);
                }
            }
            Err(e) => {
                log::error!("Failed to open patch {}: {}", name, e);
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

pub(crate) fn patches_dir() -> Result<PathBuf> {
    let modular_agent_dir = modular_agent_dir()?;
    let patches_dir = modular_agent_dir.join(MODULAR_AGENT_PATCHES_PATH);
    Ok(patches_dir)
}

// Get the file path for an patch based on its name.
// '/' in the name indicates subdirectories.
fn patch_path(patch_name: &str) -> Result<PathBuf> {
    let mut patch_path = patches_dir()?;

    let path_components: Vec<&str> = patch_name.split('/').collect();
    for &component in &path_components[..path_components.len()] {
        patch_path = patch_path.join(component);
    }

    patch_path = patch_path.with_extension("json");

    Ok(patch_path)
}

fn patch_path_exists(name: &str) -> bool {
    patch_path(name).map(|p| p.exists()).unwrap_or(false)
}

fn unique_patch_name(base_name: &str) -> String {
    if !patch_path_exists(base_name) {
        return base_name.to_string();
    }
    let copy_name = format!("{} copy", base_name);
    if !patch_path_exists(&copy_name) {
        return copy_name;
    }
    for i in 2.. {
        let name = format!("{} copy {}", base_name, i);
        if !patch_path_exists(&name) {
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
    let patch_dir = patches_dir()?;
    let dir = patch_dir.join(path);
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

fn is_valid_patch_name(new_name: &str) -> bool {
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

/// Remove empty directories walking up from `start_dir` toward `patches_root`.
fn cleanup_empty_ancestors(
    app: &AppHandle,
    start_dir: &std::path::Path,
    patches_root: &std::path::Path,
) {
    let mut dir = start_dir.to_path_buf();
    while dir != *patches_root && dir.starts_with(patches_root) {
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
        if let Ok(rel) = dir.strip_prefix(patches_root) {
            let parent_path = rel
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let _ = app.emit(
                EMIT_PATCH_LIST_CHANGED,
                PatchListChangedPayload { path: parent_path },
            );
        }
        dir = match dir.parent() {
            Some(p) => p.to_path_buf(),
            None => break,
        };
    }
}

/// Update auto_start_patches: replace exact match of old_name with new_name.
fn update_auto_start_patches(app: &AppHandle, old_name: &str, new_name: &str) {
    let core_settings = app.state::<Mutex<CoreSettings>>();
    let mut settings = core_settings.lock().unwrap();
    let mut changed = false;
    for entry in settings.auto_start_patches.iter_mut() {
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

/// Update auto_start_patches: replace old prefix with new prefix for folder moves.
fn update_auto_start_patches_prefix(app: &AppHandle, old_prefix: &str, new_prefix: &str) {
    let core_settings = app.state::<Mutex<CoreSettings>>();
    let mut settings = core_settings.lock().unwrap();
    let mut changed = false;
    for entry in settings.auto_start_patches.iter_mut() {
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

/// Drop auto_start_patches entries matching `is_removed`.
fn remove_auto_start_patches(app: &AppHandle, is_removed: impl Fn(&str) -> bool) {
    let core_settings = app.state::<Mutex<CoreSettings>>();
    let mut settings = core_settings.lock().unwrap();
    let before = settings.auto_start_patches.len();
    settings.auto_start_patches.retain(|e| !is_removed(e));
    let changed = settings.auto_start_patches.len() != before;
    drop(settings);
    if changed {
        let _ = crate::modular_agent_desktop::settings::save(app);
    }
}

#[tauri::command]
pub fn new_patch_with_name_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    name: String,
) -> Result<String, String> {
    let parent_dir = parent_patch_path(&name);
    let parent_existed = parent_dir.is_empty()
        || patches_dir()
            .map(|d| d.join(&parent_dir).exists())
            .unwrap_or(true);
    let id = asapp
        .new_patch_with_name(name.clone())
        .map_err(|e| e.to_string())?;
    // Save empty patch to disk immediately so it appears in the sidebar
    asapp
        .save_patch(name.clone(), PatchSpec::default())
        .map_err(|e| e.to_string())?;
    let _ = app.emit(
        EMIT_PATCH_LIST_CHANGED,
        PatchListChangedPayload {
            path: parent_dir.clone(),
        },
    );
    if !parent_existed {
        let _ = app.emit(
            EMIT_PATCH_LIST_CHANGED,
            PatchListChangedPayload {
                path: parent_patch_path(&parent_dir),
            },
        );
    }
    Ok(id)
}

#[tauri::command]
pub async fn move_patch_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    name: String,
    target_dir: String,
) -> Result<(), String> {
    asapp
        .move_patch(&app, &name, &target_dir)
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
pub async fn rename_patch_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    name: String,
    new_name: String,
) -> Result<(), String> {
    asapp
        .rename_patch(&app, &name, &new_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_folder_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    path: String,
    new_path: String,
) -> Result<(), String> {
    asapp
        .rename_folder(&app, &path, &new_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_patch_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    name: String,
) -> Result<(), String> {
    asapp
        .delete_patch(&app, &name)
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit(
        EMIT_PATCH_LIST_CHANGED,
        PatchListChangedPayload {
            path: parent_patch_path(&name),
        },
    );
    Ok(())
}

#[tauri::command]
pub fn delete_folder_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    path: String,
) -> Result<(), String> {
    asapp.delete_folder(&app, &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_patch_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    name: String,
    spec: PatchSpec,
) -> Result<(), String> {
    let is_new = !patch_path_exists(&name);
    let parent_dir = parent_patch_path(&name);
    let parent_existed = parent_dir.is_empty()
        || patches_dir()
            .map(|d| d.join(&parent_dir).exists())
            .unwrap_or(true);
    asapp
        .save_patch(name.clone(), spec)
        .map_err(|e| e.to_string())?;
    if is_new {
        let _ = app.emit(
            EMIT_PATCH_LIST_CHANGED,
            PatchListChangedPayload {
                path: parent_dir.clone(),
            },
        );
        if !parent_existed {
            let _ = app.emit(
                EMIT_PATCH_LIST_CHANGED,
                PatchListChangedPayload {
                    path: parent_patch_path(&parent_dir),
                },
            );
        }
    }
    Ok(())
}

#[tauri::command]
pub fn save_as_patch_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    name: String,
    spec: PatchSpec,
) -> Result<String, String> {
    let parent_dir = parent_patch_path(&name);
    let parent_existed = parent_dir.is_empty()
        || patches_dir()
            .map(|d| d.join(&parent_dir).exists())
            .unwrap_or(true);

    // Add to core engine with spec content
    let id = asapp
        .add_patch_with_name(spec.clone(), name.clone())
        .map_err(|e| e.to_string())?;

    // Save spec to disk
    asapp
        .save_patch(name.clone(), spec)
        .map_err(|e| e.to_string())?;

    // Emit sidebar refresh event
    let _ = app.emit(
        EMIT_PATCH_LIST_CHANGED,
        PatchListChangedPayload {
            path: parent_dir.clone(),
        },
    );
    if !parent_existed {
        let _ = app.emit(
            EMIT_PATCH_LIST_CHANGED,
            PatchListChangedPayload {
                path: parent_patch_path(&parent_dir),
            },
        );
    }

    Ok(id)
}

#[tauri::command]
pub async fn import_patch_cmd(
    app: AppHandle,
    asapp: State<'_, ModularAgentApp>,
    path: String,
    target_dir: String,
) -> Result<String, String> {
    let id = asapp
        .import_patch(path, target_dir.clone())
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit(
        EMIT_PATCH_LIST_CHANGED,
        PatchListChangedPayload { path: target_dir },
    );
    Ok(id)
}

#[tauri::command]
pub async fn start_patch_cmd(asapp: State<'_, ModularAgentApp>, id: String) -> Result<(), String> {
    asapp.start_patch(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_patch_cmd(asapp: State<'_, ModularAgentApp>, id: String) -> Result<(), String> {
    asapp.stop_patch(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn close_patch_cmd(
    asapp: State<'_, ModularAgentApp>,
    id: String,
) -> Result<bool, String> {
    asapp.close_patch(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_dir_entries_cmd(path: String) -> Result<Vec<String>, String> {
    get_dir_entries(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_patch_cmd(
    asapp: State<'_, ModularAgentApp>,
    name: String,
) -> Result<String, String> {
    asapp.open_patch(name).await.map_err(|e| e.to_string())
}
