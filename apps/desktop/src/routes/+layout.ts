import { getAgentDefinitions, getAgentFlows, getGlobalConfigsMap } from "tauri-plugin-askit-api";
import type { AgentDefinitions } from "tauri-plugin-askit-api";

import { deserializeAgentFlow } from "@/lib/agent";
import { getCoreSettings } from "@/lib/utils";

// Tauri doesn't have a Node.js server to do proper SSR
// so we will use adapter-static to prerender the app (SSG)
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
export const prerender = true;
export const ssr = false;

export async function load() {
  const coreSettings = await getCoreSettings();

  const agentDefs: AgentDefinitions = await getAgentDefinitions();
  const agentGlobalConfigsMap = await getGlobalConfigsMap();

  const sAgentFlows = await getAgentFlows();
  const agentFlows = Object.fromEntries(
    Object.entries(sAgentFlows).map(([key, flow]) => [key, deserializeAgentFlow(flow)]),
  );

  return {
    // dailyStats,
    coreSettings,
    agentDefs,
    agentGlobalConfigsMap,
    agentFlows,
  };
}
