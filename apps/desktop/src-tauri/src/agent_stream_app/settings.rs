use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_store::StoreExt;

use super::app::ASApp;

const SETTINGS_JSON: &str = "settings.json";

pub fn init(app: &AppHandle) -> Result<()> {
    init_core_settings(app)?;
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

    let asapp = app.state::<ASApp>();
    let agent_settings = asapp.get_global_configs();
    let agent_settings_json = serde_json::to_value(agent_settings)?;
    store.set("agents", agent_settings_json);

    Ok(())
}

pub fn quit(_app: &AppHandle) {
    // save(app);
}

// core settings

#[derive(Debug, Serialize, Deserialize)]
pub struct CoreSettings {
    pub autostart: Option<bool>,
    pub shortcut_keys: Option<HashMap<String, String>>,
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
            map.insert("screenshot_only".into(), " ".into());
            map.insert("search".into(), "Ctrl+K, Command+K".into());
            map
        });

        CoreSettings {
            autostart: Some(false),
            shortcut_keys: Some(SHORTCUT_KEYS.clone()),
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
pub fn set_core_settings_cmd(
    app: AppHandle,
    settings: State<Mutex<CoreSettings>>,
    new_settings: Value,
) -> Result<(), String> {
    if new_settings.is_null() {
        return Ok(());
    }

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

    Ok(())
}

pub fn set_agent_global_configs(app: &AppHandle) -> Result<()> {
    let store = app.store(SETTINGS_JSON)?;
    if let Some(value) = store.get("agents") {
        let agent_global_configs = serde_json::from_value(value).unwrap_or_default();
        let asapp = app.state::<ASApp>();
        asapp.set_global_configs(agent_global_configs);
    }
    Ok(())
}

#[tauri::command]
pub fn get_agent_global_configs_cmd(app: AppHandle) -> Result<Value, String> {
    let asapp = app.state::<ASApp>();
    let global_configs = asapp.get_global_configs();
    let json = serde_json::to_value(&global_configs).map_err(|e| e.to_string())?;
    return Ok(json);
}

#[tauri::command]
pub fn set_agent_global_config_cmd(
    app: AppHandle,
    agent_name: String,
    agent_config: Value,
) -> Result<(), String> {
    let asapp = app.state::<ASApp>();
    asapp.set_global_config(agent_name, agent_config);
    save(&app).map_err(|e| e.to_string())?;
    Ok(())
}
