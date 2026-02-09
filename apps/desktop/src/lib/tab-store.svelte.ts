import { removeHistory } from "$lib/components/preset-editor/history.svelte";

export type Tab = { id: string; name: string };

class TabStore {
  tabs = $state<Tab[]>([]);
  activeTabId = $state("");

  openTab(id: string, name: string) {
    if (!this.tabs.find((t) => t.id === id)) {
      this.tabs = [...this.tabs, { id, name }];
    }
    this.activeTabId = id;
  }

  closeTab(id: string) {
    removeHistory(id);
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
