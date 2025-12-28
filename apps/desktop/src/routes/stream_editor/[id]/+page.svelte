<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";

  import { onMount } from "svelte";

  import {
    useSvelteFlow,
    Background,
    BackgroundVariant,
    Controls,
    MiniMap,
    SvelteFlow,
    type NodeEventWithPointer,
    type NodeTypes,
    type OnDelete,
    type Connection,
  } from "@xyflow/svelte";
  // 👇 this is important! You need to import the styles for Svelte Flow to work
  import "@xyflow/svelte/dist/style.css";
  import hotkeys from "hotkeys-js";
  import {
    addAgent,
    addChannel,
    copySubStream,
    newAgentSpec,
    removeAgent,
    removeChannel,
    startAgent,
    stopAgent,
  } from "tauri-plugin-askit-api";
  import type { AgentSpec, ChannelSpec } from "tauri-plugin-askit-api";

  import { goto } from "$app/navigation";

  import {
    agentSpecToNode,
    channelSpecToEdge,
    edgeToChannelSpec,
    flowToStreamSpec,
    importAgentStream,
    nodeToAgentSpec,
    saveAgentStream,
    streamToFlow,
  } from "$lib/agent";
  import FlowStatus from "$lib/components/flow-status.svelte";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import { agentDefs, coreSettings } from "$lib/shared.svelte";
  import type { AgentStreamNode, AgentStreamEdge, AgentStreamFlow } from "$lib/types";

  import AgentList from "./agent-list.svelte";
  import AgentNode from "./agent-node.svelte";
  import Menubar from "./menubar.svelte";
  import NodeContextMenu from "./node-context-menu.svelte";
  import StreamActions from "./stream-actions.svelte";
  import StreamName from "./stream-name.svelte";

  const { getViewport, screenToFlowPosition, setViewport, updateEdge, updateNode, updateNodeData } =
    $derived(useSvelteFlow());

  const nodeTypes: NodeTypes = {
    agent: AgentNode,
  };

  let { data } = $props();

  let flow = $state<AgentStreamFlow | null>(null);

  let nodes = $state.raw<AgentStreamNode[]>([]);
  let edges = $state.raw<AgentStreamEdge[]>([]);

  $effect.pre(() => {
    flow = streamToFlow(data.info, data.spec);
  });

  onMount(() => {
    if (!flow) return;

    nodes = [...flow.nodes];
    edges = [...flow.edges];
    const viewport = flow.viewport;
    if (viewport) {
      setViewport(viewport);
    }

    getCurrentWindow().setTitle(flow.name + " - Agent Stream App");
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
    if (!flow) return;

    for (const n of deletedNodes) {
      await removeAgent(flow.id, n.id);
    }
  }

  async function deleteEdges(deletedEdges: AgentStreamEdge[]) {
    if (!flow) return;

    for (const e of deletedEdges) {
      let ch = edgeToChannelSpec(e);
      await removeChannel(flow.id, ch);
    }
  }

  async function handleOnConnect(connection: Connection) {
    if (!flow) return;

    let edge = {
      id: crypto.randomUUID(),
      ...connection,
    } as AgentStreamEdge;

    await addChannel(flow.id, edgeToChannelSpec(edge));
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
    if (!flow) return;

    const [selectedNodes, selectedEdges] = selectedNodesAndEdges();
    if (selectedNodes.length == 0 && selectedEdges.length == 0) {
      return;
    }
    copiedNodes = selectedNodes.map((node) => nodeToAgentSpec(node));
    copiedEdges = selectedEdges.map((edge) => edgeToChannelSpec(edge));

    for (const edge of selectedEdges) {
      let ch = edgeToChannelSpec(edge);
      await removeChannel(flow.id, ch);
    }
    for (const node of selectedNodes) {
      await removeAgent(flow.id, node.id);
    }
    nodes = nodes.filter((node) => !node.selected);
    edges = edges.filter((edge) => !edge.selected);
  }

  function copyNodesAndEdges() {
    const [selectedNodes, selectedEdges] = selectedNodesAndEdges();
    if (selectedNodes.length == 0) {
      return;
    }
    copiedNodes = selectedNodes.map((node) => nodeToAgentSpec(node));
    copiedEdges = selectedEdges.map((edge) => edgeToChannelSpec(edge));
  }

  async function pasteNodesAndEdges() {
    if (!flow) return;

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
      await addAgent(flow.id, node);
      const new_node = agentSpecToNode(node);
      new_node.selected = true;
      new_nodes.push(new_node);
    }

    let new_edges = [];
    for (const edge of cedges) {
      await addChannel(flow.id, edge);
      const new_edge = channelSpecToEdge(edge);
      new_edge.selected = true;
      new_edges.push(new_edge);
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

  $effect(() => {
    hotkeys("ctrl+r", (event) => {
      event.preventDefault();
    });

    hotkeys("ctrl+s", (event) => {
      event.preventDefault();
      onSaveStream();
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
      hotkeys.unbind("ctrl+x");
      hotkeys.unbind("ctrl+c");
      hotkeys.unbind("ctrl+v");
      hotkeys.unbind("ctrl+a");
    };
  });

  async function onSaveStream() {
    if (!flow) return;

    const viewport = getViewport();
    const s = flowToStreamSpec(nodes, edges, flow.run_on_start, viewport);
    await saveAgentStream(flow.name, s);
  }

  function onExportStream() {
    if (!flow) return;

    const viewport = getViewport();
    const s = flowToStreamSpec(nodes, edges, flow.run_on_start, viewport);
    const jsonStr = JSON.stringify(s, null, 2);
    const blob = new Blob([jsonStr], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = flow.name + ".json";
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    // TODO: show a toast notification
  }

  async function onImportStream() {
    const file = await open({ multiple: false, filter: "json" });
    if (!file) return;
    const id = await importAgentStream(file);
    goto(`/stream_editor/${id}`);
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
    if (!flow) return;

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
    await addAgent(flow.id, snode);
    const new_node = agentSpecToNode(snode);
    nodes = [...nodes, new_node];

    if (flow.running) {
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
      <Menubar {onImportStream} {onExportStream} />
    </div>
    <div class="flex flex-row items-center justify-center">
      <StreamName name={flow?.name} class="mr-4" />
      <StreamActions bind:flow />
    </div>
    <div class="justify-self-end">
      <FlowStatus running={flow?.running ?? false} run_on_start={flow?.run_on_start} />
    </div>
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
    colorMode={(coreSettings.color_mode as "light" | "dark" | "system") || "system"}
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
      class="absolute right-2 top-2 w-60 z-20 max-h-[calc(100vh-210px)] overflow-x-hidden rounded-md border shadow-lg"
    >
      <ScrollArea>
        <AgentList
          class="h-full"
          {agentDefs}
          {onAddAgent}
          onDragAgentStart={handleAgentDragStart}
        />
      </ScrollArea>
    </div>
  </SvelteFlow>
</div>

<style>
  :root {
    --resize-control-size: 2px;
    --resize-control-color: var(--color-red-500);
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
