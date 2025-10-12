use agent_stream_kit::AgentData;
use tauri::{AppHandle, Runtime};

use crate::Result;
use crate::ASKitExt;

#[tauri::command]
pub(crate) fn write_board<R: Runtime>(
    app: AppHandle<R>,
    board: String,
    message: String,
) -> Result<()> {
    app.askit().write_board_data(board, AgentData::string(message)).map_err(Into::into)
}
