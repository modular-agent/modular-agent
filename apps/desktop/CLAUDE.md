# CLAUDE.md

See root CLAUDE.md for common agent development patterns.

## Overview

Modular Agent Desktop is a Tauri 2 desktop application for multi-agent workflow orchestration. It provides a visual editor for creating and managing agent presets.

## Build & Development Commands

```bash
# Full Tauri app development
npm run tauri dev

# Build Rust backend (from anywhere in the workspace)
cargo build -p modular-agent-desktop

# Custom build (select agent packages to include)
cargo run --manifest-path ../../tools/ma-config/Cargo.toml -- desktop

# Type checking
npm run check

# Format code
npm run format

# Run tests
npm test
```

## Tech Stack

- **Frontend**: Svelte 5 + SvelteKit (static adapter), TypeScript, Tailwind CSS
- **Backend**: Rust with Tauri 2
- **Core**: `modular-agent-core` crate provides the agent runtime

## Directory Structure

```
src/                    # Svelte frontend
  routes/               # SvelteKit pages
    preset_editor/      # Visual agent workflow editor (thin page shell)
    open_presets/       # Preset file browser
    settings/           # App settings
    logs/               # Log viewer (production debug)
  lib/
    agent.ts            # Tauri command wrappers, preset conversion utilities, setCoreSettings (partial merge + store update)
    core-settings-store.svelte.ts # Reactive CoreSettings singleton ($state fields, $derived hotkeys), CORE_DEFAULTS constant — consumed by EditorState, editor-canvas, +layout, Core.svelte
    hotkeys.ts          # Keyboard shortcut definitions, matching, resolving, formatting
    log-store.svelte.ts # Global log store + console capture (attachLogger, monkey-patch)
    sanitize.ts         # HTML sanitization utilities (sanitizeHtml, renderMarkdown, escapeHtml, isSafeImageSrc)
    shared.svelte.ts    # Global agent event bus ($state + seq counter, replaces old writable stores)
    tab-store.svelte.ts # Multi-tab state singleton (tabs, activeTabId)
    modular_agent.ts    # Low-level Tauri invoke wrappers
    types.ts            # TypeScript type definitions
    components/
      preset-editor/    # Preset editor components (context-based, multi-instance)
        editor-host.svelte    # Tab instance container (renders all tabs, visibility toggle)
        editor-instance.svelte # Per-tab wrapper (SvelteFlowProvider scope, EditorState creation)
        context.svelte.ts  # EditorState class + setEditor/useEditor context API
        history.svelte.ts  # Command pattern undo/redo (CommandHistory + 11 command classes)
        editor-canvas.svelte  # SvelteFlow canvas, keyboard shortcuts, agent list popup
        editor-header.svelte  # Header bar (menubar, preset name, play/stop, status)
        agent-node.svelte     # Agent node component with event subscriptions
        agent-config.svelte   # Agent config input/display (node view, with Handles)
        sidebar-config.svelte # Agent config input (sidebar, input renderers only, no readonly/hidden)
        node-base.svelte      # Base node frame with handles
        node-context-menu.svelte  # Right-click context menu
        menubar.svelte        # File menu (New/Save/Import/Export)
        preset-actions.svelte # Play/Stop buttons
        preset-name.svelte    # Breadcrumb path display
        messages.svelte       # Message display
      agent-list/       # Agent category tree (popup, click to add)
      ui/               # shadcn-svelte UI components
        sonner/           # Toaster with OS-specific offset (Windows custom titlebar)

src-tauri/src/          # Rust backend
  lib.rs                # Tauri app setup and command registration
  modular_agent_desktop/
    app.rs              # ModularAgentApp state, preset management
    settings.rs         # CoreSettings, agent global configs
    observer.rs         # ModularAgentEvent → Tauri event bridge
    tray.rs             # System tray
    window.rs           # Window management
    autostart.rs        # OS autostart integration
    shortcut.rs         # Global keyboard shortcuts
```

## Key Concepts

- **Presets**: Agent workflow configurations stored as JSON in `~/.modular_agent/presets/`
- **ModularAgent**: Core runtime from `modular-agent-core` that manages agent lifecycle
- **ModularAgentApp**: Tauri state wrapper managing presets and the ModularAgent instance
- **tauri-plugin-modular-agent**: Plugin providing `ModularAgentExt` trait for accessing ModularAgent from Tauri

