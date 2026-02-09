import { listen } from "@tauri-apps/api/event";

import type {
  AgentConfigUpdatedMessage,
  AgentErrorMessage,
  AgentInMessage,
  AgentSpecUpdatedMessage,
} from "./types";

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

// Tauri event listeners (module-level, live for the app's lifetime)
$effect.root(() => {
  listen<AgentConfigUpdatedMessage>("ma:agent_config_updated", (event) => {
    const { agent_id, key, value } = event.payload;
    const agent = sharedAgentEvents.agents[agent_id];
    if (!agent) return;
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
    const { agent_id } = event.payload;
    const agent = sharedAgentEvents.agents[agent_id];
    if (!agent) return;
    agent.specUpdated = ++eventSeq;
  });
});
