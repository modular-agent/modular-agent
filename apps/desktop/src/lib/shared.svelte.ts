import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { writable, type Writable } from "svelte/store";

import type {
  AgentConfigUpdatedMessage,
  AgentErrorMessage,
  AgentInMessage,
  AgentSpecUpdatedMessage,
} from "./types";

// Agent Config Updated Message

let agentConfigUpdatedMessageStore: Map<string, Writable<{ key: string; value: any }>> = new Map<
  string,
  Writable<{ key: string; value: any }>
>();

export function subscribeAgentConfigUpdatedMessage(
  agentId: string,
  callback: (message: { key: string; value: any }) => void,
): () => void {
  let store = agentConfigUpdatedMessageStore.get(agentId);
  if (!store) {
    store = writable({ key: "", value: null });
    agentConfigUpdatedMessageStore.set(agentId, store);
  }
  return store.subscribe(callback);
}

let unlistenAgentConfigUpdated: UnlistenFn | null = null;

// Agent Error Message
let agentErrorMessageStore: Map<string, Writable<string>> = new Map<string, Writable<string>>();

export function subscribeAgentErrorMessage(
  agentId: string,
  callback: (message: string) => void,
): () => void {
  let store = agentErrorMessageStore.get(agentId);
  if (!store) {
    store = writable("");
    agentErrorMessageStore.set(agentId, store);
  }
  return store.subscribe(callback);
}

let unlistenAgentError: UnlistenFn | null = null;

// Agent-in Message
let agentInMessageStore: Map<string, Writable<{ ch: string; t: number }>> = new Map<
  string,
  Writable<{ ch: string; t: number }>
>();

export function subscribeAgentInMessage(
  agentId: string,
  callback: (message: { ch: string; t: number }) => void,
): () => void {
  let store = agentInMessageStore.get(agentId);
  if (!store) {
    store = writable({ ch: "", t: 0 });
    agentInMessageStore.set(agentId, store);
  }
  return store.subscribe(callback);
}

let unlistenAgentIn: UnlistenFn | null = null;

// Agent Spec Updated

let agentSpecUpdatedMessageStore: Map<string, Writable<any>> = new Map<string, Writable<any>>();

export function subscribeAgentSpecUpdatedMessage(
  agentId: string,
  callback: () => void,
): () => void {
  let store = agentSpecUpdatedMessageStore.get(agentId);
  if (!store) {
    store = writable(null);
    agentSpecUpdatedMessageStore.set(agentId, store);
  }
  return store.subscribe(callback);
}

let unlistenAgentSpecUpdated: UnlistenFn | null = null;

//

$effect.root(() => {
  listen<AgentConfigUpdatedMessage>("askit:agent_config_updated", (event) => {
    const { agent_id, key, value } = event.payload;
    let store = agentConfigUpdatedMessageStore.get(agent_id);
    if (!store) {
      return;
    }
    store.set({ key, value });
  }).then((unlistenFn) => {
    unlistenAgentConfigUpdated = unlistenFn;
  });

  // Listen for error messages
  listen<AgentErrorMessage>("askit:agent_error", (event) => {
    const { agent_id, message } = event.payload;
    let store = agentErrorMessageStore.get(agent_id);
    if (!store) {
      return;
    }
    store.set(message);
  }).then((unlistenFn) => {
    unlistenAgentError = unlistenFn;
  });

  // Listen for input messages
  listen<AgentInMessage>("askit:agent_in", (event) => {
    const { agent_id, ch } = event.payload;
    let store = agentInMessageStore.get(agent_id);
    if (!store) {
      return;
    }
    store.set({ ch, t: Date.now() });
  }).then((unlistenFn) => {
    unlistenAgentIn = unlistenFn;
  });

  listen<AgentSpecUpdatedMessage>("askit:agent_spec_updated", (event) => {
    const { agent_id } = event.payload;
    let store = agentSpecUpdatedMessageStore.get(agent_id);
    if (!store) {
      return;
    }
    store.set(Date.now());
  }).then((unlistenFn) => {
    unlistenAgentSpecUpdated = unlistenFn;
  });

  return () => {
    unlistenAgentConfigUpdated?.();
    unlistenAgentError?.();
    unlistenAgentIn?.();
    unlistenAgentSpecUpdated?.();
  };
});

// Agent Stream

export const streamState = $state({ id: "", name: "main" });