## Preset Editor Architecture

The preset editor keeps per-tab instances alive using CSS visibility toggling. Tab switching is instant — no component destruction/recreation.

- **`EditorHost`** (`editor-host.svelte`): Renders all open tabs simultaneously via `{#each tabStore.tabs}`. Active tab gets `visibility: visible`, others get `visibility: hidden` (not `display: none` — SvelteFlow needs layout dimensions). Loads preset data from backend on tab open, cleans up on tab close. Uses `untrack()` to avoid `$effect` infinite loops when managing the `flows` record.
- **`EditorInstance`** (`editor-instance.svelte`): Per-tab wrapper inside `SvelteFlowProvider`. Creates `EditorState` via `setEditor()`, syncs `active` prop, and dispatches `resize` event when becoming visible.
- **`EditorState`** class (`context.svelte.ts`): Holds all editor state (`nodes`, `edges`, `running`, etc.) and methods (save, start/stop, clipboard, node operations). Created via `setEditor()`, consumed via `useEditor()`. Has `active` property — titlebar bindings and `$effect.pre` for title sync are gated with `if (!this.active) return;`.
- **`shared.svelte.ts`**: Global agent event bus using `$state` + monotonic `seq` counter. `SharedAgentEvents` class keyed by agent ID. Tauri listeners update entries regardless of tab visibility — non-active tabs continue receiving backend events (nodes stay mounted).
- **`tab-store.svelte.ts`**: Global tab state singleton. Navigation entry points (`nav-presets`, `editor-canvas`, `context.svelte.ts`) call `tabStore.openTab()` before `goto()`. The `[id]/+page.svelte` route only handles deep-link tab activation — its `$effect` uses `untrack()` to avoid reacting to `tabStore.tabs` changes (only reacts to route param changes). Without this, `closeTab` modifying `tabs` would re-trigger the deep-link handler before navigation completes.
- **Tab close lifecycle**: `closeTab()` removes tab from `tabStore.tabs` → `editor-host` `$effect` removes `flows` entry and calls `closePreset(id)` (fire-and-forget, only for stopped presets) → backend `close_preset` calls `remove_preset` (idempotent) and removes name→ID mapping. If `loadFlow` was still in progress when tab closed, the `finally` block handles backend cleanup.
- **Window event handlers**: All `<svelte:window>` handlers in `editor-canvas.svelte` are guarded with `if (!editor.active) return;` to prevent multiple instances from handling the same event.
- **DOM scoping**: `document.querySelector('.svelte-flow')` is replaced with `bind:this` + scoped `canvasContainer.querySelector()` since multiple SvelteFlow instances coexist in the DOM.
- **Config components**: `agent-config.svelte` (node view, has display + input renderers and Handles) and `sidebar-config.svelte` (sidebar, input renderers only — skips readonly/hidden configs). Adding a new input renderer type requires updating both.
- **Resize snap**: `node-base.svelte` uses `$state` (not `$derived`) for `wd`/`ht` so that custom absolute-grid snapping in `onResize` isn't clobbered by SvelteFlow's `onChange` writing unsnapped values to the store. `localResizing` plain flag guards the `$effect` that syncs props → `wd`/`ht`. `editor.resizing` disables `effectiveSnapGrid` during resize so SvelteFlow's built-in delta snap doesn't interfere. Definitions with `hints.free_size` opt out: their resize skips the snapping and the 1-grid minimum entirely, and `editor.draggingFreeSize` (set in `handleNodeDragStart` when every dragged node is free_size) disables `effectiveSnapGrid` during drag so their position doesn't snap either. `hints.no_resize` hides the `NodeResizer` handles entirely.
- **Initial node size**: `AddAgentCommand.execute()` uses `AgentDefinition.hints.width/height` (grid unit multipliers) when available, falling back to 1 grid unit (`snapGridSize`). Pre-existing `spec.width/height` from `newAgentSpec()` takes priority over hints. With `hints.free_size`, `width`/`height` are pixel values instead of grid multipliers (fallback 120px).
- **Node title color**: `agent-node.svelte` uses `resolveNodeColor(data, agentDef)` from `agent.ts` — `data.color` (per-instance) > `agentDef.hints.color` (per-definition) > kind-based fallback. Supports both palette indices (1-7) and hex codes (`#rrggbb`). Changeable via inspector (single node, hex supported) or context menu Color submenu (multi-node batch, palette only). `KIND_COLOR_DEFAULTS` in `agent.ts` is the shared kind→palette-index mapping used by both inspector and context menu "Apply to ports".
- **Node background color**: `bg_color` extension > `agentDef.hints.bg_color`, no fallback (null = default gray). When set, both body and title bar are opaque by default; a UI package can override the body via a `nodeStyles` entry keyed on def_name (e.g. Note gets an 85% translucent body). Disabled (`bg-muted`) and unknown-definition (`bg-destructive`) styling takes precedence.
- **Node foreground color**: `fg_color` extension > `agentDef.hints.fg_color`, no fallback (null = theme default). Applied as `style:color` on the node body in `node-base.svelte`, so body text (incl. markdown) inherits it; the title bar text stays governed by `color`, and port labels keep their own colors. Same disabled/unknown-definition suppression as `bg_color`. Note ships `fg_color="#44403b"` so its sticky body stays readable in dark theme.
- **Port colors**: `port_colors` extension (`Record<string, number | string>`) overrides per-port handle/label colors and edge stroke colors from that port. Inspector provides "Apply to ports" (copies title color to all non-err ports) and "Clear". Edge creation in all 5 code paths (`presetToFlow`, `handleBeforeConnect`, `AddConnectionCommand.execute`, `DeleteCommand.undo`, `PasteCommand.execute`) passes `sourcePortColors` to `connectionSpecToEdge`. `refreshEdgeColorsForNode()` on `EditorState` updates existing edges when `port_colors` changes (including undo/redo).
- **Per-instance extensions**: `EXTENSION_KEYS` registry in `inspector-state.svelte.ts` lists extension keys (currently `["color", "port_colors", "bg_color", "fg_color"]`); adding a new extension = add key + add UI in `inspector.svelte` + add rendering where the node is drawn (`agent-node.svelte` or `node-base.svelte`). Inspector sync uses `_lastSyncedData` reference comparison to detect `updateNodeData` changes on the same selected node (enables undo/redo sync without ad-hoc patches).
- **Extension persistence**: `AgentSpec.extensions` (`#[serde(flatten)]`) stores extension values; `updateAgentSpec` with `null` removes the key (Rust `spec.rs` `update()` calls `shift_remove` on null). Frontend sends `undefined` to `updateNodeData` and `null` to `updateAgentSpec` when clearing.
- **Connection opacity**: Applied via CSS custom property `--connection-opacity` on the canvas container div, consumed by `stroke-opacity` on `.svelte-flow__edge-path` and `.svelte-flow__connection-path`. Selected edges always render at full opacity. Default 80%, configurable in Core Settings (`connection_opacity: 0.0–1.0`).
- **Keyboard shortcuts**: See "Keyboard Shortcuts" section below.
- **Undo/Redo**: See "Undo/Redo (Command Pattern)" section below.

