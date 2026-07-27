#![recursion_limit = "256"]

use modular_agent_core::ModularAgent;
use tauri::{
    Manager, RunEvent, Runtime,
    plugin::{Builder, TauriPlugin},
};

mod commands;
mod error;

pub use error::{Error, Result};

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the ma APIs.
pub trait ModularAgentExt<R: Runtime> {
    fn ma(&self) -> &ModularAgent;
}

impl<R: Runtime, T: Manager<R>> crate::ModularAgentExt<R> for T {
    fn ma(&self) -> &ModularAgent {
        self.state::<ModularAgent>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("modular-agent")
        .invoke_handler(tauri::generate_handler![
            // Preset management
            commands::new_preset,
            commands::add_preset,
            commands::add_preset_with_name,
            commands::remove_preset,
            commands::start_preset,
            commands::stop_preset,
            commands::open_preset_from_file,
            commands::save_preset,
            commands::get_preset_spec,
            commands::update_preset_spec,
            commands::get_preset_info,
            commands::get_preset_infos,
            // Agent management
            commands::get_agent_definition,
            commands::get_agent_definitions,
            commands::get_agent_spec,
            commands::update_agent_spec,
            commands::new_agent_spec,
            commands::add_agent,
            commands::remove_agent,
            commands::add_connection,
            commands::remove_connection,
            commands::add_agents_and_connections,
            commands::start_agent,
            commands::stop_agent,
            commands::set_agent_configs,
            commands::get_global_configs,
            commands::get_global_configs_map,
            commands::set_global_configs,
            commands::set_global_configs_map,
            commands::write_external_input,
        ])
        .setup(|app, _api| {
            let ma = ModularAgent::init()?.with_origin("desktop");
            app.manage(ma);
            Ok(())
        })
        .on_event(|app, event| match event {
            RunEvent::Ready => {
                tauri::async_runtime::block_on(async move {
                    let ma = app.state::<ModularAgent>();
                    ma.ready().await.unwrap();
                });
            }
            RunEvent::Exit => {
                let ma = app.state::<ModularAgent>();
                ma.quit();
            }
            _ => {}
        })
        .build()
}
