use std::sync::Mutex;

use anyhow::Result;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

use crate::modular_agent_desktop::settings::CoreSettings;

pub fn init(app: &AppHandle) -> Result<()> {
    let setting = app.state::<Mutex<CoreSettings>>();
    let is_autostart;
    {
        let setting = setting.lock().unwrap();
        is_autostart = setting.autostart;
    }

    app.plugin(tauri_plugin_autostart::init(
        MacosLauncher::LaunchAgent,
        None,
    ))?;

    apply(app, is_autostart)
}

/// Apply autostart setting at runtime. Called from init() and set_core_settings_cmd().
/// Takes `enabled` as argument instead of reading from Mutex to avoid deadlock.
pub fn apply(app: &AppHandle, enabled: bool) -> Result<()> {
    let autostart_manager = app.autolaunch();

    if enabled {
        if autostart_manager.is_enabled()? {
            log::debug!("Autostart is already enabled");
        } else {
            log::info!("Enable autostart");
            autostart_manager.enable()?;
        }
    } else {
        if autostart_manager.is_enabled()? {
            log::info!("Disable autostart");
            autostart_manager.disable()?;
        } else {
            log::debug!("Autostart is already disabled");
        }
    }

    Ok(())
}