## Keyboard Shortcuts

All keyboard shortcuts are defined in `src/lib/hotkeys.ts` and customizable via Settings → Core.

- **`hotkeys.ts`**: Central module — `DEFAULT_HOTKEYS` definitions, `matchHotkey()` matcher, `resolveHotkeys()` / `resolveQuickAddAgents()` resolvers, `formatHotkey()` display formatter.
- **`editor-canvas.svelte`**: Table-driven `handleKeydown` dispatches via action table + `matchHotkey()`. Hardcoded: Escape (popup close), Alt (snap modifier), Ctrl+R (block refresh).
- **`+layout.svelte`**: Fullscreen toggle via `matchHotkey()` + `event.preventDefault()`. Editor-canvas checks `event.defaultPrevented` to avoid double-handling.
- **Context menus**: `pane-context-menu.svelte` and `node-context-menu.svelte` display shortcuts dynamically via `formatHotkey()` + `hotkeys` prop.
- **Settings storage**: `shortcut_keys` in `CoreSettings` (Rust `HashMap<String, String>`). Quick Add agent assignments use `quick_add.N.agent` keys in the same map.
- **Key format**: `mod+s` (mod = Cmd on macOS, Ctrl on Win/Linux), `shift+a`, `f11`. Sequences: `"a 1"` (space-separated chords, 500ms timeout).
- **Quick Add**: `mod+1`–`mod+5` place agents at mouse position. Both key binding and agent type are user-customizable.

