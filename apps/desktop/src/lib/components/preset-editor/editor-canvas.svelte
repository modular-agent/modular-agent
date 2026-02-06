<script lang="ts">
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

  import { getCoreSettings } from "$lib/agent";
  import { AgentList } from "$lib/components/agent-list/index.js";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import type { PresetNode, PresetEdge } from "$lib/types";

  import AgentNode from "./agent-node.svelte";
  import NodeContextMenu from "./node-context-menu.svelte";
  import { useEditor } from "./context.svelte";

  const editor = useEditor();
  const coreSettings = getCoreSettings();

  const nodeTypes: NodeTypes = {
    agent: AgentNode,
  };

  const AGENT_DRAG_FORMAT = "application/agent-name";

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

  const handleNodeContextMenu: NodeEventWithPointer<MouseEvent, PresetNode> = ({
    event,
    node,
  }) => {
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
  };

  const handleSelectionDragStop: OnSelectionDrag = async (_event, draggedNodes) => {
    await editor.handleSelectionDragStop(draggedNodes as PresetNode[]);
  };

  const handleOnMoveEnd: OnMove = async (_event, viewport) => {
    await editor.handleOnMoveEnd(viewport);
  };

  function handleSelectionClick() {
    editor.hideNodeContextMenu();
  }

  function handlePaneClick() {
    editor.hideNodeContextMenu();
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
  <MiniMap />

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

  <div
    class="absolute right-2 top-2 w-60 z-20 max-h-[calc(100vh-210px)] overflow-x-hidden rounded-md border shadow-lg"
  >
    <ScrollArea>
      <AgentList
        class="h-full"
        onAddAgent={(name: string) => editor.addAgent(name)}
        onDragAgentStart={handleAgentDragStart}
      />
    </ScrollArea>
  </div>
</SvelteFlow>

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
