<script lang="ts">
  import { fade } from "svelte/transition";

  import {
    Background,
    BackgroundVariant,
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

  import { getCoreSettings } from "$lib/agent";
  import { AgentList } from "$lib/components/agent-list/index.js";
  import PresetActionDialog from "$lib/components/preset-action-dialog.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import type { PresetNode, PresetEdge } from "$lib/types";

  import AgentNode from "./agent-node.svelte";
  import { useEditor } from "./context.svelte";
  import NodeContextMenu from "./node-context-menu.svelte";
  import PaneContextMenu from "./pane-context-menu.svelte";

  const editor = useEditor();
  const coreSettings = getCoreSettings();

  const nodeTypes: NodeTypes = {
    agent: AgentNode,
  };

  const AGENT_DRAG_FORMAT = "application/agent-name";

  // --- MiniMap visibility ---

  let showMiniMap = $state(false);
  const HIDE_MARGIN = 50;

  function updateMiniMapVisibility() {
    const nodes = editor.nodes;
    if (nodes.length === 0) {
      showMiniMap = false;
      return;
    }

    const container = document.querySelector(".svelte-flow");
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

  // --- Keyboard shortcuts ---

  function handleKeydown(event: KeyboardEvent) {
    const mod = event.ctrlKey || event.metaKey;
    if (!mod) return;
    switch (event.key) {
      case "r":
        event.preventDefault();
        break;
      case "s":
        event.preventDefault();
        editor.savePreset();
        break;
      case "x":
        editor.cutNodesAndEdges();
        break;
      case "c":
        editor.copyNodesAndEdges();
        break;
      case "v":
        editor.pasteNodesAndEdges();
        break;
      case "a":
        event.preventDefault();
        editor.selectAllNodesAndEdges();
        break;
      case ".":
        event.preventDefault();
        if (editor.running) {
          editor.stopPreset();
        } else {
          editor.startPreset();
        }
        break;
    }
  }

  // --- Drag and drop ---

  function handleAgentDragStart(event: DragEvent, agentName: string) {
    event.dataTransfer?.setData(AGENT_DRAG_FORMAT, agentName);
    event.dataTransfer?.setData("text/plain", agentName);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
    }
  }

  function handleDragOver(event: DragEvent) {
    const hasAgent = event.dataTransfer?.types?.includes(AGENT_DRAG_FORMAT);
    if (!hasAgent) return;
    event.preventDefault();
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = "move";
    }
  }

  async function handleDrop(event: DragEvent) {
    const hasAgent = event.dataTransfer?.types?.includes(AGENT_DRAG_FORMAT);
    if (!hasAgent) return;
    event.preventDefault();
    const agentName = event.dataTransfer?.getData(AGENT_DRAG_FORMAT);
    if (!agentName) return;
    await editor.addAgent(agentName, { x: event.clientX - 40, y: event.clientY - 20 });
  }

  // --- Canvas event handlers ---

  const handleOnDelete: OnDelete<PresetNode, PresetEdge> = async ({ nodes, edges }) => {
    await editor.handleOnDelete({ nodes, edges });
  };

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
    goto(`/preset_editor/${new_id}`, { invalidateAll: true });
  }
</script>

<svelte:window onkeydown={handleKeydown} />

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
  onconnect={handleOnConnect}
  ondelete={handleOnDelete}
  ondragover={handleDragOver}
  ondrop={handleDrop}
  onnodeclick={handleNodeClick}
  onnodedragstop={handleNodeDragStop}
  onnodecontextmenu={handleNodeContextMenu}
  onmoveend={handleOnMoveEnd}
  onpaneclick={handlePaneClick}
  onpanecontextmenu={handlePaneContextMenu}
  onselectionclick={handleSelectionClick}
  onselectioncontextmenu={handleSelectionContextMenu}
  onselectiondragstop={handleSelectionDragStop}
  snapGrid={[6, 6]}
>
  <Background
    bgColor={editor.running ? "var(--color-background)" : "var(--color-muted)"}
    gap={24}
    lineWidth={1}
    variant={BackgroundVariant.Dots}
  />

  <Controls />
  {#if showMiniMap}
    <div transition:fade={{ duration: 150 }}>
      <MiniMap />
    </div>
  {/if}

  <NodeContextMenu
    bind:open={editor.openNodeContextMenu}
    x={editor.nodeContextMenuX}
    y={editor.nodeContextMenuY}
    onenable={() => editor.enable()}
    ondisable={() => editor.disable()}
    oncut={() => editor.cutNodesAndEdges()}
    oncopy={() => editor.copyNodesAndEdges()}
    ontoggleerr={() => editor.toggleErr()}
  />

  <PaneContextMenu
    bind:open={editor.openPaneContextMenu}
    x={editor.paneContextMenuX}
    y={editor.paneContextMenuY}
    running={editor.running}
    onstart={() => editor.startPreset()}
    onstop={() => editor.stopPreset()}
    onnew={() => editor.showNewPresetDialog()}
    onsave={() => editor.savePreset()}
    onsaveas={() => editor.showSaveAsDialog()}
    onimport={() => editor.importPresetAndNavigate()}
    onexport={() => editor.exportPreset()}
    onpaste={() => editor.pasteNodesAndEdges()}
  />

  <div class="absolute right-2 top-2 w-60 z-20 rounded-md border shadow-lg overflow-hidden">
    <AgentList
      onAddAgent={(name: string) => editor.addAgent(name)}
      onDragAgentStart={handleAgentDragStart}
    />
  </div>
</SvelteFlow>

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
</style>
