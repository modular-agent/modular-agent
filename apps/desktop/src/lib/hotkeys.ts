// Hotkey definitions, matching, resolving, and formatting

const isMac =
  typeof navigator !== "undefined" && /Mac|iPod|iPhone|iPad/.test(navigator.platform);

// ── Types ──────────────────────────────────────────────────────────

export type HotkeyDefinition = {
  id: string;
  label: string;
  group: "Global" | "Editor" | "Quick Add";
  defaultKey: string;
  /** Quick Add only: default agent def_name */
  defaultAgent?: string;
};

export type ResolvedHotkey = {
  id: string;
  key: string; // normalised hotkey string (e.g. "mod+s")
};

export type ResolvedHotkeys = ResolvedHotkey[];

// ── Default definitions ────────────────────────────────────────────

export const DEFAULT_HOTKEYS: HotkeyDefinition[] = [
  // Global
  { id: "global_shortcut", label: "Show App Window", group: "Global", defaultKey: "" },
  {
    id: "fullscreen",
    label: "Toggle Fullscreen",
    group: "Global",
    defaultKey: isMac ? "" : "f11",
  },
  // Editor
  { id: "editor.save", label: "Save", group: "Editor", defaultKey: "mod+s" },
  { id: "editor.toggle_run", label: "Play / Stop", group: "Editor", defaultKey: "mod+." },
  { id: "editor.cut", label: "Cut", group: "Editor", defaultKey: "mod+x" },
  { id: "editor.copy", label: "Copy", group: "Editor", defaultKey: "mod+c" },
  { id: "editor.paste", label: "Paste", group: "Editor", defaultKey: "mod+v" },
  { id: "editor.select_all", label: "Select All", group: "Editor", defaultKey: "mod+a" },
  { id: "editor.add_agent", label: "Open Agent List", group: "Editor", defaultKey: "shift+a" },
  { id: "editor.undo", label: "Undo", group: "Editor", defaultKey: "mod+z" },
  { id: "editor.redo", label: "Redo", group: "Editor", defaultKey: "mod+shift+z" },
  { id: "editor.toggle_grid", label: "Toggle Grid", group: "Editor", defaultKey: "g" },
  // Quick Add
  {
    id: "quick_add.1",
    label: "Quick Add 1",
    group: "Quick Add",
    defaultKey: "mod+1",
    defaultAgent: "modular_agent_std::input::UnitInputAgent",
  },
  {
    id: "quick_add.2",
    label: "Quick Add 2",
    group: "Quick Add",
    defaultKey: "mod+2",
    defaultAgent: "modular_agent_std::input::TextInputAgent",
  },
  {
    id: "quick_add.3",
    label: "Quick Add 3",
    group: "Quick Add",
    defaultKey: "mod+3",
    defaultAgent: "modular_agent_std::display::DisplayValueAgent",
  },
  {
    id: "quick_add.4",
    label: "Quick Add 4",
    group: "Quick Add",
    defaultKey: "mod+4",
    defaultAgent: "modular_agent_std::ui::RouterAgent",
  },
  {
    id: "quick_add.5",
    label: "Quick Add 5",
    group: "Quick Add",
    defaultKey: "mod+5",
    defaultAgent: "modular_agent_std::ui::NoteAgent",
  },
];

// ── Resolving ──────────────────────────────────────────────────────

/** Resolve key bindings only (excludes .agent entries) */
export function resolveHotkeys(
  userOverrides: Record<string, string> | null | undefined,
): ResolvedHotkeys {
  return DEFAULT_HOTKEYS.map((def) => ({
    id: def.id,
    key: userOverrides?.[def.id] ?? def.defaultKey,
  }));
}

/** Resolve Quick Add agent assignments (reads .agent entries from shortcut_keys) */
export function resolveQuickAddAgents(
  userOverrides: Record<string, string> | null | undefined,
): Map<string, string> {
  const map = new Map<string, string>();
  for (const def of DEFAULT_HOTKEYS) {
    if (def.defaultAgent) {
      const override = userOverrides?.[`${def.id}.agent`];
      map.set(def.id, override ?? def.defaultAgent);
    }
  }
  return map;
}

