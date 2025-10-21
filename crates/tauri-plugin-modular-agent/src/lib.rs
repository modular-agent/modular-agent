use agent_stream_kit::ASKit;
use tauri::{
  plugin::{Builder, TauriPlugin},
  Manager, Runtime, RunEvent,
};

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;
mod observer;

pub use error::{Error, Result};
pub use models::*;

use observer::BoardObserver;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the askit APIs.
pub trait ASKitExt<R: Runtime> {
  fn askit(&self) -> &ASKit;
}

impl<R: Runtime, T: Manager<R>> crate::ASKitExt<R> for T {
  fn askit(&self) -> &ASKit {
    self.state::<ASKit>().inner()
  }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
  Builder::new("askit")
    .invoke_handler(tauri::generate_handler![commands::write_board])
    .setup(|app, api| {
      #[cfg(mobile)]
      let askit = mobile::init(app, api)?;
      #[cfg(desktop)]
      let askit = desktop::init(app, api)?;
      askit.subscribe(Box::new(BoardObserver {
          app: app.clone(),
      }));
      app.manage(askit);
      Ok(())
    })
    .on_event(|app, event| {
      match event {
        RunEvent::Ready => {
          tauri::async_runtime::block_on(async move {
            let askit = app.state::<ASKit>();
            askit.ready().await.unwrap();
          });
        }
        RunEvent::Exit => {
          let askit = app.state::<ASKit>();
          askit.quit();
        }
        _ => {}
      }
    })
    .build()
}
