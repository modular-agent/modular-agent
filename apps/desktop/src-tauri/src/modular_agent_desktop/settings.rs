use anyhow::{Context as _, Result};
use modular_agent_core::CancellationToken;
use modular_agent_core::indexmap::IndexMap;
use modular_agent_core::mcp_server::{McpServerConfig, McpServerHandle, mcp_router};
use modular_agent_core::workspace::Workspace;
use parking_lot::Mutex;
use rand::RngExt as _;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{collections::HashMap, net::Ipv4Addr, ops::Not, sync::LazyLock};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_modular_agent::ModularAgentExt;
use tauri_plugin_store::StoreExt;

const SETTINGS_JSON: &str = "settings.json";

pub fn init(app: &AppHandle) -> Result<()> {
    init_core_settings(app)?;
    init_workspace(app)?;
    app.manage(McpServerState::default());
    Ok(())
}

/// Saves the UI settings to the Tauri store. Settings that affect how patches
/// run live in the workspace; see [`save_workspace`].
pub fn save(app: &AppHandle) -> Result<()> {
    let store = app.store(SETTINGS_JSON)?;

    let core_settings = app.state::<Mutex<CoreSettings>>();
    let settings_json;
    {
        let core_settings = core_settings.lock();
        settings_json = serde_json::to_value(&*core_settings)?;
    }
    store.set("core", settings_json);

    Ok(())
}

pub fn quit(_app: &AppHandle) {
    // save(app);
}

// Core Settings

#[derive(Debug, Serialize, Deserialize)]
pub struct CoreSettings {
    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub autostart: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_mode: Option<String>,

    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub run_in_background: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut_keys: Option<HashMap<String, String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snap_enabled: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snap_grid_size: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_gap: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_history_length: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_opacity: Option<f64>,

    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub mcp_server_enabled: bool,

    #[serde(default = "default_mcp_server_port")]
    pub mcp_server_port: u16,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_server_token: Option<String>,
}

fn default_mcp_server_port() -> u16 {
    8765
}

impl Default for CoreSettings {
    fn default() -> Self {
        static SHORTCUT_KEYS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
            let mut map = HashMap::new();
            map.insert("global_shortcut".into(), "".into());
            #[cfg(target_os = "macos")]
            {
                map.insert("fullscreen".into(), "".into()); // macOS has its own fullscreen shortcut (Cmd+Ctrl+F)
            }
            #[cfg(not(target_os = "macos"))]
            {
                map.insert("fullscreen".into(), "F11".into());
            }
            map
        });

        CoreSettings {
            autostart: false,
            color_mode: None,
            run_in_background: false,
            shortcut_keys: Some(SHORTCUT_KEYS.clone()),
            snap_enabled: None,
            snap_grid_size: None,
            grid_gap: None,
            max_history_length: None,
            connection_opacity: None,
            mcp_server_enabled: false,
            mcp_server_port: default_mcp_server_port(),
            mcp_server_token: None,
        }
    }
}

fn init_core_settings(app: &AppHandle) -> Result<()> {
    let store = app.store(SETTINGS_JSON)?;

    let core_settings: CoreSettings;
    if let Some(store_value) = store.get("core") {
        let mut value = serde_json::to_value(CoreSettings::default())
            .context("Failed to serialize default core settings")?;
        json_merge(&mut value, store_value.clone());

        core_settings = serde_json::from_value(value).unwrap_or_else(|e| {
            log::error!("Failed to load core settings: {}", e);
            CoreSettings::default()
        });
    } else {
        core_settings = CoreSettings::default();
    }

    app.manage(Mutex::new(core_settings));

    Ok(())
}

// Workspace

/// Opens `~/.modular_agent` as the workspace. A settings.json that cannot be
/// read is an error rather than a reset to defaults: the next save would
/// overwrite the hand-written file.
fn init_workspace(app: &AppHandle) -> Result<()> {
    let dir = crate::modular_agent_desktop::app::modular_agent_dir()?;
    std::fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    // Relative paths in patches (e.g. data/...) resolve against the workspace,
    // as they do under `ma serve`, whatever directory the app was launched from.
    std::env::set_current_dir(&dir)
        .with_context(|| format!("Failed to enter workspace {}", dir.display()))?;
    let workspace = Workspace::open(&dir)
        .with_context(|| format!("Failed to open workspace {}", dir.display()))?;
    app.manage(Mutex::new(workspace));
    Ok(())
}

