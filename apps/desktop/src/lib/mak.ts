import { invoke } from "@tauri-apps/api/core";

import type { AgentConfigs } from "tauri-plugin-modular-agent-api";

import type { CoreSettings } from "./types";

// Tauri

const isEdge = typeof navigator !== "undefined" && navigator.userAgent?.includes("Edg");

// app

export async function exitApp(): Promise<void> {
  await invoke("exit_app_cmd");
}

// settings

export async function getCoreSettings(): Promise<CoreSettings> {
  return await invoke("get_core_settings_cmd");
}

export async function setCoreSettings(newSettings: Partial<CoreSettings>): Promise<void> {
  await invoke("set_core_settings_cmd", { newSettings });
}

export async function setGlobalConfigs(defName: string, configs: AgentConfigs): Promise<void> {
  await invoke("set_global_configs_cmd", { defName, configs });
}

// utilities

export function truncate(str: string, maxLength: number, suffix: string = "..."): string {
  if (str.length <= maxLength) {
    return str;
  }
  return str.slice(0, maxLength) + suffix;
}

export async function getDirEntries(path: string): Promise<string[]> {
  return await invoke("get_dir_entries_cmd", { path });
}

export async function openPreset(name: string): Promise<string> {
  return await invoke("open_preset_cmd", { name });
}
