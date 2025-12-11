use agent_stream_kit::{ASKitEvent, ASKitObserver};
use tauri::{AppHandle, Emitter, Runtime};

use crate::models::BoardMessage;

pub(crate) struct BoardObserver<R: Runtime> {
    pub app: AppHandle<R>,
}

impl<R: Runtime> ASKitObserver for BoardObserver<R> {
    fn notify(&self, event: &ASKitEvent) {
        match event {
            ASKitEvent::Board(name, value) => {
                self.app
                    .emit(
                        "notify_board",
                        BoardMessage {
                            name: name.to_string(),
                            value: value.clone(),
                        },
                    )
                    .unwrap_or_else(|e| {
                        eprintln!("Failed to emit board event: {}", e);
                    });
            }
            _ => {}
        }
    }
}
