<script lang="ts">
  import { fade } from "svelte/transition";

  import {
    Background,
    BackgroundVariant,
    ControlButton,
    Controls,
    MiniMap,
    SvelteFlow,
    type Connection,
    type NodeEventWithPointer,
    type NodeTargetEventWithPointer,
    type NodeTypes,
    type OnDelete,
    type OnMove,
    type OnSelectionDrag,
  } from "@xyflow/svelte";
  import { getPresetSpec } from "tauri-plugin-modular-agent-api";
  import { addPresetWithName } from "tauri-plugin-modular-agent-api";

  import { goto } from "$app/navigation";

  import { getCoreSettings, getEdgeColor } from "$lib/agent";
  import { AgentList } from "$lib/components/agent-list/index.js";
  import PresetActionDialog from "$lib/components/preset-action-dialog.svelte";
  import { tabStore } from "$lib/tab-store.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import {
    resolveHotkeys,
    resolveQuickAddAgents,
    getHotkeyKey,
    matchHotkey,
    isSequence,
    getSequenceFirst,
    getSequenceSecond,
    matchFirstChord,
    formatHotkey,
    type ResolvedHotkeys,
  } from "$lib/hotkeys";
  import type { PresetNode, PresetEdge } from "$lib/types";

  import AgentNode from "./agent-node.svelte";
  import { useEditor } from "./context.svelte";
  import NodeContextMenu from "./node-context-menu.svelte";
  import PaneContextMenu from "./pane-context-menu.svelte";

  const editor = useEditor();
  const coreSettings = getCoreSettings();

  // --- Hotkey resolution ---

  const hotkeys: ResolvedHotkeys = resolveHotkeys(coreSettings.shortcut_keys);
  const quickAddAgents = resolveQuickAddAgents(coreSettings.shortcut_keys);

  const nodeTypes: NodeTypes = {
    agent: AgentNode,
  };

  // --- MiniMap visibility ---

  let showMiniMap = $state(false);
  let canvasContainer: HTMLElement;
  const HIDE_MARGIN = 50;

  function updateMiniMapVisibility() {
    const nodes = editor.nodes;
    if (nodes.length === 0) {
      showMiniMap = false;
      return;
    }

    const container = canvasContainer?.querySelector(".svelte-flow");
    if (!container) return;

    const w = container.clientWidth;
    const h = container.clientHeight;
    const bounds = editor.props.svelteFlow.getNodesBounds(nodes);
    const vp = editor.props.svelteFlow.getViewport();

    const sL = bounds.x * vp.zoom + vp.x;
    const sT = bounds.y * vp.zoom + vp.y;
    const sR = (bounds.x + bounds.width) * vp.zoom + vp.x;
    const sB = (bounds.y + bounds.height) * vp.zoom + vp.y;

    const isOutside = sL < 0 || sT < 0 || sR > w || sB > h;
    const isWellInside =
      sL > HIDE_MARGIN && sT > HIDE_MARGIN && sR < w - HIDE_MARGIN && sB < h - HIDE_MARGIN;

    if (isOutside) showMiniMap = true;
    else if (isWellInside) showMiniMap = false;
  }

  $effect(() => {
    editor.nodes;
    updateMiniMapVisibility();
  });

  // --- Mouse position tracking ---

  let mouseX = $state(0);
  let mouseY = $state(0);

  function handleMouseMove(event: MouseEvent) {
    if (!editor.active) return;
    mouseX = event.clientX;
    mouseY = event.clientY;
  }

  // --- Keyboard shortcuts ---

  function isEditableElement(el: EventTarget | null): boolean {
    return (
      el instanceof HTMLInputElement ||
      el instanceof HTMLTextAreaElement ||
      (el instanceof HTMLElement && el.isContentEditable)
    );
  }

  function handleKeyup(event: KeyboardEvent) {
    if (!editor.active) return;
    if (event.key === "Alt") editor.modifierPressed = false;
  }

  function handleWindowBlur() {
    if (!editor.active) return;
    editor.modifierPressed = false;
  }

  // Key sequence state
  let pendingSequence: { firstChord: string; timestamp: number } | null = $state(null);
  const SEQUENCE_TIMEOUT = 500;

  // Quick Add debounce
  let lastQuickAddTime = 0;
  const QUICK_ADD_DEBOUNCE = 200;

  // Action table: id → { handler, skipEditable, preventDefault }
  type ActionEntry = {
    id: string;
    handler: () => void;
    skipEditable?: boolean;
    preventDefault?: boolean;
    /** Allow Ctrl+C to pass through when text is selected */
    passThroughOnSelection?: boolean;
  };

  const actions: ActionEntry[] = [
    {
      id: "editor.save",
      handler: () => editor.savePreset(),
      preventDefault: true,
    },
    {
      id: "editor.toggle_run",
      handler: () => {
        if (editor.running) editor.stopPreset();
        else editor.startPreset();
      },
      preventDefault: true,
    },
    {
      id: "editor.cut",
      handler: () => editor.cutNodesAndEdges(),
      skipEditable: true,
    },
    {
      id: "editor.copy",
      handler: () => editor.copyNodesAndEdges(),
      skipEditable: true,
      passThroughOnSelection: true,
    },
    {
      id: "editor.paste",
      handler: () => editor.pasteNodesAndEdges(),
      skipEditable: true,
    },
    {
      id: "editor.select_all",
      handler: () => editor.selectAllNodesAndEdges(),
      skipEditable: true,
      preventDefault: true,
    },
    {
      id: "editor.add_agent",
      handler: () => editor.showAgentList(mouseX, mouseY),
      skipEditable: true,
      preventDefault: true,
    },
    {
      id: "editor.undo",
      handler: () => {
        if (!editor.history.executing) editor.undo();
      },
      skipEditable: true,
      preventDefault: true,
    },
    {
      id: "editor.redo",
      handler: () => {
        if (!editor.history.executing) editor.redo();
      },
      skipEditable: true,
      preventDefault: true,
    },
    {
      id: "editor.toggle_grid",
      handler: () => editor.toggleGrid(),
      skipEditable: true,
      preventDefault: true,
    },
    // Quick Add entries are handled separately
  ];

  // Build quick add action entries from resolved hotkeys
  const quickAddIds = [...quickAddAgents.keys()];

  function handleKeydown(event: KeyboardEvent) {
    if (!editor.active) return;
    // Skip if already handled by +layout.svelte (e.g. fullscreen toggle)
    if (event.defaultPrevented) return;

    // Escape: close popups (hardcoded)
    if (event.key === "Escape") {
      if (editor.openAgentList) {
        editor.hideAgentList();
        return;
      }
      return;
    }

    // Alt: snap modifier (hardcoded)
    if (event.key === "Alt") {
      editor.modifierPressed = true;
      return;
    }

    // Ctrl+R: prevent browser refresh (hardcoded)
    if (event.key === "r" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      return;
    }

    const editable = isEditableElement(event.target);
    const now = Date.now();

    // --- Sequence handling ---
    const pending = pendingSequence;
    if (pending) {
      const elapsed = now - pending.timestamp;
      if (elapsed < SEQUENCE_TIMEOUT) {
        // Check all hotkeys for a sequence whose first chord matches pending
        for (const h of hotkeys) {
          if (!isSequence(h.key)) continue;
          if (getSequenceFirst(h.key) !== pending.firstChord) continue;
          // Match the second chord
          const secondChord = getSequenceSecond(h.key);
          if (!matchHotkey(event, secondChord)) continue;

          // Found a sequence match
          pendingSequence = null;

          // Find action or quick add
          const action = actions.find((a) => a.id === h.id);
          if (action) {
            if (action.skipEditable && editable) return;
            if (action.preventDefault) event.preventDefault();
            action.handler();
            return;
          }
          if (quickAddIds.includes(h.id)) {
            if (editable) return;
            handleQuickAdd(h.id, now);
            event.preventDefault();
            return;
          }
        }
      }
      // Timeout or no match — reset
      pendingSequence = null;
    }

    // --- Check if any sequence starts with this keypress ---
    let sequenceCandidate = false;
    for (const h of hotkeys) {
      if (!isSequence(h.key)) continue;
      if (matchFirstChord(event, getSequenceFirst(h.key))) {
        sequenceCandidate = true;
        break;
      }
    }

    if (sequenceCandidate) {
      // Build chord string for pending state
      const parts: string[] = [];
      if (event.ctrlKey) parts.push("ctrl");
      if (event.metaKey) parts.push("meta");
      if (event.shiftKey) parts.push("shift");
      parts.push(event.key.toLowerCase());
      pendingSequence = { firstChord: parts.join("+"), timestamp: now };
      event.preventDefault();
      return;
    }

    // --- Single-chord matching ---

    // Check standard actions
    for (const action of actions) {
      const key = getHotkeyKey(hotkeys, action.id);
      if (!key || isSequence(key)) continue;
      if (!matchHotkey(event, key)) continue;

      if (action.skipEditable && editable) return;

      // Special: Ctrl+C pass-through when text is selected
      if (action.passThroughOnSelection) {
        const selection = window.getSelection();
        if (selection && !selection.isCollapsed) return;
      }

      if (action.preventDefault) event.preventDefault();
      action.handler();
      return;
    }

    // Check quick add actions
    for (const qaId of quickAddIds) {
      const key = getHotkeyKey(hotkeys, qaId);
      if (!key || isSequence(key)) continue;
      if (!matchHotkey(event, key)) continue;
      if (editable) return;
      handleQuickAdd(qaId, now);
      event.preventDefault();
      return;
    }
  }

  function handleQuickAdd(actionId: string, now: number) {
    if (now - lastQuickAddTime < QUICK_ADD_DEBOUNCE) return;
    lastQuickAddTime = now;
    const agentName = quickAddAgents.get(actionId);
    if (agentName) {
      editor.addAgent(agentName, { x: mouseX, y: mouseY });
    }
  }

  // --- Agent list popup ---

  let agentListRef: HTMLDivElement | null = $state(null);

  function handlePaneDblClick(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (
      target.closest(".svelte-flow__node") ||
      target.closest(".svelte-flow__controls") ||
      target.closest(".svelte-flow__minimap") ||
      target.closest(".svelte-flow__edge")
    )
      return;
    editor.showAgentList(event.clientX, event.clientY);
  }

  async function handleAddAgentFromPopup(name: string) {
    await editor.addAgent(name, { x: editor.agentListOriginX, y: editor.agentListOriginY });
    editor.hideAgentList();
  }

  function handleWindowMouseDown(event: MouseEvent) {
    if (!editor.active) return;
    if (editor.openAgentList && !agentListRef?.contains(event.target as Node)) {
      editor.hideAgentList();
    }
  }

  // --- Canvas event handlers ---

  const handleOnDelete: OnDelete<PresetNode, PresetEdge> = async ({ nodes, edges }) => {
    await editor.handleOnDelete({ nodes, edges });
  };

  function handleBeforeConnect(connection: Connection): PresetEdge {
    const color = getEdgeColor(connection.sourceHandle);
    return {
      id: crypto.randomUUID(),
      ...connection,
      ...(color ? { style: `stroke: ${color};` } : {}),
    };
  }

  async function handleOnConnect(connection: Connection) {
    await editor.handleOnConnect(connection);
  }

  const handleNodeContextMenu: NodeEventWithPointer<MouseEvent, PresetNode> = ({ event, node }) => {
    event.preventDefault();

    const [selectedNodes] = editor.selectedNodesAndEdges();
    if (!selectedNodes.some((n) => n.id === node.id)) {
      editor.nodes.forEach((n) => {
        editor.props.svelteFlow.updateNode(n.id, { selected: false });
      });
      editor.props.svelteFlow.updateNode(node.id, { selected: true });
    }

    editor.showNodeContextMenu(event.clientX, event.clientY);
  };

  function handleSelectionContextMenu({ event }: { event: MouseEvent }) {
    event.preventDefault();
    editor.showNodeContextMenu(event.clientX, event.clientY);
  }

  function handleNodeClick() {
    editor.hideNodeContextMenu();
    editor.hideAgentList();
  }

  function handleNodeDragStart({ nodes }: { nodes: PresetNode[] }) {
    editor.handleNodeDragStart(nodes);
  }

  function handleSelectionDragStart(_event: MouseEvent | TouchEvent, nodes: PresetNode[]) {
    editor.handleNodeDragStart(nodes);
  }

  const handleNodeDragStop: NodeTargetEventWithPointer<
    MouseEvent | TouchEvent,
    PresetNode
  > = async ({ targetNode }) => {
    await editor.handleNodeDragStop(targetNode ?? null);
    updateMiniMapVisibility();
  };

  const handleSelectionDragStop: OnSelectionDrag = async (_event, draggedNodes) => {
    await editor.handleSelectionDragStop(draggedNodes as PresetNode[]);
    updateMiniMapVisibility();
  };

  const handleOnMoveEnd: OnMove = async (_event, viewport) => {
    await editor.handleOnMoveEnd(viewport);
    updateMiniMapVisibility();
  };

  function handleSelectionClick() {
    editor.hideNodeContextMenu();
  }

  function handlePaneClick() {
    editor.hideNodeContextMenu();
    editor.hidePaneContextMenu();
    editor.hideAgentList();
  }

  function handlePaneContextMenu({ event }: { event: MouseEvent }) {
    event.preventDefault();
    editor.showPaneContextMenu(event.clientX, event.clientY);
  }

  async function handleSaveAsSubmit() {
    editor.openSaveAsDialog = false;
    if (!editor.saveAsName) return;
    if (editor.saveAsName === editor.name) {
      await editor.savePreset();
      return;
    }
    const s = await getPresetSpec(editor.preset_id);
    if (!s) return;
    const new_id = await addPresetWithName(s, editor.saveAsName);
    if (!new_id) return;
    tabStore.openTab(new_id, editor.saveAsName);
    goto(`/preset_editor/${new_id}`, { noScroll: true });
  }