/** Look up the resolved key string for a hotkey id */
export function getHotkeyKey(hotkeys: ResolvedHotkeys, id: string): string {
  return hotkeys.find((h) => h.id === id)?.key ?? "";
}

// ── Matching ───────────────────────────────────────────────────────

/**
 * Parse a single-chord hotkey string into its components.
 * Example: "mod+shift+a" → { ctrl: true (on Win), meta: true (on Mac), shift: true, key: "a" }
 */
function parseChord(chord: string): {
  ctrl: boolean;
  meta: boolean;
  shift: boolean;
  key: string;
} {
  const parts = chord.toLowerCase().split("+");
  let ctrl = false;
  let meta = false;
  let shift = false;
  let key = "";

  for (const p of parts) {
    if (p === "mod") {
      if (isMac) {
        meta = true;
      } else {
        ctrl = true;
      }
    } else if (p === "ctrl" || p === "control") {
      ctrl = true;
    } else if (p === "meta" || p === "command" || p === "cmd") {
      meta = true;
    } else if (p === "shift") {
      shift = true;
    } else {
      key = p;
    }
  }

  return { ctrl, meta, shift, key };
}

/** Check if a KeyboardEvent matches a single chord */
function matchChord(event: KeyboardEvent, chord: string): boolean {
  const parsed = parseChord(chord);

  // Check modifier keys match exactly (ignore Alt — used for snap)
  if (event.ctrlKey !== parsed.ctrl) return false;
  if (event.metaKey !== parsed.meta) return false;
  if (event.shiftKey !== parsed.shift) return false;

  // Compare key (case-insensitive)
  return event.key.toLowerCase() === parsed.key;
}

/**
 * Check if a hotkey string is a sequence (contains a space).
 * Example: "g c" → true, "mod+s" → false
 */
export function isSequence(hotkey: string): boolean {
  return hotkey.includes(" ");
}

/** Get the first chord of a sequence */
export function getSequenceFirst(hotkey: string): string {
  return hotkey.split(" ")[0];
}

/** Get the second chord of a sequence */
export function getSequenceSecond(hotkey: string): string {
  return hotkey.split(" ")[1] ?? "";
}

/**
 * Match a KeyboardEvent against a single-chord hotkey string.
 * For sequences, use isSequence/getSequenceFirst/getSequenceSecond + matchHotkey on each chord.
 */
export function matchHotkey(event: KeyboardEvent, hotkey: string): boolean {
  if (!hotkey) return false;
  if (isSequence(hotkey)) return false; // sequences must be handled by caller
  return matchChord(event, hotkey);
}

/** Match the first chord of a sequence or single-chord hotkey */
export function matchFirstChord(event: KeyboardEvent, hotkey: string): boolean {
  if (!hotkey) return false;
  const first = isSequence(hotkey) ? getSequenceFirst(hotkey) : hotkey;
  return matchChord(event, first);
}

// ── Formatting ─────────────────────────────────────────────────────

/** Format a hotkey string for display (e.g. "mod+s" → "Ctrl+S" / "⌘S") */
export function formatHotkey(hotkey: string): string {
  if (!hotkey) return "";

  // Handle sequences
  if (isSequence(hotkey)) {
    return hotkey
      .split(" ")
      .map((chord) => formatChord(chord))
      .join(" ");
  }

  return formatChord(hotkey);
}

function formatChord(chord: string): string {
  const parts = chord.toLowerCase().split("+");
  const result: string[] = [];

  for (const p of parts) {
    if (p === "mod") {
      result.push(isMac ? "⌘" : "Ctrl");
    } else if (p === "ctrl" || p === "control") {
      result.push(isMac ? "⌃" : "Ctrl");
    } else if (p === "meta" || p === "command" || p === "cmd") {
      result.push("⌘");
    } else if (p === "shift") {
      result.push(isMac ? "⇧" : "Shift");
    } else if (p === "alt") {
      result.push(isMac ? "⌥" : "Alt");
    } else {
      // Capitalise the key name
      result.push(p.charAt(0).toUpperCase() + p.slice(1));
    }
  }

  if (isMac) {
    // Mac style: no separators (⌘⇧A)
    return result.join("");
  }
  // Windows/Linux style: Ctrl+Shift+A
  return result.join("+");
}
