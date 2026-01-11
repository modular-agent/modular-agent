<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";

  import { onMount } from "svelte";

  import {
    Background,
    BackgroundVariant,
    Controls,
    MiniMap,
    SvelteFlow,
    useSvelteFlow,
    type Connection,
    type NodeEventWithPointer,
    type NodeTargetEventWithPointer,
    type NodeTypes,
    type OnDelete,
    type OnMove,
  } from "@xyflow/svelte";
  // 👇 this is important! You need to import the styles for Svelte Flow to work
  import "@xyflow/svelte/dist/style.css";
  import hotkeys from "hotkeys-js";
  import {
    addAgent,
    addAgentsAndChannels,
    addAgentStream,
    addChannel,
    getAgentSpec,
    getAgentStreamSpec,
    newAgentSpec,
    removeAgent,
    removeChannel,
    startAgent,
    stopAgent,
    uniqueStreamName,
    updateAgentSpec,
    updateAgentStreamSpec,
  } from "tauri-plugin-askit-api";
  import type { AgentSpec, ChannelSpec } from "tauri-plugin-askit-api";

  import { goto } from "$app/navigation";

  import {
    agentSpecToNode,
    channelSpecToEdge,
    edgeToChannelSpec,
    saveAgentStream,
    getCoreSettings,
  } from "$lib/agent";
  import FlowStatus from "$lib/components/flow-status.svelte";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import { importStream, newStream, startStream, stopStream } from "$lib/shared.svelte";
  import type { AgentStreamNode, AgentStreamEdge } from "$lib/types";

  import type { PageProps } from "./$types";
  import AgentList from "./agent-list.svelte";
  import AgentNode from "./agent-node.svelte";
  import Menubar from "./menubar.svelte";
  import NodeContextMenu from "./node-context-menu.svelte";
  import StreamActions from "./stream-actions.svelte";
  import StreamName from "./stream-name.svelte";

  const { screenToFlowPosition, updateEdge, updateNode, updateNodeData } =
    $derived(useSvelteFlow());

  const coreSettings = getCoreSettings();

  const nodeTypes: NodeTypes = {
    agent: AgentNode,
  };

  let { data }: PageProps = $props();

  let stream_id = $derived(data.stream_id);
  let running = $derived(data.flow?.running ?? false);

  let nodes = $state.raw<AgentStreamNode[]>([]);
  let edges = $state.raw<AgentStreamEdge[]>([]);

  $effect.pre(() => {
    nodes = [...data.flow.nodes];
    edges = [...data.flow.edges];

    getCurrentWindow().setTitle(data.flow.name + " - Agent Stream App");
  });

  const handleOnDelete: OnDelete<AgentStreamNode, AgentStreamEdge> = async ({
    nodes: deletedNodes,
    edges: deletedEdges,
  }) => {
    if (deletedEdges && deletedEdges.length > 0) {
      await deleteEdges(deletedEdges);
    }
    if (deletedNodes && deletedNodes.length > 0) {
      await deleteNodes(deletedNodes);
    }
  };

  async function deleteNodes(deletedNodes: AgentStreamNode[]) {
    for (const n of deletedNodes) {
      await removeAgent(stream_id, n.id);
    }
  }

  async function deleteEdges(deletedEdges: AgentStreamEdge[]) {
    for (const e of deletedEdges) {
      let ch = edgeToChannelSpec(e);
      await removeChannel(stream_id, ch);
    }
  }

  async function handleOnConnect(connection: Connection) {
    let edge = {
      id: crypto.randomUUID(),
      ...connection,
    } as AgentStreamEdge;

    await addChannel(stream_id, edgeToChannelSpec(edge));
  }

  // cut, copy and paste

  let copiedNodes = $state.raw<AgentSpec[]>([]);
  let copiedEdges = $state.raw<ChannelSpec[]>([]);

  function selectedNodesAndEdges(): [AgentStreamNode[], AgentStreamEdge[]] {
    const selectedNodes = nodes.filter((node) => node.selected);
    const selectedEdges = edges.filter((edge) => edge.selected);
    return [selectedNodes, selectedEdges];
  }

  async function cutNodesAndEdges() {
    const [selectedNodes, selectedEdges] = selectedNodesAndEdges();
    if (selectedNodes.length == 0 && selectedEdges.length == 0) {
      return;
    }
    copiedNodes = (
      await Promise.all(selectedNodes.map(async (node) => await getAgentSpec(node.id)))
    ).filter((spec) => spec !== null);
    copiedEdges = selectedEdges.map((edge) => edgeToChannelSpec(edge));

    for (const edge of selectedEdges) {
      let ch = edgeToChannelSpec(edge);
      await removeChannel(stream_id, ch);
    }
    for (const node of selectedNodes) {
      await removeAgent(stream_id, node.id);
    }
    nodes = nodes.filter((node) => !node.selected);
    edges = edges.filter((edge) => !edge.selected);
  }

  async function copyNodesAndEdges() {
    const [selectedNodes, selectedEdges] = selectedNodesAndEdges();
    if (selectedNodes.length == 0) {
      return;
    }
    copiedNodes = (
      await Promise.all(selectedNodes.map(async (node) => await getAgentSpec(node.id)))
    ).filter((spec) => spec !== null);
    copiedEdges = selectedEdges.map((edge) => edgeToChannelSpec(edge));
  }

  async function pasteNodesAndEdges() {
    nodes.forEach((node) => {
      if (node.selected) {
        updateNode(node.id, { selected: false });
      }
    });
    edges.forEach((edge) => {
      if (edge.selected) {
        updateEdge(edge.id, { selected: false });
      }
    });

    if (copiedNodes.length == 0) {
      return;
    }

    let [added_agents, added_edges] = await addAgentsAndChannels(
      stream_id,
      copiedNodes,
      copiedEdges,
    );

    if (added_agents.length == 0 && added_edges.length == 0) return;

    let new_nodes = [];
    for (const a of added_agents) {
      a.x += 80;
      a.y += 80;
      const new_node = agentSpecToNode(a);
      new_node.selected = true;
      new_nodes.push(new_node);
    }

    let new_edges = [];
    for (const edge of added_edges) {
      const new_edge = channelSpecToEdge(edge);
      new_edge.selected = true;
      new_edges.push(new_edge);
    }

    nodes = [...nodes, ...new_nodes];
    edges = [...edges, ...new_edges];

    if (running) {
      for (const node of new_nodes) {
        if (node.data.disabled) continue;
        await startAgent(node.id);
      }
    }
  }

  function selectAllNodesAndEdges() {
    nodes.forEach((node) => {
      updateNode(node.id, { selected: true });
    });
    edges.forEach((edge) => {
      updateEdge(edge.id, { selected: true });
    });
  }

  // shortcuts

  onMount(() => {
    hotkeys("ctrl+r, command+r", (event) => {
      event.preventDefault();
    });

    hotkeys("ctrl+s, command+s", (event) => {
      event.preventDefault();
      onSaveStream();
    });

    hotkeys("ctrl+x, command+x", () => {
      /* await */ cutNodesAndEdges();
    });
    hotkeys("ctrl+c, command+c", () => {
      copyNodesAndEdges();
    });
    hotkeys("ctrl+v, command+v", () => {
      pasteNodesAndEdges();
    });
    hotkeys("ctrl+a, command+a", (ev) => {
      ev.preventDefault();
      selectAllNodesAndEdges();
    });

    hotkeys("ctrl+., command+.", (ev) => {
      ev.preventDefault();
      if (running) {
        onStopStream();
      } else {
        onStartStream();
      }
    });

    return () => {
      hotkeys.unbind("ctrl+r");
      hotkeys.unbind("command+r");
      hotkeys.unbind("ctrl+s");
      hotkeys.unbind("command+s");
      hotkeys.unbind("ctrl+x");
      hotkeys.unbind("command+x");
      hotkeys.unbind("ctrl+c");
      hotkeys.unbind("command+c");
      hotkeys.unbind("ctrl+v");
      hotkeys.unbind("command+v");
      hotkeys.unbind("ctrl+a");
      hotkeys.unbind("command+a");
      hotkeys.unbind("ctrl+.");
      hotkeys.unbind("command+.");
    };
  });

  async function onNewStream(name: string) {
    const new_id = await newStream(name);
    if (new_id) {
      goto(`/stream_editor/${new_id}`, { invalidateAll: true });
    }
  }

  async function onSaveStream() {
    const s = await getAgentStreamSpec(stream_id);
    if (!s) return;
    await saveAgentStream(data.flow.name, s);
  }

  async function onDuplicateStream() {
    const s = await getAgentStreamSpec(stream_id);
    if (!s) return;
    const new_name = await uniqueStreamName(data.flow.name);
    const new_id = await addAgentStream(new_name, s);
    if (new_id) {
      goto(`/stream_editor/${new_id}`, { invalidateAll: true });
    }
  }

  async function onExportStream() {
    const s = await getAgentStreamSpec(stream_id);
    const jsonStr = JSON.stringify(s, null, 2);
    const blob = new Blob([jsonStr], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = data.flow.name + ".json";
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    // TODO: show a toast notification
  }

  async function onImportStream() {
    const file = await open({ multiple: false, filter: "json" });
    if (!file) return;
    const id = await importStream(file);
    goto(`/stream_editor/${id}`, { invalidateAll: true });
  }

  async function onStartStream() {
    await startStream(stream_id);
    running = true;
  }

  async function onStopStream() {
    await stopStream(stream_id);
    running = false;
  }

  const AGENT_DRAG_FORMAT = "application/agent-name";

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
    await onAddAgent(agentName, { x: event.clientX - 40, y: event.clientY - 20 });
  }

  async function onAddAgent(agent_name: string, position?: { x: number; y: number }) {
    const snode = await newAgentSpec(agent_name);
    const xy =
      position !== undefined
        ? screenToFlowPosition(position)
        : screenToFlowPosition({
            x: window.innerWidth * 0.45,
            y: window.innerHeight * 0.3,
          });
    snode.x = xy.x;
    snode.y = xy.y;
    const id = await addAgent(stream_id, snode);
    snode.id = id;
    const new_node = agentSpecToNode(snode);
    nodes = [...nodes, new_node];

    if (running) {
      await startAgent(new_node.id);
    }
  }

  async function onEnable() {
    const [selectedNodes, selectedEdges] = selectedNodesAndEdges();
    if (selectedNodes.length > 0 || selectedEdges.length > 0) {
      // enable selected agents
      for (const node of selectedNodes) {
        if (node.data.disabled) {
          updateNodeData(node.id, { disabled: false });
          await startAgent(node.id);
        }
      }
      return;
    }
  }

  async function onDisable() {
    const [selectedNodes, selectedEdges] = selectedNodesAndEdges();
    if (selectedNodes.length > 0 || selectedEdges.length > 0) {
      // disable selected agents
      for (const node of selectedNodes) {
        if (!node.data.disabled) {
          updateNodeData(node.id, { disabled: true });
          await stopAgent(node.id);
        }
      }
      return;
    }
  }

  async function onToggleErr() {
    const [selectedNodes, _] = selectedNodesAndEdges();
    if (selectedNodes.length == 0) {
      return;
    }
    selectedNodes.forEach((node) => {
      updateNodeData(node.id, { show_err: !node.data.show_err });
    });
  }

  let openNodeContextMenu = $state(false);
  let nodeContextMenuX = $state(0);
  let nodeContextMenuY = $state(0);

  function showNodeContextMenu(x: number, y: number) {
    nodeContextMenuX = x;
    nodeContextMenuY = y;
    openNodeContextMenu = true;
  }

  function hideNodeContextMenu() {
    openNodeContextMenu = false;
  }

  const handleNodeContextMenu: NodeEventWithPointer<MouseEvent, AgentStreamNode> = ({
    event,
    node,
  }) => {
    event.preventDefault();

    const [selectedNodes, _] = selectedNodesAndEdges();
    if (!selectedNodes.some((n) => n.id === node.id)) {
      nodes.forEach((n) => {
        updateNode(n.id, { selected: false });
      });
      updateNode(node.id, { selected: true });
    }

    showNodeContextMenu(event.clientX, event.clientY);
  };

  function handleSelectionContextMenu({ event }: { event: MouseEvent }) {
    event.preventDefault();
    showNodeContextMenu(event.clientX, event.clientY);
  }

  function handleNodeClick() {
    hideNodeContextMenu();
  }

  const handleNodeDragStop: NodeTargetEventWithPointer<
    MouseEvent | TouchEvent,
    AgentStreamNode
  > = async ({ targetNode }) => {
    if (!targetNode) return;
    await updateAgentSpec(targetNode.id, {
      x: targetNode.position.x,
      y: targetNode.position.y,
    });
  };

  const handleOnMoveEnd: OnMove = async (_event, viewport) => {
    await updateAgentStreamSpec(stream_id, { viewport });
  };

  function handleSelectionClick() {
    hideNodeContextMenu();
  }

  function handlePaneClick() {
    hideNodeContextMenu();
  }
