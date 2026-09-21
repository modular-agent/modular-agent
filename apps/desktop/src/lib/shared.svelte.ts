import { listen } from "@tauri-apps/api/event";

import type { ModuleStatus } from "tauri-plugin-modular-agent-api";

import { closeTabAndNavigate, tabStore } from "$lib/tab-store.svelte";

import type {
  ModuleConfigUpdatedMessage,
  ModuleErrorMessage,
  ModuleInMessage,
  ModuleSpecUpdatedMessage,
  ModuleStatusChangedMessage,
  PatchRemovedMessage,
  PatchRunningChangedMessage,
  PatchStructureChangedMessage,
} from "./types";

// Origin convention: "desktop" is our own echo (ignore); "mcp" and null
// (module runtime internal) are external and must be reflected.
const SELF_ORIGIN = "desktop";
export function isExternalOrigin(origin: string | null | undefined): boolean {
  return origin !== SELF_ORIGIN; // null counts as external
}

let eventSeq = 0;

export type ModuleEventState = {
  configUpdated: { key: string; value: any; seq: number };
  error: { message: string; seq: number };
  input: { port: string; seq: number };
  specUpdated: number;
};

function defaultModuleEvent(): ModuleEventState {
  return {
    configUpdated: { key: "", value: null, seq: 0 },
    error: { message: "", seq: 0 },
    input: { port: "", seq: 0 },
    specUpdated: 0,
  };
}

class SharedModuleEvents {
  modules = $state<Record<string, ModuleEventState>>({});

  // Creates entry if not exists. Only call from module-node components, not from Tauri listeners.
  getModule(id: string): ModuleEventState {
    if (!this.modules[id]) {
      this.modules[id] = defaultModuleEvent();
    }
    return this.modules[id];
  }

  removeModule(id: string) {
    delete this.modules[id];
  }
}

export const sharedModuleEvents = new SharedModuleEvents();

// moduleId → last lifecycle status reported by the backend. Kept regardless
// of whether the node is mounted, unlike `sharedModuleEvents`: the status is
// state, not a one-shot event, and a node mounted later must see it. Entries
// are never removed (a stale id only holds a short string). The editor pulls
// the current statuses when a patch is running, since transitions made before
// this window listened (auto start, background launch) never arrive here.
export const sharedModuleStatuses = $state<Record<string, ModuleStatus>>({});

class SharedPatchEvents {
  // patchId → latest seq of an externally-originated structure change
  structureChanged = $state<Record<string, number>>({});

  // patchId → latest externally-originated run state. Without this, a patch
  // started from MCP, auto-start, or another window leaves the UI showing the
  // state it had when the tab was opened.
  runningChanged = $state<Record<string, { running: boolean; seq: number }>>({});
}

export const sharedPatchEvents = new SharedPatchEvents();

// Tauri event listeners (module-level, live for the app's lifetime)
$effect.root(() => {
  listen<ModuleConfigUpdatedMessage>("ma:module_config_updated", (event) => {
    if (!isExternalOrigin(event.payload.origin)) return;
    const { module_id, key, value } = event.payload;
    const module = sharedModuleEvents.modules[module_id];
    if (!module) return;
    // Note: `modules` is deeply reactive, so consumers reading `configUpdated.value`
    // get a $state proxy regardless of what is assigned here. Consumers that copy
    // the value into non-reactive storage ($state.raw nodes) must snapshot it.
    module.configUpdated = { key, value, seq: ++eventSeq };
  });

  listen<ModuleErrorMessage>("ma:module_error", (event) => {
    const { module_id, message } = event.payload;
    const module = sharedModuleEvents.modules[module_id];
    if (!module) return;
    module.error = { message, seq: ++eventSeq };
  });

  listen<ModuleInMessage>("ma:module_in", (event) => {
    const { module_id, port } = event.payload;
    const module = sharedModuleEvents.modules[module_id];
    if (!module) return;
    module.input = { port, seq: ++eventSeq };
  });

  listen<ModuleSpecUpdatedMessage>("ma:module_spec_updated", (event) => {
    if (!isExternalOrigin(event.payload.origin)) return;
    const { module_id } = event.payload;
    const module = sharedModuleEvents.modules[module_id];
    if (!module) return;
    module.specUpdated = ++eventSeq;
  });

  // Not origin-filtered: runtime state, never an echo of a local edit.
  listen<ModuleStatusChangedMessage>("ma:module_status_changed", (event) => {
    const { module_id, status } = event.payload;
    sharedModuleStatuses[module_id] = status;
  });

  listen<PatchStructureChangedMessage>("ma:patch_structure_changed", (event) => {
    if (!isExternalOrigin(event.payload.origin)) return;
    const { patch_id } = event.payload;
    sharedPatchEvents.structureChanged[patch_id] = ++eventSeq;
  });

  listen<PatchRunningChangedMessage>("ma:patch_running_changed", (event) => {
    if (!isExternalOrigin(event.payload.origin)) return;
    const { patch_id, running } = event.payload;
    sharedPatchEvents.runningChanged[patch_id] = { running, seq: ++eventSeq };
  });

  // Deliberately not origin-filtered: a sidebar delete goes through the plugin
  // handle and arrives as our own "desktop" echo, and this is the only path
  // that closes the tab of a removed patch.
  listen<PatchRemovedMessage>("ma:patch_removed", (event) => {
    const { patch_id } = event.payload;
    // Closing the tab triggers editor-host's cleanup, which unloads the
    // (already removed) patch from the backend idempotently.
    if (tabStore.tabs.find((t) => t.id === patch_id)) {
      closeTabAndNavigate(patch_id);
    }
  });
});
