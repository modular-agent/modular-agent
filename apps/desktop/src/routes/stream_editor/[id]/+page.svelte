<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";

  import { getContext, onMount } from "svelte";

  import {
    useSvelteFlow,
    Background,
    BackgroundVariant,
    Controls,
    MiniMap,
    SvelteFlow,
    type NodeEventWithPointer,
    type OnDelete,
    type NodeTypes,
  } from "@xyflow/svelte";
  // 👇 this is important! You need to import the styles for Svelte Flow to work
  import "@xyflow/svelte/dist/style.css";
  import hotkeys from "hotkeys-js";
  import {
    addAgent,
    addChannel,
    newAgentSpec,
    removeAgent,
    removeChannel,
    startAgent,
    stopAgent,
    copySubStream,
  } from "tauri-plugin-askit-api";
  import type { AgentSpec, ChannelSpec } from "tauri-plugin-askit-api";

  import { goto } from "$app/navigation";

  import {
    deserializeAgentStream,
    deserializeChannelSpec,
    deserializeAgentStreamNode,
    importAgentStream,
    saveAgentStream,
    serializeAgentStream,
    serializeAgentStreamEdge,
    serializeAgentStreamNode,
    setAgentDefinitionsContext,
  } from "@/lib/agent";
  import type { TAgentStreamNode, TAgentStreamEdge, TAgentStream } from "@/lib/types";

  import AgentList from "./agent-list.svelte";
  import AgentNode from "./agent-node.svelte";
  import Menubar from "./menubar.svelte";
  import NodeContextMenu from "./node-context-menu.svelte";
  import StreamActions from "./stream-actions.svelte";
  import StreamName from "./stream-name.svelte";

  let { data } = $props();

  // FIXME: should we move this to a higher level component?
  $effect.pre(() => {
    setAgentDefinitionsContext(data.agentDefs);
  });

  const agentDefs = $derived(data.agentDefs);
  const streams = getContext<() => Record<string, TAgentStream>>("AgentStreams");

  const stream_id = $derived(data.id);
  const stream = $derived.by(() => streams()[stream_id]);

  const { getViewport, screenToFlowPosition, setViewport, updateEdge, updateNode, updateNodeData } =
    $derived(useSvelteFlow());

  const nodeTypes: NodeTypes = {
    agent: AgentNode,
  };

  let nodes = $state.raw<TAgentStreamNode[]>([]);
  let edges = $state.raw<TAgentStreamEdge[]>([]);

  function updateNodesAndEdges() {
    nodes = [...stream.nodes];
    edges = [...stream.edges];
    const viewport = stream.viewport;
    if (viewport) {
      setViewport(viewport);
    }
  }

  onMount(() => {
    updateNodesAndEdges();

    getCurrentWindow().setTitle(stream.name + " - Agent Stream App");
  });

  const handleOnDelete: OnDelete<TAgentStreamNode, TAgentStreamEdge> = async ({
    nodes: deletedNodes,
    edges: deletedEdges,
  }) => {
    if (deletedEdges && deletedEdges.length > 0) {
      await checkEdgeChange(edges);
    }
    if (deletedNodes && deletedNodes.length > 0) {
      await checkNodeChange(nodes);
    }
  };

  async function handleOnConnect() {
    await checkEdgeChange(edges);
  }

  async function checkNodeChange(nodes: TAgentStreamNode[]) {
    const nodeIds = new Set(nodes.map((node) => node.id));

    const deletedNodes = stream.nodes.filter((node) => !nodeIds.has(node.id));
    if (deletedNodes) {
      for (const node of deletedNodes) {
        await removeAgent(stream.id, node.id);
        stream.nodes = stream.nodes.filter((n) => n.id !== node.id);
        streams()[stream.id].nodes = stream.nodes;
      }
    }
  }

  async function checkEdgeChange(edges: TAgentStreamEdge[]) {
    const edgeIds = new Set(edges.map((edge) => edge.id));

    const deletedEdges = stream.edges.filter((edge) => !edgeIds.has(edge.id));
    if (deletedEdges) {
      for (const edge of deletedEdges) {
        await removeChannel(stream.id, edge.id);
        stream.edges = stream.edges.filter((e) => e.id !== edge.id);
        streams()[stream.id].edges = stream.edges;
      }
    }

    const addedEdges = edges.filter((edge) => !stream.edges.some((e) => e.id === edge.id));
    for (const edge of addedEdges) {
      await addChannel(stream.id, serializeAgentStreamEdge(edge));
      stream.edges.push(edge);
      streams()[stream.id].edges = stream.edges;
    }
  }

  // cut, copy and paste

  let copiedNodes = $state.raw<AgentSpec[]>([]);
  let copiedEdges = $state.raw<ChannelSpec[]>([]);

  function selectedNodesAndEdges(): [TAgentStreamNode[], TAgentStreamEdge[]] {
    const selectedNodes = nodes.filter((node) => node.selected);
    const selectedEdges = edges.filter((edge) => edge.selected);
    return [selectedNodes, selectedEdges];
  }

  async function cutNodesAndEdges() {
    const [selectedNodes, selectedEdges] = selectedNodesAndEdges();
    if (selectedNodes.length == 0 && selectedEdges.length == 0) {
      return;
    }
    copiedNodes = selectedNodes.map((node) => serializeAgentStreamNode(node));
    copiedEdges = selectedEdges.map((edge) => serializeAgentStreamEdge(edge));

    nodes = nodes.filter((node) => !node.selected);
    edges = edges.filter((edge) => !edge.selected);
    await checkNodeChange(nodes);
    await checkEdgeChange(edges);
  }

  function copyNodesAndEdges() {
    const [selectedNodes, selectedEdges] = selectedNodesAndEdges();
    if (selectedNodes.length == 0) {
      return;
    }
    copiedNodes = selectedNodes.map((node) => serializeAgentStreamNode(node));
    copiedEdges = selectedEdges.map((edge) => serializeAgentStreamEdge(edge));
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

    let [cnodes, cedges] = await copySubStream(copiedNodes, copiedEdges);
    if (cnodes.length == 0 && cedges.length == 0) return;

    let new_nodes = [];
    for (const node of cnodes) {
      node.x += 80;
      node.y += 80;
      await addAgent(stream.id, node);
      const new_node = deserializeAgentStreamNode(node);
      new_node.selected = true;
      new_nodes.push(new_node);
      stream.nodes.push(new_node);
      streams()[stream.id].nodes = stream.nodes;
    }

    let new_edges = [];
    for (const edge of cedges) {
      await addChannel(stream.id, edge);
      const new_edge = deserializeChannelSpec(edge);
      new_edge.selected = true;
      new_edges.push(new_edge);
      stream.edges.push(new_edge);
      streams()[stream.id].edges = stream.edges;
    }

    nodes = [...nodes, ...new_nodes];
    edges = [...edges, ...new_edges];
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

  let hiddenAgents = $state(true);
  const key_open_agent = "a";

  let openStream = $state(false);
  const key_open_stream = "f";

  $effect(() => {
    hotkeys("ctrl+r", (event) => {
      event.preventDefault();
    });

    hotkeys("ctrl+s", (event) => {
      event.preventDefault();
      onSaveStream();
    });

    hotkeys(key_open_agent, () => {
      hiddenAgents = !hiddenAgents;
    });

    hotkeys(key_open_stream, () => {
      openStream = !openStream;
    });

    hotkeys("ctrl+x", () => {
      /* await */ cutNodesAndEdges();
    });
    hotkeys("ctrl+c", () => {
      copyNodesAndEdges();
    });
    hotkeys("ctrl+v", () => {
      pasteNodesAndEdges();
    });
    hotkeys("ctrl+a", (ev) => {
      ev.preventDefault();
      selectAllNodesAndEdges();
    });

    return () => {
      hotkeys.unbind("ctrl+r");
      hotkeys.unbind("ctrl+s");
      hotkeys.unbind(key_open_agent);
      hotkeys.unbind(key_open_stream);
      hotkeys.unbind("ctrl+x");
      hotkeys.unbind("ctrl+c");
      hotkeys.unbind("ctrl+v");
      hotkeys.unbind("ctrl+a");
    };
  });

  async function onSaveStream() {
    if (stream.id in streams()) {
      const viewport = getViewport();
      const s = serializeAgentStream(
        stream.id,
        stream.name,
        nodes,
        edges,
        stream.run_on_start,
        viewport,
      );
      await saveAgentStream(s);
      streams()[stream.id] = deserializeAgentStream(s);
    }
  }

  function onExportStream() {
    const viewport = getViewport();
    const s = serializeAgentStream(
      stream.id,
      stream.name,
      nodes,
      edges,
      stream.run_on_start,
      viewport,
    );
    const jsonStr = JSON.stringify(s, null, 2);
    const blob = new Blob([jsonStr], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = stream.name + ".json";
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    // TODO: show a toast notification
  }

  async function onImportStream() {
    const file = await open({ multiple: false, filter: "json" });
    if (!file) return;
    const sStream = await importAgentStream(file);
    if (!sStream.agents || !sStream.channels) return;
    const stream = deserializeAgentStream(sStream);
    streams()[stream.id] = stream;
    goto(`/stream_editor/${stream.id}`);
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
    await addAgent(stream.id, snode);
    const new_node = deserializeAgentStreamNode(snode);
    stream.nodes.push(new_node);
    streams()[stream.id].nodes = stream.nodes;
    nodes = [...nodes, new_node];
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

  const handleNodeContextMenu: NodeEventWithPointer<MouseEvent, TAgentStreamNode> = ({
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

  function handleSelectionClick() {
    hideNodeContextMenu();
  }

  function handlePaneClick() {
    hideNodeContextMenu();
  }
</script>

<div class="flex flex-col w-full min-h-screen">
  <header class="flex flex-none items-center justify-between pl-2">
    <Menubar {onImportStream} {onExportStream} />
    <div class="flex flex-row items-center">
      <StreamName name={stream.name} class="mr-4" />
      <StreamActions {stream} />
    </div>
    <div>{" "}</div>
  </header>
  <SvelteFlow
    bind:nodes
    bind:edges
    {nodeTypes}
    onnodecontextmenu={handleNodeContextMenu}
    onselectioncontextmenu={handleSelectionContextMenu}
    onnodeclick={handleNodeClick}
    onselectionclick={handleSelectionClick}
    onpaneclick={handlePaneClick}
    ondelete={handleOnDelete}
    onconnect={handleOnConnect}
    ondragover={handleDragOver}
    ondrop={handleDrop}
    deleteKey={["Delete"]}
    connectionRadius={38}
    colorMode={(data.coreSettings.color_mode as "light" | "dark" | "system") || "system"}
    fitView
    maxZoom={2}
    minZoom={0.1}
    attributionPosition="bottom-right"
    class="flex-1 w-full"
  >
    <Background variant={BackgroundVariant.Dots} gap={28} lineWidth={1} />

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
      class="absolute right-6 top-6 w-60 z-20 max-h-[calc(100vh-216px)] overflow-y-auto pretty-scroll overflow-x-hidden rounded-md border shadow-lg"
    >
      <AgentList class="h-full" {agentDefs} {onAddAgent} onDragAgentStart={handleAgentDragStart} />
    </div>
  </SvelteFlow>
</div>

<style>
  :root {
    --resize-control-size: 6px;
    --resize-control-color: #00eeff;
  }

  :global(.svelte-flow__edge .svelte-flow__edge-path) {
    stroke-width: 8px;
    stroke-opacity: 0.8;
  }

  :global(.svelte-flow__resize-control.handle) {
    border: calc(var(--resize-control-size) * 1.5) solid var(--resize-control-color);
    border-radius: var(--resize-control-size);
    width: 0px;
    height: 0px;
  }
  :global(.svelte-flow__resize-control.line.top) {
    border: var(--resize-control-size) solid var(--resize-control-color);
    border-image: linear-gradient(to top, var(--background), var(--resize-control-color)) 1;
  }
  :global(.svelte-flow__resize-control.line.right) {
    border: var(--resize-control-size) solid var(--resize-control-color);
    border-image: linear-gradient(to right, var(--background), var(--resize-control-color)) 1;
  }
  :global(.svelte-flow__resize-control.line.bottom) {
    border: var(--resize-control-size) solid var(--resize-control-color);
    border-image: linear-gradient(to bottom, var(--background), var(--resize-control-color)) 1;
  }
  :global(.svelte-flow__resize-control.line.left) {
    border: var(--resize-control-size) solid var(--resize-control-color);
    border-image: linear-gradient(to left, var(--background), var(--resize-control-color)) 1;
  }
  :global(.svelte-flow__controls) {
    margin-left: 4px;
    margin-bottom: 4px;
  }
  :global(.svelte-flow__minimap) {
    margin-right: 4px;
    margin-bottom: 4px;
  }
  :global(.svelte-flow__attribution) {
    z-index: 20;
  }
</style>
