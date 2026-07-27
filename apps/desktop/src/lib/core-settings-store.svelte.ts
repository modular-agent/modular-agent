import { resolveHotkeys, resolveQuickAddAgents, type ResolvedHotkeys } from "./hotkeys";
import type { CoreSettings } from "./types";

export const CORE_DEFAULTS = {
  connectionOpacity: 0.8,
  gridGap: 240,
  maxHistoryLength: 2000,
  mcpServerEnabled: false,
  mcpServerPort: 8765,
  showGrid: true,
  snapEnabled: true,
  snapGridSize: 240,
} as const;

class CoreSettingsStore {
  colorMode = $state<string>("");
  connectionOpacity = $state<number>(CORE_DEFAULTS.connectionOpacity);
  gridGap = $state<number>(CORE_DEFAULTS.gridGap);
  maxHistoryLength = $state<number>(CORE_DEFAULTS.maxHistoryLength);
  shortcutKeys = $state<Record<string, string> | null>(null);
  showGrid = $state<boolean>(CORE_DEFAULTS.showGrid);
  snapEnabled = $state<boolean>(CORE_DEFAULTS.snapEnabled);
  snapGridSize = $state<number>(CORE_DEFAULTS.snapGridSize);

  hotkeys: ResolvedHotkeys = $derived(resolveHotkeys(this.shortcutKeys));
  quickAddAgents = $derived(resolveQuickAddAgents(this.shortcutKeys));

  update(settings: CoreSettings) {
    this.colorMode = settings.color_mode ?? "";
    this.connectionOpacity = settings.connection_opacity ?? CORE_DEFAULTS.connectionOpacity;
    this.gridGap = settings.grid_gap ?? CORE_DEFAULTS.gridGap;
    this.maxHistoryLength = settings.max_history_length ?? CORE_DEFAULTS.maxHistoryLength;
    this.shortcutKeys = settings.shortcut_keys ?? null;
    this.showGrid = settings.show_grid ?? CORE_DEFAULTS.showGrid;
    this.snapEnabled = settings.snap_enabled ?? CORE_DEFAULTS.snapEnabled;
    this.snapGridSize = settings.snap_grid_size ?? CORE_DEFAULTS.snapGridSize;
  }
}

export const coreSettingsStore = new CoreSettingsStore();
