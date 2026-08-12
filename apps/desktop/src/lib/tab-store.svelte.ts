import { goto } from "$app/navigation";

import { removeHistory } from "$lib/components/preset-editor/history.svelte";

export type Tab = { id: string; name: string };

class TabStore {
  tabs = $state<Tab[]>([]);
  activeTabId = $state("");
  runningMap = $state<Record<string, boolean>>({});
  dirtyMap = $state<Record<string, boolean>>({});

  openTab(id: string, name: string) {
    if (!this.tabs.find((t) => t.id === id)) {
      this.tabs = [...this.tabs, { id, name }];
    }
    this.activeTabId = id;
  }

  setRunning(id: string, running: boolean) {
    this.runningMap[id] = running;
  }

  setDirty(id: string, dirty: boolean) {
    this.dirtyMap[id] = dirty;
  }

  closeTab(id: string) {
    removeHistory(id);
    delete this.runningMap[id];
    delete this.dirtyMap[id];
    const index = this.tabs.findIndex((t) => t.id === id);
    if (index === -1) return;
    this.tabs = this.tabs.filter((t) => t.id !== id);
    if (this.activeTabId === id) {
      if (this.tabs.length === 0) {
        this.activeTabId = "";
      } else {
        this.activeTabId = this.tabs[Math.min(index, this.tabs.length - 1)].id;
      }
    }
  }

  updateName(id: string, name: string) {
    const tab = this.tabs.find((t) => t.id === id);
    if (tab) tab.name = name;
  }
}

export const tabStore = new TabStore();

/**
 * Close a tab and move the route off it when it was the active one. Guarded on
 * the current path so a sidebar delete made from Settings or the preset list
 * does not yank the user into the editor. Reads `window.location` rather than
 * `$app/state` because this also runs from a Tauri event callback, outside any
 * component.
 */
export async function closeTabAndNavigate(id: string) {
  const wasActive = tabStore.activeTabId === id;
  const onEditorRoute = window.location.pathname.startsWith("/preset_editor");
  tabStore.closeTab(id);
  if (!wasActive || !onEditorRoute) return;
  if (tabStore.tabs.length === 0) {
    await goto("/open_presets");
  } else {
    await goto(`/preset_editor/${tabStore.activeTabId}`, { noScroll: true });
  }
}