</script>

<div class="flex flex-col w-full min-h-screen">
  <header class="grid grid-cols-[auto_1fr_100px] flex-none items-center pl-1 pr-2 gap-4">
    <div class="justify-self-start">
      <Menubar
        {stream_id}
        name={data.flow?.name}
        {onNewStream}
        {onSaveStream}
        {onImportStream}
        {onExportStream}
      />
    </div>
    <div class="flex flex-row items-center justify-center">
      <StreamName name={data.flow?.name} class="mr-4" />
      <StreamActions {running} {onStartStream} {onStopStream} />
    </div>
    <div class="justify-self-end">
      <FlowStatus {running} run_on_start={data.flow?.run_on_start} />
    </div>
  </header>
  <SvelteFlow
    attributionPosition="bottom-right"
    class="flex-1 w-full"
    colorMode={(coreSettings.color_mode as "light" | "dark" | "system") || "system"}
    connectionRadius={38}
    deleteKey={["Delete", "Backspace"]}
    bind:edges
    initialViewport={data.flow?.viewport!}
    maxZoom={2}
    minZoom={0.1}
    bind:nodes
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
    snapGrid={[6, 6]}
  >
    <Background variant={BackgroundVariant.Dots} gap={24} lineWidth={1} />

    <Controls />
    <MiniMap />

    <NodeContextMenu
      bind:open={openNodeContextMenu}
      x={nodeContextMenuX}
      y={nodeContextMenuY}
      onenable={onEnable}
      ondisable={onDisable}
      oncut={cutNodesAndEdges}
      oncopy={copyNodesAndEdges}
      ontoggleerr={onToggleErr}
    />

    <div
      class="absolute right-2 top-2 w-60 z-20 max-h-[calc(100vh-210px)] overflow-x-hidden rounded-md border shadow-lg"
    >
      <ScrollArea>
        <AgentList class="h-full" {onAddAgent} onDragAgentStart={handleAgentDragStart} />
      </ScrollArea>
    </div>
  </SvelteFlow>
</div>

<style>
  :root {
    --resize-control-size: 2px;
    --resize-control-color: var(--color-teal-500);
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