/// Writes the workspace settings (auto-start patches, global configs).
pub fn save_workspace(app: &AppHandle) {
    let workspace = app.state::<Mutex<Workspace>>();
    if let Err(e) = workspace.lock().save() {
        log::error!("Failed to save workspace settings: {}", e);
    }
}

/// Applies the workspace's global configs. Entries that cannot be applied
/// (unknown module or key, unset variable) are logged and skipped: settings
/// written from the UI can keep stale keys after a module is renamed.
pub fn apply_workspace_global_configs(app: &AppHandle) {
    let workspace = app.state::<Mutex<Workspace>>();
    for warning in workspace.lock().apply_global_configs(app.ma()) {
        log::warn!("Workspace settings: {}", warning);
    }
}

// MCP Server

/// The running built-in MCP server, if any. The async Mutex is held across the
/// whole stop/start sequence so concurrent settings changes cannot interleave
/// and orphan a running server (neither part stops on drop).
#[derive(Default)]
pub struct McpServerState(tokio::sync::Mutex<Option<RunningMcpServer>>);

struct RunningMcpServer {
    handle: McpServerHandle,
    shutdown: CancellationToken,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl RunningMcpServer {
    async fn stop(self) {
        // End the MCP sessions first: an open SSE stream would otherwise keep
        // the graceful shutdown waiting.
        self.handle.stop().await;
        self.shutdown.cancel();
        match self.server.await {
            Ok(Err(e)) => log::error!("MCP server error: {}", e),
            Err(e) => log::warn!("MCP server task join error: {}", e),
            Ok(Ok(())) => {}
        }
    }
}

fn generate_token() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Start the MCP server on app launch if enabled in settings.
pub async fn init_mcp_server(app: &AppHandle) {
    if let Err(e) = apply_mcp_server(app).await {
        log::error!("Failed to start MCP server: {}", e);
    }
}

/// Apply the current MCP server settings at runtime. Reads enabled/port/token
/// after stopping the old server so the last serialized caller always applies
/// the freshest settings, regardless of how concurrent calls interleaved.
/// Generates and persists a token when the server is enabled without one, so
/// the frontend only ever reads the token and never has to send it back.
pub async fn apply_mcp_server(app: &AppHandle) -> Result<()> {
    let state = app.state::<McpServerState>();
    let mut guard = state.0.lock().await;

    if let Some(server) = guard.take() {
        server.stop().await;
        log::info!("MCP server stopped");
    }

    let (enabled, port, mut token) = {
        let settings = app.state::<Mutex<CoreSettings>>();
        let settings = settings.lock();
        (
            settings.mcp_server_enabled,
            settings.mcp_server_port,
            settings.mcp_server_token.clone(),
        )
    };

    if enabled && token.is_none() {
        let new_token = generate_token();
        {
            let settings = app.state::<Mutex<CoreSettings>>();
            let mut settings = settings.lock();
            settings.mcp_server_token = Some(new_token.clone());
        }
        save(app)?;
        token = Some(new_token);
    }

    if enabled {
        let patches_dir = crate::modular_agent_desktop::app::patches_dir().ok();
        let config = McpServerConfig {
            patches_dir,
            token,
            ..Default::default()
        };
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .await
            .with_context(|| format!("Failed to bind MCP server to port {port}"))?;
        let (router, handle) = mcp_router(app.ma().clone(), config);
        let shutdown = CancellationToken::new();
        let server = tokio::spawn(
            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown.clone().cancelled_owned())
                .into_future(),
        );
        *guard = Some(RunningMcpServer {
            handle,
            shutdown,
            server,
        });
        log::info!("MCP server started on http://127.0.0.1:{}/mcp", port);
    }

    Ok(())
}

fn json_merge(a: &mut JsonValue, b: JsonValue) {
    if let JsonValue::Object(a) = a {
        if let JsonValue::Object(b) = b {
            for (k, v) in b {
                if v.is_null() {
                    a.remove(&k);
                } else {
                    json_merge(a.entry(k).or_insert(JsonValue::Null), v);
                }
            }
            return;
        }
    }
    *a = b;
}

#[tauri::command]
pub fn get_core_settings_cmd(settings: State<Mutex<CoreSettings>>) -> Result<JsonValue, String> {
    let settings = settings.lock();
    let json = serde_json::to_value(&*settings).map_err(|e| e.to_string())?;
    Ok(json)
}

