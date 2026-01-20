#![recursion_limit = "256"]

use modular_agent_kit::MAK;
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, RunEvent, Runtime,
};

mod commands;
mod error;

pub use error::{Error, Result};

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the mak APIs.
pub trait MAKExt<R: Runtime> {
    fn mak(&self) -> &MAK;
}

impl<R: Runtime, T: Manager<R>> crate::MAKExt<R> for T {
    fn mak(&self) -> &MAK {
        self.state::<MAK>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("mak")
        .invoke_handler(tauri::generate_handler![
            commands::get_agent_definition,
            commands::get_agent_definitions,
            commands::get_agent_spec,
            commands::update_agent_spec,
            commands::get_preset_info,
            commands::get_preset_infos,
            commands::get_preset_spec,
            commands::update_preset_spec,
            commands::new_preset,
            commands::rename_preset,
            commands::unique_preset_name,
            commands::add_preset,
            commands::remove_preset,
            commands::add_agents_and_connections,
            commands::start_preset,
            commands::stop_preset,
            commands::new_agent_spec,
            commands::add_agent,
            commands::remove_agent,
            commands::add_connection,
            commands::remove_connection,
            commands::start_agent,
            commands::stop_agent,
            commands::write_board,
            commands::set_agent_configs,
            commands::get_global_configs,
            commands::get_global_configs_map,
            commands::set_global_configs,
            commands::set_global_configs_map,
        ])
        .setup(|app, _api| {
            let mak = MAK::init()?;
            app.manage(mak);
            Ok(())
        })
        .on_event(|app, event| match event {
            RunEvent::Ready => {
                tauri::async_runtime::block_on(async move {
                    let mak = app.state::<MAK>();
                    mak.ready().await.unwrap();
                });
            }
            RunEvent::Exit => {
                let mak = app.state::<MAK>();
                mak.quit();
            }
            _ => {}
        })
        .build()
}