## Core Settings (Auto-Save)

- **`CORE_DEFAULTS`** (`core-settings-store.svelte.ts`): Exported `as const` object with all numeric/boolean setting defaults. `CoreSettingsStore` and `Core.svelte` use it as fallback; `EditorState` reads from `coreSettingsStore` (runtime SSoT) for field initializers.
- **`coreSettingsStore`** (`core-settings-store.svelte.ts`): Reactive singleton — `EditorState` field initializers read from it (guaranteed initialized by `initGlobals()`), and subscribes via `$effect` + `untrack()` for runtime changes.
- **Auto-save pattern** (`Core.svelte`): All settings auto-save on change (`onCheckedChange`/`onchange`/`onValueChange`) except `global_shortcut` which has a manual Save button because it requires app restart (`tauri-plugin-global-shortcut` only reads on startup).
- **`setCoreSettings(Partial<CoreSettings>)`** (`agent.ts`): Merges into `_coreSettings` cache + updates `coreSettingsStore` optimistically before the backend IPC call; backend `autostart::apply()` runs outside Mutex lock to prevent deadlock.

## Undo/Redo (Command Pattern)

`history.svelte.ts` implements undo/redo via the Command pattern. `EditorState` holds a `CommandHistory` instance.

- **"command executes everything"**: `EditorState` methods create a `Command` and call `history.executeAndPush(cmd)`. Both initial execution and redo go through `Command.execute()`. Logic lives only in Command classes.
- **Special cases** (SvelteFlow acts first):
  - `ondelete`: SvelteFlow removes items → `handleOnDelete` creates `DeleteCommand`, calls `cmd.execute()` (backend sync only, items already gone), then `history.push(cmd)`.
  - `onconnect`: SvelteFlow adds edge via `onbeforeconnect` → `handleOnConnect` finds the edge, does backend call, then `history.push(cmd)`.
  - Node drag/resize: SvelteFlow moves/resizes → `handleNodeDragStop`/`handleResizeEnd` create commands and `history.push()`.
- **ID remapping**: Backend assigns new IDs when agents are re-created (redo of Add, undo of Delete). `CommandHistory.redo()`/`undo()` propagates ID changes to remaining stack commands via `Command.remapId()`. All 12 command classes implement `remapId`.
- **Config coalescing**: `pushCoalescing()` merges rapid config changes (same node+key within 500ms) into one undo entry.
- **Not undoable**: Viewport pan/zoom, snap toggle, preset start/stop, selection changes, backend-initiated config updates (`agentEvent.configUpdated`).

Command classes (13): `AddAgentCommand`, `DeleteCommand`, `CutCommand` (extends Delete), `AddConnectionCommand`, `MoveNodesCommand` (also for Align/Distribute), `ResizeNodeCommand`, `PasteCommand`, `UpdateConfigCommand`, `UpdateTitleCommand`, `UpdateExtensionCommand`, `BatchUpdateExtensionCommand` (multi-node color/port_colors via context menu), `ToggleDisabledCommand`, `ToggleShowErrCommand`.

## $effect + $state: Avoiding Infinite Loops

When writing to `$state` inside `$effect`, always use `untrack()` to prevent read-write cycles:

```typescript
// BAD: .push() reads array (tracked) → mutates → re-triggers → infinite loop
$effect(() => {
  const { message, seq } = agentEvent.error;
  if (!seq) return;
  errorMessages.push(message);
});

// GOOD: untrack prevents the write from creating a dependency
$effect(() => {
  const { message, seq } = agentEvent.error;
  if (!seq) return;
  untrack(() => errorMessages.push(message));
});
```

Common traps: `.push()` on arrays, `x += 1` (reads then writes), reading `data` props then calling `updateNodeData`.