#[tauri::command]
pub async fn set_core_settings_cmd(
    app: AppHandle,
    settings: State<'_, Mutex<CoreSettings>>,
    mut new_settings: JsonValue,
) -> Result<(), String> {
    if new_settings.is_null() {
        return Ok(());
    }

    // The MCP server token is only writable through backend generation
    // (apply_mcp_server / regenerate_mcp_server_token_cmd). Drop any echoed
    // token so a stale frontend settings snapshot cannot revert a regenerated
    // token.
    if let Some(obj) = new_settings.as_object_mut() {
        obj.remove("mcp_server_token");
    }

    // Capture old runtime-applied values before merge
    let (old_autostart, old_mcp) = {
        let current = settings.lock();
        (
            current.autostart,
            (current.mcp_server_enabled, current.mcp_server_port),
        )
    };

    // Merge new settings into existing settings
    if new_settings.is_object() {
        let mut settings = settings.lock();
        let mut value = serde_json::to_value(&*settings)
            .map_err(|e| format!("Failed to serialize current settings: {}", e))?;
        json_merge(&mut value, new_settings);
        *settings = serde_json::from_value(value)
            .map_err(|e| format!("Failed to deserialize new settings: {}", e))?;
    } else {
        return Err("Invalid settings format".to_string());
    }

    save(&app).map_err(|e| e.to_string())?;

    // Apply autostart and MCP server changes at runtime (after lock is released)
    let (new_autostart, new_mcp) = {
        let current = settings.lock();
        (
            current.autostart,
            (current.mcp_server_enabled, current.mcp_server_port),
        )
    };
    if old_autostart != new_autostart {
        crate::modular_agent_desktop::autostart::apply(&app, new_autostart)
            .map_err(|e| format!("Failed to apply autostart setting: {}", e))?;
    }
    if old_mcp != new_mcp {
        // The settings themselves are already saved at this point; make that
        // clear so a bind failure isn't reported as a failed save.
        apply_mcp_server(&app).await.map_err(|e| {
            format!(
                "Settings saved, but applying the MCP server setting failed: {}",
                e
            )
        })?;
    }

    Ok(())
}

/// Generate a fresh MCP server token, persist it, and restart the server with
/// it if enabled. Returns the new token; any previous token stops working
/// immediately.
#[tauri::command]
pub async fn regenerate_mcp_server_token_cmd(
    app: AppHandle,
    settings: State<'_, Mutex<CoreSettings>>,
) -> Result<String, String> {
    let token = generate_token();
    {
        let mut settings = settings.lock();
        settings.mcp_server_token = Some(token.clone());
    }
    save(&app).map_err(|e| e.to_string())?;

    apply_mcp_server(&app).await.map_err(|e| {
        format!(
            "Token saved, but applying the MCP server setting failed: {}",
            e
        )
    })?;

    Ok(token)
}

// Global Module Configs

type RawConfigs = IndexMap<String, JsonValue>;

/// Global configs as the settings page edits them: each definition's
/// defaults overlaid with the workspace values as written, so `${VAR}`
/// references show instead of the secrets they expand to.
#[tauri::command]
pub(crate) fn get_global_configs_cmd(
    app: AppHandle,
) -> Result<IndexMap<String, RawConfigs>, String> {
    let mut map = IndexMap::new();
    for (def_name, configs) in app.ma().get_global_configs_map() {
        let json = serde_json::to_value(configs).map_err(|e| e.to_string())?;
        let configs: RawConfigs = serde_json::from_value(json).map_err(|e| e.to_string())?;
        map.insert(def_name, configs);
    }
    let workspace = app.state::<Mutex<Workspace>>();
    for (def_name, raw) in &workspace.lock().settings.modules {
        if let Some(configs) = map.get_mut(def_name) {
            for (key, value) in raw {
                if configs.contains_key(key) {
                    configs.insert(key.clone(), value.clone());
                }
            }
        }
    }
    Ok(map)
}

/// Stores the configs as written (unexpanded) and re-applies the workspace.
#[tauri::command]
pub(crate) fn set_global_configs_cmd(
    app: AppHandle,
    def_name: String,
    configs: RawConfigs,
) -> Result<(), String> {
    {
        let workspace = app.state::<Mutex<Workspace>>();
        let mut workspace = workspace.lock();
        workspace.settings.modules.insert(def_name, configs);
        workspace.save().map_err(|e| e.to_string())?;
    }
    apply_workspace_global_configs(&app);
    Ok(())
}
