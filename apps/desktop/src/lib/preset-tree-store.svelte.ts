import { listen } from "@tauri-apps/api/event";

import { getDirEntries } from "$lib/modular_agent";

class PresetTreeStore {
  entries = $state<Record<string, string[]>>({ "": [] });
  private refreshSeq = new Map<string, number>();

  async loadRoot() {
    try {
      this.entries = { "": await getDirEntries("") };
    } catch (e) {
      console.error("Failed to load preset directory:", e);
    }
  }

  async expandFolder(path: string) {
    try {
      const es = await getDirEntries(path);
      this.entries = { ...this.entries, [path]: es };
    } catch (e) {
      console.error("Failed to expand folder:", e);
    }
  }

  /** Refresh a loaded directory from backend. Ignores unexpanded paths. Discards stale responses. */
  async refresh(path: string) {
    if (!(path in this.entries)) return;
    const seq = (this.refreshSeq.get(path) ?? 0) + 1;
    this.refreshSeq.set(path, seq);
    try {
      const es = await getDirEntries(path);
      if (this.refreshSeq.get(path) !== seq) return;

      // Prune stale children entries for folders that no longer exist
      const childPrefix = path ? path + "/" : "";
      const validFolders = new Set(
        es.filter((e) => e.endsWith("/")).map((e) => childPrefix + e.slice(0, -1)),
      );
      const updated: Record<string, string[]> = { ...this.entries, [path]: es };
      for (const key of Object.keys(updated)) {
        if (key !== path && key.startsWith(childPrefix)) {
          const topLevel = key
            .split("/")
            .slice(0, (path ? path.split("/").length : 0) + 1)
            .join("/");
          if (!validFolders.has(topLevel)) {
            delete updated[key];
          }
        }
      }
      this.entries = updated;
    } catch (e) {
      console.error("Failed to refresh preset directory:", e);
    }
  }
}

export const presetTreeStore = new PresetTreeStore();

// App-lifetime Tauri event listener (component-independent, like SharedAgentEvents)
$effect.root(() => {
  listen<{ path: string }>("ma:preset_list_changed", (event) => {
    presetTreeStore.refresh(event.payload.path);
  });
});
