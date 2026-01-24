import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { SvelteMap } from "svelte/reactivity";
import { writable, type Writable } from "svelte/store";

import type { PresetSpec } from "tauri-plugin-mak-api";
import { getPresetInfos, updatePresetSpec } from "tauri-plugin-mak-api";

import {
  getCoreSettings,
  importPreset,
  newPreset,
  removePreset,
  renamePreset,
  startPreset,
  stopPreset,
} from "./agent";
import type {
  AgentConfigUpdatedMessage,
  AgentErrorMessage,
  AgentInMessage,
  AgentSpecUpdatedMessage,
  PresetInfoExt,
} from "./types";

// Presets

export const presetInfos = new SvelteMap<string, PresetInfoExt>();

export async function reloadPreseetInfos() {
  const presets = (await getPresetInfos()) as PresetInfoExt[];
  const coreSettings = await getCoreSettings();
  const auto_start_presets = coreSettings.auto_start_presets || [];
  presetInfos.clear();
  presets.forEach((s) => {
    if (auto_start_presets.includes(s.name)) {
      s.run_on_start = true;
    }
    presetInfos.set(s.id, s);
  });
}

export async function newPresetAndReload(name: string): Promise<string> {
  const id = await newPreset(name);
  await reloadPreseetInfos();
  return id;
}

export async function renamePresetAndReload(id: string, newName: string): Promise<string> {
  const name = await renamePreset(id, newName);
  await reloadPreseetInfos();
  return name;
}

export async function deletePresetAndReload(id: string): Promise<void> {
  await removePreset(id);
  await reloadPreseetInfos();
}

export async function importPresetAndReload(path: string): Promise<string> {
  const id = await importPreset(path);
  await reloadPreseetInfos();
  return id;
}

export async function startPresetAndReload(id: string) {
  let info = presetInfos.get(id);
  if (info?.running) {
    return;
  }
  await startPreset(id);
  await reloadPreseetInfos();
}

export async function stopPresetAndReload(id: string) {
  let info = presetInfos.get(id);
  if (!info || !info.running) {
    return;
  }
  await stopPreset(id);
  await reloadPreseetInfos();
}

export async function updatePresetSpecAndReload(
  id: string,
  spec: Partial<PresetSpec>,
): Promise<void> {
  await updatePresetSpec(id, spec);
  await reloadPreseetInfos();
}

$effect.root(() => {
  reloadPreseetInfos();
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
let agentInMessageStore: Map<string, Writable<{ port: string; t: number }>> = new Map<
  string,
  Writable<{ port: string; t: number }>
>();

export function subscribeAgentInMessage(
  agentId: string,
  callback: (message: { port: string; t: number }) => void,
): () => void {
  let store = agentInMessageStore.get(agentId);
  if (!store) {
    store = writable({ port: "", t: 0 });
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
  listen<AgentConfigUpdatedMessage>("mak:agent_config_updated", (event) => {
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
  listen<AgentErrorMessage>("mak:agent_error", (event) => {
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
  listen<AgentInMessage>("mak:agent_in", (event) => {
    const { agent_id, port } = event.payload;
    let store = agentInMessageStore.get(agent_id);
    if (!store) {
      return;
    }
    store.set({ port, t: Date.now() });
  }).then((unlistenFn) => {
    unlistenAgentIn = unlistenFn;
  });

  listen<AgentSpecUpdatedMessage>("mak:agent_spec_updated", (event) => {
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
