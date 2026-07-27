import { listen } from "@tauri-apps/api/event";

import { tabStore } from "$lib/tab-store.svelte";

import type {
  AgentConfigUpdatedMessage,
  AgentErrorMessage,
  AgentInMessage,
  AgentSpecUpdatedMessage,
  PresetRemovedMessage,
  PresetStructureChangedMessage,
} from "./types";

// Origin convention: "desktop" is our own echo (ignore); "mcp" and null
// (agent runtime internal) are external and must be reflected.
const SELF_ORIGIN = "desktop";
export function isExternalOrigin(origin: string | null | undefined): boolean {
  return origin !== SELF_ORIGIN; // null counts as external
}

let eventSeq = 0;

export type AgentEventState = {
  configUpdated: { key: string; value: any; seq: number };
  error: { message: string; seq: number };
  input: { port: string; seq: number };
  specUpdated: number;
};

function defaultAgentEvent(): AgentEventState {
  return {
    configUpdated: { key: "", value: null, seq: 0 },
    error: { message: "", seq: 0 },
    input: { port: "", seq: 0 },
    specUpdated: 0,
  };
}

class SharedAgentEvents {
  agents = $state<Record<string, AgentEventState>>({});

  // Creates entry if not exists. Only call from agent-node components, not from Tauri listeners.
  getAgent(id: string): AgentEventState {
    if (!this.agents[id]) {
      this.agents[id] = defaultAgentEvent();
    }
    return this.agents[id];
  }

  removeAgent(id: string) {
    delete this.agents[id];
  }
}

export const sharedAgentEvents = new SharedAgentEvents();

class SharedPresetEvents {
  // presetId → latest seq of an externally-originated structure change
  structureChanged = $state<Record<string, number>>({});
}

export const sharedPresetEvents = new SharedPresetEvents();

// Tauri event listeners (module-level, live for the app's lifetime)
$effect.root(() => {
  listen<AgentConfigUpdatedMessage>("ma:agent_config_updated", (event) => {
    if (!isExternalOrigin(event.payload.origin)) return;
    const { agent_id, key, value } = event.payload;
    const agent = sharedAgentEvents.agents[agent_id];
    if (!agent) return;
    // Note: `agents` is deeply reactive, so consumers reading `configUpdated.value`
    // get a $state proxy regardless of what is assigned here. Consumers that copy
    // the value into non-reactive storage ($state.raw nodes) must snapshot it.
    agent.configUpdated = { key, value, seq: ++eventSeq };
  });

  listen<AgentErrorMessage>("ma:agent_error", (event) => {
    const { agent_id, message } = event.payload;
    const agent = sharedAgentEvents.agents[agent_id];
    if (!agent) return;
    agent.error = { message, seq: ++eventSeq };
  });

  listen<AgentInMessage>("ma:agent_in", (event) => {
    const { agent_id, port } = event.payload;
    const agent = sharedAgentEvents.agents[agent_id];
    if (!agent) return;
    agent.input = { port, seq: ++eventSeq };
  });

  listen<AgentSpecUpdatedMessage>("ma:agent_spec_updated", (event) => {
    if (!isExternalOrigin(event.payload.origin)) return;
    const { agent_id } = event.payload;
    const agent = sharedAgentEvents.agents[agent_id];
    if (!agent) return;
    agent.specUpdated = ++eventSeq;
  });

  listen<PresetStructureChangedMessage>("ma:preset_structure_changed", (event) => {
    if (!isExternalOrigin(event.payload.origin)) return;
    const { preset_id } = event.payload;
    sharedPresetEvents.structureChanged[preset_id] = ++eventSeq;
  });

  listen<PresetRemovedMessage>("ma:preset_removed", (event) => {
    if (!isExternalOrigin(event.payload.origin)) return;
    const { preset_id } = event.payload;
    // Closing the tab triggers editor-host's cleanup, which unloads the
    // (already removed) preset from the backend idempotently.
    if (tabStore.tabs.find((t) => t.id === preset_id)) {
      tabStore.closeTab(preset_id);
    }
  });
});
