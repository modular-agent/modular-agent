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
    addAgentsAndChannels,
    addChannel,
    newAgentSpec,
    removeAgent,
    removeChannel,
    setAgentStreamSpec,
    startAgent,
    stopAgent,
  } from "tauri-plugin-askit-api";
  import type { AgentSpec, AgentStreamSpec, ChannelSpec } from "tauri-plugin-askit-api";

  import { goto } from "$app/navigation";

  import {
    agentSpecToNode,
    channelSpecToEdge,
    edgeToChannelSpec,
    flowToStreamSpec,
    nodeToAgentSpec,
    saveAgentStream,
  } from "$lib/agent";
  import FlowStatus from "$lib/components/flow-status.svelte";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import {
    agentDefs,
    coreSettings,
    importStream,
    newStream,
    updateStreamSpec,
  } from "$lib/shared.svelte";
  import type { AgentStreamNode, AgentStreamEdge } from "$lib/types";

  import AgentList from "./agent-list.svelte";
  import AgentNode from "./agent-node.svelte";
  import Menubar from "./menubar.svelte";
  import NodeContextMenu from "./node-context-menu.svelte";
  import StreamActions from "./stream-actions.svelte";
  import StreamName from "./stream-name.svelte";

  const { getViewport, screenToFlowPosition, updateEdge, updateNode, updateNodeData } =
    $derived(useSvelteFlow());

  const nodeTypes: NodeTypes = {
    agent: AgentNode,
  };

  let { data } = $props();

  let stream_id = $derived(data.stream_id);
  let running = $derived(data.flow?.running ?? false);

  let nodes = $state.raw<AgentStreamNode[]>([]);
  let edges = $state.raw<AgentStreamEdge[]>([]);

  $effect.pre(() => {
    nodes = [...data.flow.nodes];
    edges = [...data.flow.edges];

    getCurrentWindow().setTitle(data.flow.name + " - Agent Stream App");
  });

  onMount(() => {
    return async () => {
      await syncStream();
    };
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
    copiedNodes = selectedNodes.map((node) => nodeToAgentSpec(node));
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

  function copyNodesAndEdges() {
    const [selectedNodes, selectedEdges] = selectedNodesAndEdges();
    if (selectedNodes.length == 0) {
      return;
    }
    copiedNodes = selectedNodes.map((node) => nodeToAgentSpec(node));
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

  async function syncStream(): Promise<null | AgentStreamSpec> {
    const viewport = getViewport();
    const s = flowToStreamSpec(nodes, edges, data.flow.run_on_start, viewport);
    await setAgentStreamSpec(stream_id, s);
    return s;
  }

  async function onNewStream(name: string) {
    await syncStream();

    const new_id = await newStream(name);
    if (new_id) {
      goto(`/stream_editor/${new_id}`, { invalidateAll: true });
    }
  }

  async function onSaveStream() {
    const s = await syncStream();
    if (!s) return;
    await saveAgentStream(data.flow.name, s);
  }

  async function onDuplicateStream() {
    const s = await syncStream();
    if (!s) return;
    const new_id = await newStream(data.flow.name);
    if (new_id) {
      await updateStreamSpec(new_id, s);
      goto(`/stream_editor/${new_id}`, { invalidateAll: true });
    }
  }

  async function onExportStream() {
    const s = await syncStream();
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
    await syncStream();

    const file = await open({ multiple: false, filter: "json" });
    if (!file) return;
    const id = await importStream(file);
    goto(`/stream_editor/${id}`, { invalidateAll: true });
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
      <Menubar {onNewStream} {onSaveStream} {onDuplicateStream} {onImportStream} {onExportStream} />
    </div>
    <div class="flex flex-row items-center justify-center">
      <StreamName name={data.flow?.name} class="mr-4" />
      <StreamActions {stream_id} bind:running />
    </div>
    <div class="justify-self-end">
      <FlowStatus {running} run_on_start={data.flow?.run_on_start} />
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
