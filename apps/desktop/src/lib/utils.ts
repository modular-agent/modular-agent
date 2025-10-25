import { invoke } from "@tauri-apps/api/core";

import type { CoreSettings, AgentConfig, AgentConfigs } from "tauri-plugin-askit-api";

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

// export async function getAgentGlobalConfigs(): Promise<AgentConfigs> {
//   return await invoke("get_agent_global_configs_cmd");
// }

// export async function setAgentGlobalConfig(
//   agentName: string,
//   agentConfig: AgentConfig,
// ): Promise<void> {
//   await invoke("set_agent_global_config_cmd", { agentName, agentConfig });
// }
