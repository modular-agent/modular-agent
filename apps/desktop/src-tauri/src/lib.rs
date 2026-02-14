#![recursion_limit = "256"]

use tauri::{AppHandle, Manager};
use tauri_plugin_log::{Target, TargetKind};
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

#[allow(unused_imports)]
use modular_agent_lifelog;

#[allow(unused_imports)]
use modular_agent_llm;

#[allow(unused_imports)]
use modular_agent_slack;

#[allow(unused_imports)]
use modular_agent_std;

#[allow(unused_imports)]
use modular_agent_sqlx;

#[allow(unused_imports)]
use modular_agent_web;

mod modular_agent_desktop;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                    Target::new(TargetKind::Webview),
                ])
                .level(log::LevelFilter::Info)
                .level_for(
                    "modular_agent_desktop_lib",
                    if cfg!(debug_assertions) {
                        log::LevelFilter::Debug
                    } else {
                        log::LevelFilter::Info
                    },
                )
                // Suppress known harmless tao warning on Windows:
                // "RedrawEventsCleared emitted without explicit MainEventsCleared"
                // See: https://github.com/tauri-apps/tao/issues/872
                .level_for(
                    "tao::platform_impl::platform::event_loop::runner",
                    log::LevelFilter::Error,
                )
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_modular_agent::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log::info!("show main window");
            modular_agent_desktop::window::show_main(app).unwrap_or_else(|e| {
                log::error!("Failed to show main window: {}", e);
            });
        }))
        .setup(|app| {
            #[cfg(not(target_os = "macos"))]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.set_decorations(false).unwrap_or_else(|e| {
                        log::error!("Failed to set decorations: {}", e);
                    });
                }
            }

            let app_handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                modular_agent_desktop::settings::init(&app_handle).unwrap_or_else(|e| {
                    panic!("Failed to initialize settings: {}", e);
                });
                modular_agent_desktop::tray::init(&app_handle).unwrap_or_else(|e| {
                    log::error!("Failed to initialize tray: {}", e);
                    app_handle.exit(1);
                });
                modular_agent_desktop::app::init(&app_handle).unwrap_or_else(|e| {
                    log::error!("Failed to initialize agent: {}", e);
                    app_handle.exit(1);
                });
                modular_agent_desktop::settings::load_agent_global_configs(&app_handle)
                    .unwrap_or_else(|e| {
                        log::error!("Failed to load agent global configs: {}", e);
                        app_handle.exit(1);
                    });
                modular_agent_desktop::autostart::init(&app_handle).unwrap_or_else(|e| {
                    log::error!("Failed to initialize autostart: {}", e);
                });
                modular_agent_desktop::shortcut::init(&app_handle).unwrap_or_else(|e| {
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
            modular_agent_desktop::app::new_preset_with_name_cmd,
            modular_agent_desktop::app::move_preset_cmd,
            modular_agent_desktop::app::move_folder_cmd,
            modular_agent_desktop::app::delete_preset_cmd,
            modular_agent_desktop::app::close_preset_cmd,
            modular_agent_desktop::app::import_preset_cmd,
            modular_agent_desktop::app::save_preset_cmd,
            modular_agent_desktop::app::start_preset_cmd,
            modular_agent_desktop::app::stop_preset_cmd,
            modular_agent_desktop::app::get_dir_entries_cmd,
            modular_agent_desktop::app::open_preset_cmd,
            modular_agent_desktop::settings::get_core_settings_cmd,
            modular_agent_desktop::settings::set_core_settings_cmd,
            modular_agent_desktop::settings::set_global_configs_cmd,
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
                    let core_settings = app
                        .state::<std::sync::Mutex<modular_agent_desktop::settings::CoreSettings>>();
                    let guard = core_settings.lock().unwrap();
                    guard.run_in_background
                };
                if !run_in_background {
                    log::info!("Exiting ModularAgent Desktop...");
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
                    modular_agent_desktop::app::ready(app)
                        .await
                        .unwrap_or_else(|e| {
                            log::error!("Failed to start agents: {}", e);
                        });
                    log::info!("Module Agent Desktop is ready.");
                });
            }
            tauri::RunEvent::Exit => {
                log::info!("Exiting Module Agent Desktop...");
                tauri::async_runtime::block_on(async move {
                    modular_agent_desktop::window::hide_main(app).unwrap_or_else(|e| {
                        log::error!("Failed to hide main window: {}", e);
                    });
                    app.save_window_state(StateFlags::all())
                        .unwrap_or_else(|e| {
                            log::error!("Failed to save window state: {}", e);
                        });
                    modular_agent_desktop::app::quit(app);
                    modular_agent_desktop::settings::quit(app);
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