When tracking one `$state` but reading/writing another inside the same `$effect`, wrap the non-tracked operations in `untrack()`:

```typescript
// EditorHost pattern: track tabStore.tabs, but read/write flows without tracking
$effect(() => {
  const currentTabs = tabStore.tabs; // tracked dependency
  untrack(() => {
    // read/write flows here without creating dependency
    for (const tab of currentTabs) {
      if (!(tab.id in flows)) loadFlow(tab.id);
    }
  });
});
```

## Error Handling (Tauri IPC)

IPC errors in `EditorState` use `withErrorToast` (user actions → toast) or `withErrorLog` (background → console only) from `context.svelte.ts`. Loop methods aggregate errors into a single toast. Messages are in English.

## Frontend-Backend Communication

Frontend calls Rust via Tauri's `invoke()`. Commands are defined in `lib.rs` with `#[tauri::command]` and wrapped in `src/lib/modular_agent.ts`.

Backend emits events to frontend via two mechanisms:

1. **Agent events** (`observer.rs`): Core engine events (`ModularAgentEvent`) relayed via broadcast channel. Events: `ma:agent_config_updated`, `ma:agent_error`, `ma:agent_in`, `ma:agent_spec_updated`. Listened globally in `shared.svelte.ts` via `$effect.root()`.
2. **Preset list events** (`app.rs`): Desktop app emits `ma:preset_list_changed` directly from Tauri commands via `app.emit()`. Payload: `{ path: String }` (the parent directory that changed). Listened globally in `preset-tree-store.svelte.ts` via `$effect.root()`.

## Preset List Events

When the filesystem under `~/.modular_agent/presets/` changes, the backend emits `ma:preset_list_changed` so the sidebar refreshes automatically.

