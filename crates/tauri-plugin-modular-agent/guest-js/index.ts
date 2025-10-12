import { invoke } from '@tauri-apps/api/core'

export async function writeBoard(board: string, message: string): Promise<void> {
  await invoke<{value?: string}>('plugin:askit|write_board', { board, message })
}
