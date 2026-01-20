use modular_agent_kit::{AgentConfigs, AgentValue};
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    ops::Not,
    sync::{LazyLock, Mutex},
};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_mak::MAKExt;
use tauri_plugin_store::StoreExt;

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

    let agent_settings = app.mak().get_global_configs_map();
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
        let mut global_configs_map = app.mak().get_global_configs_map();
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
        app.mak().set_global_configs_map(global_configs_map);
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

// Global Agent Configs

#[tauri::command]
pub(crate) fn set_global_configs_cmd(
    app: AppHandle,
    def_name: String,
    configs: AgentConfigs,
) -> Result<(), String> {
    app.mak().set_global_configs(def_name, configs);

    save(&app).map_err(|e| e.to_string())?;

    Ok(())
}
