import { getAgentDefinitions, getAgentStreams, getGlobalConfigsMap } from "tauri-plugin-askit-api";
import type { AgentDefinitions } from "tauri-plugin-askit-api";

import { deserializeAgentStream } from "@/lib/agent";
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

  const sAgentStreams = await getAgentStreams();
  const AgentStreams = Object.fromEntries(
    Object.entries(sAgentStreams).map(([key, stream]) => [key, deserializeAgentStream(stream)]),
  );

  return {
    coreSettings,
    agentDefs,
    agentGlobalConfigsMap,
    AgentStreams,
  };
}
