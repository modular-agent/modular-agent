use tauri::{
  plugin::{Builder, TauriPlugin},
  Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Askit;
#[cfg(mobile)]
use mobile::Askit;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the askit APIs.
pub trait AskitExt<R: Runtime> {
  fn askit(&self) -> &Askit<R>;
}

impl<R: Runtime, T: Manager<R>> crate::AskitExt<R> for T {
  fn askit(&self) -> &Askit<R> {
    self.state::<Askit<R>>().inner()
  }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
  Builder::new("askit")
    .invoke_handler(tauri::generate_handler![commands::ping])
    .setup(|app, api| {
      #[cfg(mobile)]
      let askit = mobile::init(app, api)?;
      #[cfg(desktop)]
      let askit = desktop::init(app, api)?;
      app.manage(askit);
      Ok(())
    })
    .build()
}
