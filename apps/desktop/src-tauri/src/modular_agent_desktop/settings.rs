use anyhow::{Context as _, Result};
use modular_agent_core::mcp_server::{start_mcp_server, McpServerConfig, McpServerHandle};
use modular_agent_core::{AgentConfigs, AgentValue};
use rand::Rng as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    ops::Not,
    sync::{LazyLock, Mutex},
};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_modular_agent::ModularAgentExt;
use tauri_plugin_store::StoreExt;

const SETTINGS_JSON: &str = "settings.json";

pub fn init(app: &AppHandle) -> Result<()> {
    init_core_settings(app)?;
    app.manage(McpServerState::default());
    Ok(())
}

pub fn save(app: &AppHandle) -> Result<()> {
    let store = app.store(SETTINGS_JSON)?;

    let core_settings = app.state::<Mutex<CoreSettings>>();
    let settings_json;
    {
        let core_settings = core_settings.lock().unwrap();
        settings_json = serde_json::to_value(&*core_settings)?;
    }
    store.set("core", settings_json);

    let agent_settings = app.ma().get_global_configs_map();
    let agent_settings_json = serde_json::to_value(agent_settings)?;
    store.set("agents", agent_settings_json);

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

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto_start_presets: Vec<String>,

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
    pub show_grid: Option<bool>,

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
            auto_start_presets: Vec::new(),
            color_mode: None,
            run_in_background: false,
            shortcut_keys: Some(SHORTCUT_KEYS.clone()),
            snap_enabled: None,
            snap_grid_size: None,
            show_grid: None,
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

pub fn load_agent_global_configs(app: &AppHandle) -> Result<()> {
    let store = app.store(SETTINGS_JSON)?;

    if let Some(store_value) = store.get("agents") {
        let mut global_configs_map = app.ma().get_global_configs_map();
        for (agent_name, configs) in store_value.as_object().unwrap_or(&Default::default()) {
            if let Some(agent_configs) = global_configs_map.get_mut(agent_name) {
                for (key, value) in configs.as_object().unwrap_or(&Default::default()) {
                    if agent_configs.contains_key(key) {
                        if let Ok(value) = AgentValue::from_json(value.clone()) {
                            agent_configs.set(key.clone(), value);
                        }
                    }
                }
            }
        }
        app.ma().set_global_configs_map(global_configs_map);
    }

    Ok(())
}

// MCP Server

/// Handle of the running built-in MCP server, if any. The async Mutex is held
/// across the whole stop/start sequence so concurrent settings changes cannot
/// interleave and orphan a running server (McpServerHandle does not stop on
/// drop).
#[derive(Default)]
pub struct McpServerState(tokio::sync::Mutex<Option<McpServerHandle>>);

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

    if let Some(handle) = guard.take() {
        handle.stop().await;
        log::info!("MCP server stopped");
    }

    let (enabled, port, mut token) = {
        let settings = app.state::<Mutex<CoreSettings>>();
        let settings = settings.lock().unwrap();
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
            let mut settings = settings.lock().unwrap();
            settings.mcp_server_token = Some(new_token.clone());
        }
        save(app)?;
        token = Some(new_token);
    }

    if enabled {
        let presets_dir = crate::modular_agent_desktop::app::presets_dir().ok();
        let config = McpServerConfig {
            port,
            presets_dir,
            token,
        };
        let handle = start_mcp_server(app.ma().clone(), config).await?;
        *guard = Some(handle);
        log::info!("MCP server started on http://127.0.0.1:{}/mcp", port);
    }

    Ok(())
}

fn json_merge(a: &mut Value, b: Value) {
    if let Value::Object(a) = a {
        if let Value::Object(b) = b {
            for (k, v) in b {
                if v.is_null() {
                    a.remove(&k);
                } else {
                    json_merge(a.entry(k).or_insert(Value::Null), v);
                }
            }
            return;
        }
    }
    *a = b;
}

#[tauri::command]
pub fn get_core_settings_cmd(settings: State<Mutex<CoreSettings>>) -> Result<Value, String> {
    let settings = settings.lock().unwrap();
    let json = serde_json::to_value(&*settings).map_err(|e| e.to_string())?;
    Ok(json)
}

#[tauri::command]
pub async fn set_core_settings_cmd(
    app: AppHandle,
    settings: State<'_, Mutex<CoreSettings>>,
    mut new_settings: Value,
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
        let current = settings.lock().unwrap();
        (
            current.autostart,
            (current.mcp_server_enabled, current.mcp_server_port),
        )
    };

    // Merge new settings into existing settings
    if new_settings.is_object() {
        let mut settings = settings.lock().unwrap();
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
        let current = settings.lock().unwrap();
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
        let mut settings = settings.lock().unwrap();
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

// Global Agent Configs

#[tauri::command]
pub(crate) fn set_global_configs_cmd(
    app: AppHandle,
    def_name: String,
    configs: AgentConfigs,
) -> Result<(), String> {
    app.ma().set_global_configs(def_name, configs);

    save(&app).map_err(|e| e.to_string())?;

    Ok(())
}