</script>

<svelte:window
  onkeydown={handleKeydown}
  onkeyup={handleKeyup}
  onblur={handleWindowBlur}
  onmousedown={handleWindowMouseDown}
  onmousemove={handleMouseMove}
/>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div bind:this={canvasContainer} class="flex-1 w-full" ondblclick={handlePaneDblClick}>
  <SvelteFlow
    attributionPosition="bottom-right"
    class="flex-1 w-full"
    colorMode={(coreSettings.color_mode as "light" | "dark" | "system") || "system"}
    connectionRadius={38}
    deleteKey={["Delete", "Backspace"]}
    bind:edges={editor.edges}
    initialViewport={editor.props.flow().viewport!}
    maxZoom={2}
    minZoom={0.1}
    bind:nodes={editor.nodes}
    {nodeTypes}
    onbeforeconnect={handleBeforeConnect}
    onconnect={handleOnConnect}
    ondelete={handleOnDelete}
    onnodeclick={handleNodeClick}
    onnodedragstart={handleNodeDragStart}
    onnodedragstop={handleNodeDragStop}
    onnodecontextmenu={handleNodeContextMenu}
    onmoveend={handleOnMoveEnd}
    onpaneclick={handlePaneClick}
    onpanecontextmenu={handlePaneContextMenu}
    onselectionclick={handleSelectionClick}
    onselectioncontextmenu={handleSelectionContextMenu}
    onselectiondragstart={handleSelectionDragStart}
    onselectiondragstop={handleSelectionDragStop}
    snapGrid={editor.effectiveSnapGrid}
    zoomOnDoubleClick={false}
  >
    <Background
      bgColor={editor.running ? "var(--color-background)" : "var(--color-muted)"}
      gap={editor.gridGap}
      lineWidth={1}
      variant={BackgroundVariant.Dots}
      patternColor={editor.showGrid ? undefined : "transparent"}
    />

    <Controls>
      {#snippet before()}
        <ControlButton
          onclick={() => editor.toggleSnap()}
          title="Snap (Alt)"
          class={editor.snapEnabled ? undefined : "icon-slashed"}
        >
          <img
            src="data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxNiIgaGVpZ2h0PSIxNiIgdmlld0JveD0iMCAwIDI0IDI0IiBmaWxsPSJub25lIiBzdHJva2U9IiNmZmZmZmYiIHN0cm9rZS13aWR0aD0iMiIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIiBjbGFzcz0ibHVjaWRlIGx1Y2lkZS1tYWduZXQtaWNvbiBsdWNpZGUtbWFnbmV0Ij48cGF0aCBkPSJtMTIgMTUgNCA0Ii8+PHBhdGggZD0iTTIuMzUyIDEwLjY0OGExLjIwNSAxLjIwNSAwIDAgMCAwIDEuNzA0bDIuMjk2IDIuMjk2YTEuMjA1IDEuMjA1IDAgMCAwIDEuNzA0IDBsNi4wMjktNi4wMjlhMSAxIDAgMSAxIDMgM2wtNi4wMjkgNi4wMjlhMS4yMDUgMS4yMDUgMCAwIDAgMCAxLjcwNGwyLjI5NiAyLjI5NmExLjIwNSAxLjIwNSAwIDAgMCAxLjcwNCAwbDYuMzY1LTYuMzY3QTEgMSAwIDAgMCA4LjcxNiA0LjI4MnoiLz48cGF0aCBkPSJtNSA4IDQgNCIvPjwvc3ZnPg=="
            alt="snap"
          />
        </ControlButton>
        <ControlButton
          onclick={() => editor.toggleGrid()}
          title="Grid ({formatHotkey(getHotkeyKey(hotkeys, 'editor.toggle_grid'))})"
          class={editor.showGrid ? undefined : "icon-slashed"}
        >
          <img
            src="data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxNiIgaGVpZ2h0PSIxNiIgdmlld0JveD0iMCAwIDI0IDI0IiBmaWxsPSJub25lIiBzdHJva2U9IiNmZmZmZmYiIHN0cm9rZS13aWR0aD0iMiIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIiBjbGFzcz0ibHVjaWRlIGx1Y2lkZS1ncmlkM3gzLWljb24gbHVjaWRlLWdyaWQtM3gzIj48cmVjdCB3aWR0aD0iMTgiIGhlaWdodD0iMTgiIHg9IjMiIHk9IjMiIHJ4PSIyIi8+PHBhdGggZD0iTTMgOWgxOCIvPjxwYXRoIGQ9Ik0zIDE1aDE4Ii8+PHBhdGggZD0iTTkgM3YxOCIvPjxwYXRoIGQ9Ik0xNSAzdjE4Ii8+PC9zdmc+"
            alt="grid"
          />
        </ControlButton>
      {/snippet}
    </Controls>
    {#if showMiniMap}
      <div transition:fade={{ duration: 150 }}>
        <MiniMap />
      </div>
    {/if}

    <NodeContextMenu
      bind:open={editor.openNodeContextMenu}
      x={editor.nodeContextMenuX}
      y={editor.nodeContextMenuY}
      selectedCount={editor.selectedCount}
      {hotkeys}
      onenable={() => editor.enable()}
      ondisable={() => editor.disable()}
      oncut={() => editor.cutNodesAndEdges()}
      oncopy={() => editor.copyNodesAndEdges()}
      ontoggleerr={() => editor.toggleErr()}
      onalign={(d) => editor.alignNodes(d)}
      ondistribute={(d) => editor.distributeNodes(d)}
    />

    <PaneContextMenu
      bind:open={editor.openPaneContextMenu}
      x={editor.paneContextMenuX}
      y={editor.paneContextMenuY}
      running={editor.running}
      snapEnabled={editor.snapEnabled}
      showGrid={editor.showGrid}
      {hotkeys}
      onstart={() => editor.startPreset()}
      onstop={() => editor.stopPreset()}
      onnew={() => editor.showNewPresetDialog()}
      onsave={() => editor.savePreset()}
      onsaveas={() => editor.showSaveAsDialog()}
      onimport={() => editor.importPresetAndNavigate()}
      onexport={() => editor.exportPreset()}
      canUndo={editor.history.canUndo}
      canRedo={editor.history.canRedo}
      onundo={() => editor.undo()}
      onredo={() => editor.redo()}
      onpaste={() => editor.pasteNodesAndEdges()}
      onaddagent={() => editor.showAgentList(mouseX, mouseY)}
      ontogglesnap={() => editor.toggleSnap()}
      ontogglegrid={() => editor.toggleGrid()}
    />
  </SvelteFlow>
</div>

<div
  bind:this={agentListRef}
  class="fixed z-50 w-64 rounded-md border shadow-lg bg-popover text-popover-foreground"
  class:hidden={!editor.openAgentList}
  style="left: {editor.agentListX}px; top: {editor.agentListY}px;"
>
  <AgentList onAddAgent={handleAddAgentFromPopup} visible={editor.openAgentList} />
</div>

<PresetActionDialog
  action="New"
  name={editor.name}
  onAction={(name) => editor.newPresetAndNavigate(name)}
  bind:open={editor.openNewPresetDialog}
/>

<Dialog.Root bind:open={editor.openSaveAsDialog}>
  <Dialog.Content class="sm:max-w-[425px]">
    <form
      onsubmit={(e) => {
        e.preventDefault();
        handleSaveAsSubmit();
      }}
    >
      <Dialog.Header>
        <Dialog.Title>Save As...</Dialog.Title>
      </Dialog.Header>
      <div class="mt-4 mb-4">
        <Input id="save-as-name" name="name" bind:value={editor.saveAsName} />
      </div>
      <Dialog.Footer>
        <Button type="submit">Save</Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>

<style>
  :root {
    --resize-control-size: 2px;
    --resize-control-color: var(--color-ring);
  }

  :global(.svelte-flow__edge .svelte-flow__edge-path) {
    stroke-width: 8px;
  }

  :global(.svelte-flow__resize-control.handle) {
    border: calc(var(--resize-control-size) * 1.5) solid var(--resize-control-color);
    border-radius: var(--resize-control-size);
    width: 0px;
    height: 0px;
  }
  :global(.svelte-flow__resize-control.line.top) {
    border: var(--resize-control-size) solid var(--resize-control-color);
  }
  :global(.svelte-flow__resize-control.line.right) {
    border: var(--resize-control-size) solid var(--resize-control-color);
  }
  :global(.svelte-flow__resize-control.line.bottom) {
    border: var(--resize-control-size) solid var(--resize-control-color);
  }
  :global(.svelte-flow__resize-control.line.left) {
    border: var(--resize-control-size) solid var(--resize-control-color);
  }
  :global(.svelte-flow__controls) {
    margin-left: 8px;
    margin-bottom: 8px;
  }
  :global(.svelte-flow__minimap) {
    margin-right: 8px;
    margin-bottom: 8px;
  }
  :global(.svelte-flow__attribution) {
    z-index: 20;
  }
  :global(.svelte-flow__controls-button svg.size-4) {
    max-width: 16px;
    max-height: 16px;
  }
  :global(.svelte-flow__controls-button.icon-slashed) {
    position: relative;
    opacity: 0.5;
  }
  :global(.svelte-flow__controls-button.icon-slashed::after) {
    content: "";
    position: absolute;
    top: 50%;
    left: 50%;
    width: 80%;
    height: 0;
    border-top: 1.5px solid currentColor;
    transform: translate(-50%, -50%) rotate(-45deg);
    pointer-events: none;
  }
</style>
