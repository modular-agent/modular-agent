#![recursion_limit = "256"]

use tauri::{AppHandle, Manager};
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

#[allow(unused_imports)]
use mak_cozodb_agents;

#[allow(unused_imports)]
use mak_lancedb_agents;

#[allow(unused_imports)]
use mak_lifelog_agents;

#[allow(unused_imports)]
use mak_llm_agents;

#[allow(unused_imports)]
use mak_std_agents;

#[allow(unused_imports)]
use mak_web_agents;

mod mak_desktop;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .level_for(
                    "mak_desktop_lib",
                    if cfg!(debug_assertions) {
                        log::LevelFilter::Debug
                    } else {
                        log::LevelFilter::Info
                    },
                )
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_mak::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log::info!("show main window");
            mak_desktop::window::show_main(app).unwrap_or_else(|e| {
                log::error!("Failed to show main window: {}", e);
            });
        }))
        .setup(|app| {
            let app_handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                mak_desktop::settings::init(&app_handle).unwrap_or_else(|e| {
                    panic!("Failed to initialize settings: {}", e);
                });
                mak_desktop::tray::init(&app_handle).unwrap_or_else(|e| {
                    log::error!("Failed to initialize tray: {}", e);
                    app_handle.exit(1);
                });
                mak_desktop::app::init(&app_handle).unwrap_or_else(|e| {
                    log::error!("Failed to initialize agent: {}", e);
                    app_handle.exit(1);
                });
                mak_desktop::settings::load_agent_global_configs(&app_handle).unwrap_or_else(|e| {
                    log::error!("Failed to load agent global configs: {}", e);
                    app_handle.exit(1);
                });
                mak_desktop::autostart::init(&app_handle).unwrap_or_else(|e| {
                    log::error!("Failed to initialize autostart: {}", e);
                });
                mak_desktop::shortcut::init(&app_handle).unwrap_or_else(|e| {
                    log::error!("Failed to initialize shortcut: {}", e);
                });

                let app_handle2 = app_handle.clone();
                ctrlc::set_handler(move || {
                    app_handle2.exit(0);
                })
                .unwrap_or_else(|e| {
                    log::error!("Failed to set ctrl-c handler: {}", e);
                    app_handle.exit(1);
                });
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            exit_app_cmd,
            mak_desktop::app::new_preset_cmd,
            mak_desktop::app::rename_preset_cmd,
            mak_desktop::app::remove_preset_cmd,
            mak_desktop::app::import_preset_cmd,
            mak_desktop::app::save_preset_cmd,
            mak_desktop::app::start_preset_cmd,
            mak_desktop::app::stop_preset_cmd,
            mak_desktop::settings::get_core_settings_cmd,
            mak_desktop::settings::set_core_settings_cmd,
            mak_desktop::settings::set_global_configs_cmd,
        ])
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                #[cfg(not(target_os = "macos"))]
                {
                    window.hide().unwrap();
                }
                #[cfg(target_os = "macos")]
                {
                    tauri::AppHandle::hide(window.app_handle()).unwrap();
                }
                api.prevent_close();

                let run_in_background = {
                    let app = window.app_handle();
                    let core_settings =
                        app.state::<std::sync::Mutex<mak_desktop::settings::CoreSettings>>();
                    let guard = core_settings.lock().unwrap();
                    guard.run_in_background
                };
                if !run_in_background {
                    log::info!("Exiting MAK Desktop...");
                    window.app_handle().exit(0);
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            tauri::RunEvent::Ready => {
                tauri::async_runtime::block_on(async move {
                    mak_desktop::app::ready(app).await.unwrap_or_else(|e| {
                        log::error!("Failed to start agents: {}", e);
                    });
                    log::info!("MAK Desktop is ready.");
                });
            }
            tauri::RunEvent::Exit => {
                log::info!("Exiting MAK Desktop...");
                tauri::async_runtime::block_on(async move {
                    mak_desktop::window::hide_main(app).unwrap_or_else(|e| {
                        log::error!("Failed to hide main window: {}", e);
                    });
                    app.save_window_state(StateFlags::all())
                        .unwrap_or_else(|e| {
                            log::error!("Failed to save window state: {}", e);
                        });
                    mak_desktop::app::quit(app);
                    mak_desktop::settings::quit(app);
                });
            }
            _ => {}
        });
}

#[tauri::command]
fn exit_app_cmd(app: AppHandle) -> Result<(), String> {
    // The application will not exit immediately;
    // the Exit event processing above will be executed.
    app.exit(0);
    Ok(())
}