| Command | Emit condition |
|---------|---------------|
| `new_preset_with_name_cmd` | Always (saves empty `PresetSpec::default()` to disk immediately) |
| `save_as_preset_cmd` | Always (uses `add_preset_with_name` to load spec into core engine + register name→ID mapping) |
| `save_preset_cmd` | Only when file is new (`!preset_path_exists`) |
| `delete_preset_cmd` | Always |
| `delete_folder_cmd` | Always (the deleted folder's parent) |
| `import_preset_cmd` | Always |
| `rename_preset_cmd` | Always (also emits `ma:preset_renamed` if preset is open) |
| `rename_folder_cmd` | Always (also emits `ma:preset_renamed` per affected open preset) |

- **Rename as primitive**: `rename_preset(name, new_name)` and `rename_folder(path, new_path)` are the core methods; `move_preset` and `move_folder` are thin wrappers that compute `new_name` from `target_dir` + basename.
- **Delete**: `delete_folder(path)` only removes an **empty** folder (`remove_dir`) — a right-click lands on the wrong row too easily for a recursive delete to be safe, so a non-empty folder fails with "Delete its contents first". `delete_preset` refuses while the preset is running ("Stop it first", same policy as rename/move) and drops its `auto_start_presets` entry via `remove_auto_start_presets`. Neither prunes emptied ancestor directories (rename/move do) — deleting removes exactly the row that was clicked.
- **Ancestor notification**: When `save_preset_cmd` or `new_preset_with_name_cmd` creates a new subdirectory (via `create_dir_all`), an additional event is emitted for the grandparent directory.
- **Helper**: `parent_preset_path(name)` extracts the parent directory from a preset name (e.g., `"Cat/MyPreset"` → `"Cat"`).
- **Frontend**: `PresetTreeStore` (`preset-tree-store.svelte.ts`) is a singleton ViewModel with `$effect.root()` listener (same pattern as `SharedAgentEvents`). Refreshes the directory at `event.payload.path` if it is in the loaded `entries` record; otherwise walks up to the nearest loaded ancestor and refreshes that instead (a new folder appeared on disk — e.g. an MCP `save_preset` into a fresh subdirectory — and must show up in an already-loaded parent). Uses per-path request counter to discard stale responses from concurrent events. `nav-presets.svelte` is a pure View that reads `presetTreeStore.entries` — no direct Tauri event handling. Uses keyed `{#each entries as entry (entry)}` to preserve folder open/close state across refreshes.
- **Preset file list UI**: VS Code-style full-row interaction — indentation via `depth` prop and `padding-left` (formula: `8 + depth * 16` px) instead of nested margins, so the entire row (left edge to right edge) is clickable, draggable, and right-clickable. Hover uses `hover:bg-sidebar-accent`. Tooltip uses native `title` attribute with preset-relative path (not OS path).

## MCP Server (External Agent Editing)

`modular-agent-core`'s built-in MCP server (feature `mcp-server`) lets external AI agents (Claude Code, Codex) read and edit presets. Disabled by default.

- **Enabling**: Settings → Core → "MCP Server" section — auto-saving toggle + port input (`CoreSettings.mcp_server_enabled` default `false`, `mcp_server_port` default `8765`). `apply_mcp_server()` in `settings.rs` stops/starts the server on change; `init_mcp_server()` starts it on launch from `RunEvent::Ready`. Handle lives in managed `McpServerState` (`tokio::sync::Mutex` held across the whole stop/start sequence — concurrent settings changes serialize, and enabled/port/token are re-read from `CoreSettings` inside the lock so the last caller applies the freshest state).
- **Token (backend-only)**: `mcp_server_token` is auto-generated (32 random bytes, hex) by `apply_mcp_server()` on first enable and persisted; the server requires `Authorization: Bearer <token>` on every request (constant-time compare, 401 + `WWW-Authenticate: Bearer` otherwise). `set_core_settings_cmd` strips `mcp_server_token` from incoming JSON — the frontend only ever reads it; the sole other writer is `regenerate_mcp_server_token_cmd`, which persists a fresh token, restarts the server with it, and returns it (old token dies immediately). Core.svelte shows the token and a ready-to-copy connect command: `claude mcp add --transport http modular-agent http://127.0.0.1:<port>/mcp --header "Authorization: Bearer <token>"` (streamable HTTP, binds 127.0.0.1 only).
- **Tools** (17, defined in core `src/mcp_server.rs`): `list_agent_definitions`, `get_agent_definition`, `list_presets`, `create_preset`, `get_preset_spec`, `add_agent`, `update_agent_spec`, `set_agent_configs`, `remove_agent`, `add_connection`, `remove_connection`, `save_preset`, `start_preset`, `stop_preset`, `write_external_input`, `get_agent_errors`, `get_external_outputs`. Failures an agent can fix itself return `is_error` tool results with corrective hints (valid handle lists, `PresetNameExists` guidance), not protocol errors. The two polling tools read a shared 200-entry ring of `AgentError`/`ExternalOutput` events; `latest_seq` is the seq of the last *returned* record (echoes `since_seq` on an empty page) — cursor semantics, not a global counter — plus a `dropped` count for broadcast-lag losses.
- **Origin convention**: every core mutation entry point stamps `EventEnvelope.origin` via `ModularAgent::with_origin` — `"desktop"` (tauri-plugin handle), `"mcp"` (MCP server handles), `None` (agent runtime internal). `shared.svelte.ts` defines `isExternalOrigin(origin)`: anything but `"desktop"` is external (null counts as external). Self-echo filtering is purely origin-based — there are no frontend suppression wrappers around the plugin APIs.
- **Event relay** (`observer.rs`): subscribes to `EventEnvelope`s and forwards `origin` in every Tauri payload. Besides the four agent events, it relays `PresetStructureChanged` → `ma:preset_structure_changed { origin, preset_id }`, `PresetRemoved` → `ma:preset_removed { origin, preset_id, name }`, `PresetRenamed` → `ma:preset_renamed` plus `ma:preset_list_changed` for old/new parent folders, and `PresetAdded` (named) / `PresetSaved` → `ma:preset_list_changed { path }` via `parent_preset_path` — so MCP-side create/save/rename refresh the sidebar. `ma:preset_removed` for an open tab calls `closeTabAndNavigate(preset_id)`; editor-host cleanup then unloads the already-removed preset idempotently. Like `ma:preset_renamed`, it is deliberately **not** origin-filtered — a sidebar delete arrives as our own `"desktop"` echo, and this is the only path that closes the tab. The only other desktop-origin removal is `close_preset` (fired *after* the tab is gone), so it no-ops.
- **Name index in core**: core owns the name→ID mapping (`preset_names`, `find_preset_id_by_name`); the desktop keeps no map of its own. Name collisions on create/rename fail with `AgentError::PresetNameExists`, surfaced to the UI (and hinted to MCP callers). `ModularAgentApp::open_preset` returns the live instance when the name is already loaded in core (e.g. MCP-created) instead of loading a duplicate from the file; `close_preset` calls core `remove_preset` (idempotent). `remove_agent` also removes spec-only agents (unknown definitions, no runtime instance) and still emits `PresetStructureChanged`. `get_preset_spec` keeps spec-only agents (it overlays live specs onto the stored entries rather than rebuilding from instances, so a save cannot drop them), and `update_agent_spec` / `set_agent_configs` fall back to patching the stored spec entry when there is no instance.
- **Diff merge (external edit → canvas)**: core emits `PresetStructureChanged { preset_id }` from all structure mutations (incl. `update_agent_spec` when the patch has non-`configs` keys) → `shared.svelte.ts` bumps `sharedPresetEvents.structureChanged[preset_id]` (per-preset seq, external origins only) → each `EditorState` `$effect` compares against `lastStructureSeq` (initialized from `baseStructureSeq`, captured by the host *before* the flow fetch — a reopened tab doesn't replay old events, a change landing mid-fetch still merges) → 300ms debounce (`scheduleExternalMerge`, re-arms while a drag/resize/command is in flight) → `applyExternalChanges()` fetches `getPresetInfo` + `getPresetSpec`, then re-checks after the IPC awaits: `mergeGen` generation token (newest run wins), tab still open, and `history.mutationSeq` unchanged (a local edit that landed mid-fetch discards the stale snapshot and re-arms) → `reconcileFlow()` (`merge.ts`) merges the fetched flow identity-preservingly — nodes keyed by agent id (unchanged nodes keep their object, array order, and `selected`), edges keyed by connection tuple — and returns `removedAgentIds`/`removedConnKeys`. On change: apply nodes/edges, `history.purgeInvalidated(...)`, `savedIndex = -1` (backend diverged from last save). Undo history and selection survive. Any merge failure falls back to `reloadFromBackend()` — full `presetToFlow` rebuild + `history.clear()`.
- **Undo purge policy**: `CommandHistory.purgeInvalidated` runs `Command.pruneExternalRemovals(removedAgentIds, removedConnKeys)` over both stacks; returning `false` drops the command, and commands may instead trim themselves partially — `MoveNodesCommand`/`BatchUpdateExtension`/`ToggleDisabled`/`ToggleShowErr`-style commands filter their per-node deltas (dropped only when empty), `DeleteCommand`/`PasteCommand` filter saved edges by `removedConnKeys` but drop entirely if a saved node (or an endpoint of a saved edge) was removed, single-node commands (config/title/extension/resize) drop when their node is gone. Commands without the hook are untouched.
- **Cargo note**: `src-tauri/Cargo.toml` enables the `mcp-server` feature on `modular-agent-core`. core and the Tauri plugin are in-tree workspace dependencies, and out-of-tree agents reach core by path from their `custom_agents/` clone, so nothing in the workspace patches crates.io. ma-config codegen keeps the feature on regen.

## Agent Plugins

Agent functionality is provided by external crates linked in `lib.rs`:

- `modular-agent-llm` - LLM integrations
- `modular-agent-std` - Standard utilities
- `modular-agent-web` - Web/HTTP agents
- `modular-agent-slack`, `modular-agent-sqlx`, etc.

std / llm / web are in-tree (`crates/`) and linked as workspace dependencies. The rest are
cloned from their own repositories into `custom_agents/` at the workspace root and linked
from there by ma-config.

## Custom Build (ma-config)

`tools/ma-config/` at the workspace root is a TUI wizard for selecting which agent packages a build links. It serves both apps: `ma-config desktop` and `ma-config cli`.

- Codegen generates `src-tauri/src/agents.rs` (do not edit manually) and updates `src-tauri/Cargo.toml`; `lib.rs` has `mod agents;` to link them.
- core and the Tauri plugin are in-tree and always linked, so they are not selectable. In-tree agents (std / llm / web) are emitted as `{ workspace = true }`; a feature override spells the path out instead, because cargo ignores a member's `default-features = false` when the workspace entry does not set it.
- Out-of-tree agents are emitted as `path = "../../../custom_agents/<name>"` — clone them there by hand first (`custom_agents/README.md` lists the repositories), since the wizard only offers clones that exist; ma-config fails pointing at that README when a selected clone has gone missing, and with a fix-up message when a clone still depends on `modular-agent-core` from crates.io (two copies of core, no visible agents). Custom agents outside the registry keep an inline `path` of their own in `[dependencies]`.
- Configuration is saved to `apps/desktop/ma-config.toml` (gitignored) with paths relative to the workspace root. The catalog is split: `tools/ma-config/registry.yaml` lists the in-tree agents (std / llm / web) only, and each out-of-tree crate carries its own entry — description, features, conflicts, default selection — in an `ma-registry.yaml` at the root of its clone, which the wizard picks up by scanning `custom_agents/`.

## HTML Sanitization

All `{@html}` rendering must go through `src/lib/sanitize.ts`:

- **`sanitizeHtml(html)`**: DOMPurify with restricted ALLOWED_TAGS/ALLOWED_ATTR. Use for HTML content from agents.
- **`renderMarkdown(raw)`**: `marked.parse()` then `sanitizeHtml()`. Use for markdown content.
- **`escapeHtml(str)`**: Text escaping (`<` → `&lt;` etc.). Use for plain-text fields interpolated into HTML strings (e.g., tool names, thinking text).
- **`isSafeImageSrc(src)`**: Validates image URL schemes (`data:image/`, `https://`, `http://` only).
- **Link safety**: DOMPurify `afterSanitizeAttributes` hook auto-adds `target="_blank" rel="noopener noreferrer"` to all `<a>` tags. Global click handler in `+layout.svelte` intercepts `https?://` links and opens them via `openUrl()` from `@tauri-apps/plugin-opener`.

## Markdown Styling

- Scoped CSS with `:global()` selectors per component — `@tailwindcss/typography` is NOT installed, do not use `prose` classes.
- Dark-mode-aware colors use CSS variables: `var(--muted)` for code backgrounds, `var(--link-color)` for links, `var(--border)` for borders, `var(--muted-foreground)` for subdued text.
- When rendering `{@html}` with block-level content (`<p>`, `<pre>`, etc.), the container must be a `<div>`, not `<p>` — browsers auto-close `<p>` on nested block elements, breaking DOM structure and scoped CSS.

## Logging

`tauri-plugin-log` provides logging to stdout, log files, and the in-app log viewer:

- **Rust side** (`lib.rs`): Targets are `Stdout`, `LogDir`, and `Webview`. Rust `log::info!()` etc. flow to all three.
- **Frontend** (`log-store.svelte.ts`): `initLogging()` is called in root `+layout.svelte` `onMount`. It:
  1. Uses `attachLogger()` to capture all log events into a reactive `LogStore`
  2. Monkey-patches `console.log/info/warn/error/debug` to forward to Rust's log pipeline
  3. Does NOT use `attachConsole()` (would cause infinite loop with monkey-patch)
- **Log viewer** (`/logs`): Displays `logStore.entries` with level filtering, text search, auto-scroll, and "Open Log Folder" button.
- **Log files**: Stored in platform-specific log directory (`appLogDir()`).

## Toast Notifications

`svelte-sonner` for toast. Import `<Toaster />` from `$lib/components/ui/sonner` (not `svelte-sonner` directly) — includes dark mode and Windows titlebar offset.

## Tauri Plugin Setup Checklist

When adding a new Tauri plugin:

1. `src-tauri/Cargo.toml` — Add dependency (use `[target.cfg(...)]` for desktop-only plugins)
2. `src-tauri/src/lib.rs` — Register with `.plugin()`
3. `src-tauri/capabilities/default.json` — Add permissions (some commands like `opener:allow-open-path` need scoped permissions with `allow: [{ "path": "..." }]`)
4. `package.json` — Add `@tauri-apps/plugin-*` JS binding
