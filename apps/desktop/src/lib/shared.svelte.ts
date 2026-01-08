import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { SvelteMap } from "svelte/reactivity";
import { writable, type Writable } from "svelte/store";

import type { AgentStreamInfo, AgentStreamSpec } from "tauri-plugin-askit-api";
import { getAgentStreamInfos, updateAgentStreamSpec } from "tauri-plugin-askit-api";

import {
  importAgentStream,
  newAgentStream,
  removeAgentStream,
  renameAgentStream,
  startAgentStream,
  stopAgentStream,
} from "./agent";
import type {
  AgentConfigUpdatedMessage,
  AgentErrorMessage,
  AgentInMessage,
  AgentSpecUpdatedMessage,
} from "./types";

// Agent Streams

export const streamInfos = new SvelteMap<string, AgentStreamInfo>();

export async function reloadStreamInfos() {
  const streams = await getAgentStreamInfos();
  streamInfos.clear();
  streams.forEach((s) => streamInfos.set(s.id, s));
}

export async function newStream(name: string): Promise<string> {
  const id = await newAgentStream(name);
  await reloadStreamInfos();
  return id;
}

export async function renameStream(id: string, newName: string): Promise<string> {
  const name = await renameAgentStream(id, newName);
  await reloadStreamInfos();
  return name;
}

export async function deleteStream(id: string): Promise<void> {
  await removeAgentStream(id);
  await reloadStreamInfos();
}

export async function importStream(path: string): Promise<string> {
  const id = await importAgentStream(path);
  await reloadStreamInfos();
  return id;
}

export async function startStream(id: string) {
  let info = streamInfos.get(id);
  if (!info || info.running) {
    return;
  }
  await startAgentStream(id);
  await reloadStreamInfos();
}

export async function stopStream(id: string) {
  let info = streamInfos.get(id);
  if (!info || !info.running) {
    return;
  }
  await stopAgentStream(id);
  await reloadStreamInfos();
}

export async function updateStreamSpec(id: string, spec: Partial<AgentStreamSpec>): Promise<void> {
  await updateAgentStreamSpec(id, spec);
}

$effect.root(() => {
  reloadStreamInfos();
});

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
