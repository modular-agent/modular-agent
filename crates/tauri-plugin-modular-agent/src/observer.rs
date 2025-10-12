use agent_stream_kit::{ASKitEvent, ASKitObserver};
use tauri::{AppHandle, Emitter, Runtime};

use crate::models::BoardMessage;

pub(crate) struct BoardObserver<R: Runtime> {
    pub app: AppHandle<R>,
}

impl<R: Runtime> ASKitObserver for BoardObserver<R> {
    fn notify(&self, event: &ASKitEvent) {
        match event {
            ASKitEvent::Board(name, data) => {
                self.app
                    .emit(
                        "plugin:askit|notify_board",
                        BoardMessage {
                            name: name.to_string(),
                            data: data.clone(),
                        },
                    )
                    .unwrap();
            }
            _ => {}
        }
    }
}
