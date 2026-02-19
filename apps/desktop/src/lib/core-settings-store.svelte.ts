import { resolveHotkeys, resolveQuickAddAgents, type ResolvedHotkeys } from "./hotkeys";
import type { CoreSettings } from "./types";

class CoreSettingsStore {
  snapEnabled = $state(true);
  snapGridSize = $state(192);
  showGrid = $state(true);
  gridGap = $state(192);
  connectionOpacity = $state(0.8);
  maxHistoryLength = $state(2000);
  shortcutKeys = $state<Record<string, string> | null>(null);
  colorMode = $state<string>("");

  hotkeys: ResolvedHotkeys = $derived(resolveHotkeys(this.shortcutKeys));
  quickAddAgents = $derived(resolveQuickAddAgents(this.shortcutKeys));

  update(settings: CoreSettings) {
    this.snapEnabled = settings.snap_enabled ?? true;
    this.snapGridSize = settings.snap_grid_size ?? 192;
    this.showGrid = settings.show_grid ?? true;
    this.gridGap = settings.grid_gap ?? 192;
    this.connectionOpacity = settings.connection_opacity ?? 0.8;
    this.maxHistoryLength = settings.max_history_length ?? 2000;
    this.shortcutKeys = settings.shortcut_keys ?? null;
    this.colorMode = settings.color_mode ?? "";
  }
}

export const coreSettingsStore = new CoreSettingsStore();
