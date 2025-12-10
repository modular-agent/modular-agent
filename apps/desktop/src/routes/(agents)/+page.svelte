<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";

  import { getContext, onMount, tick } from "svelte";

  import {
    useSvelteFlow,
    Background,
    BackgroundVariant,
    Controls,
    MiniMap,
    SvelteFlow,
    type Edge,
    type NodeTypes,
  } from "@xyflow/svelte";
  // 👇 this is important! You need to import the styles for Svelte Flow to work
  import "@xyflow/svelte/dist/style.css";
  import { Button, ButtonGroup, GradientButton, Modal, Toast } from "flowbite-svelte";
  import { ExclamationCircleOutline, PauseOutline, PlayOutline } from "flowbite-svelte-icons";
  import hotkeys from "hotkeys-js";
  import {
    addAgentFlowEdge,
    newAgentFlowNode,
    addAgentFlowNode,
    removeAgentFlowEdge,
    removeAgentFlowNode,
    startAgent,
    stopAgent,
    newAgentFlow,
    copySubFlow,
  } from "tauri-plugin-askit-api";
  import type { AgentFlowNode, AgentFlowEdge } from "tauri-plugin-askit-api";

  import {
    deserializeAgentFlow,
    deserializeAgentFlowEdge,
    deserializeAgentFlowNode,
    importAgentFlow,
    removeAgentFlow,
    renameAgentFlow,
    saveAgentFlow,
    serializeAgentFlow,
    serializeAgentFlowEdge,
    serializeAgentFlowNode,
    setAgentDefinitionsContext,
  } from "@/lib/agent";
  import { flowState } from "@/lib/shared.svelte";
  import type { TAgentFlowNode, TAgentFlowEdge, TAgentFlow } from "@/lib/types";

  import AgentList from "./AgentList.svelte";
  import AgentNode from "./AgentNode.svelte";
  import FileMenu from "./FileMenu.svelte";
  import FlowList from "./FlowList.svelte";
  import NodeContextMenu from "./NodeContextMenu.svelte";

  let { data } = $props();

  const { getViewport, screenToFlowPosition, setViewport, updateEdge, updateNode, updateNodeData } =
    $derived(useSvelteFlow());
  setAgentDefinitionsContext(data.agentDefs);

  const nodeTypes: NodeTypes = {
    agent: AgentNode,
  };

  let nodes = $state.raw<TAgentFlowNode[]>([]);
  let edges = $state.raw<TAgentFlowEdge[]>([]);

  const agentDefs = $derived(data.agentDefs);
  const flows = getContext<() => Record<string, TAgentFlow>>("agentFlows");

  let flowNames = $state.raw<{ id: string; name: string }[]>([]);

  // id -> flow activity
  let flowActivities = $state<Record<string, boolean>>({});

  function updateNodesAndEdges() {
    nodes = [...flows()[flowState.id].nodes];
    edges = [...flows()[flowState.id].edges];
    const viewport = flows()[flowState.id].viewport;
    if (viewport) {
      setViewport(viewport);
    }
  }

  function updateFlowNames() {
    flowNames = Object.entries(flows())
      .map(([id, flow]) => ({ id, name: flow.name }))
      .sort((a, b) => a.name.localeCompare(b.name));
  }

  function updateFlowActivities() {
    flowActivities = Object.fromEntries(
      Object.entries(flows()).map(([id, flow]) => [
        id,
        flow.nodes.some((node) => node.data.enabled),
      ]),
    );
  }

  function updateCurrentFlowActivity() {
    flowActivities[flowState.id] = nodes.some((node) => node.data.enabled);
  }

  onMount(() => {
    if (flowState.id === "") {
      Object.entries(flows()).forEach(([id, flow]) => {
        if (flow.name === flowState.name) {
          flowState.id = id;
        }
      });
    }
    updateNodesAndEdges();
    updateFlowNames();
    updateFlowActivities();
    // return async () => {
    //   await syncFlow();
    // };
  });

  async function changeFlow(id: string) {
    flowState.id = id;
    flowState.name = flows()[id].name;
    // await syncFlow();
    updateNodesAndEdges();
  }

  // async function changeFlowByName(name: string) {
  //   Object.entries(flows()).forEach(([id, flow]) => {
  //     if (flow.name === name) {
  //       flowState.id = id;
  //       flowState.name = name;
  //     }
  //   });
  //   await syncFlow();
  //   updateNodesAndEdges();
  // }

  async function handleOnDelete(params: { nodes: Node[]; edges: Edge[] }) {
    if (params.edges && params.edges.length > 0) {
      await checkEdgeChange(edges);
    }
    if (params.nodes && params.nodes.length > 0) {
      await checkNodeChange(nodes);
      updateCurrentFlowActivity();
    }
  }

  async function handleOnConnect() {
    await checkEdgeChange(edges);
  }

  async function checkNodeChange(nodes: TAgentFlowNode[]) {
    const nodeIds = new Set(nodes.map((node) => node.id));

    const deletedNodes = flows()[flowState.id]?.nodes.filter((node) => !nodeIds.has(node.id));
    if (deletedNodes) {
      for (const node of deletedNodes) {
        await removeAgentFlowNode(flowState.id, node.id);
        flows()[flowState.id].nodes = flows()[flowState.id].nodes.filter((n) => n.id !== node.id);
      }
    }
  }

  async function checkEdgeChange(edges: TAgentFlowEdge[]) {
    const edgeIds = new Set(edges.map((edge) => edge.id));

    const deletedEdges = flows()[flowState.id]?.edges.filter((edge) => !edgeIds.has(edge.id));
    if (deletedEdges) {
      for (const edge of deletedEdges) {
        await removeAgentFlowEdge(flowState.id, edge.id);
        flows()[flowState.id].edges = flows()[flowState.id].edges.filter((e) => e.id !== edge.id);
      }
    }

    const addedEdges = edges.filter(
      (edge) => !flows()[flowState.id].edges.some((e) => e.id === edge.id),
    );
    for (const edge of addedEdges) {
      await addAgentFlowEdge(flowState.id, serializeAgentFlowEdge(edge));
      flows()[flowState.id].edges.push(edge);
    }
  }

  // async function syncFlow() {
  //   const viewport = getViewport();
  //   const flow = serializeAgentFlow(
  //     flowState.id,
  //     flowState.name,
  //     nodes,
  //     edges,
  //     agentDefs,
  //     viewport,
  //   );
  //   flows()[flowState.id] = deserializeAgentFlow(flow, agentDefs);
  //   await insertAgentFlow(flow);
  // }

  // cut, copy and paste

  let copiedNodes = $state.raw<AgentFlowNode[]>([]);
  let copiedEdges = $state.raw<AgentFlowEdge[]>([]);

  function selectedNodesAndEdges(): [TAgentFlowNode[], TAgentFlowEdge[]] {
    const selectedNodes = nodes.filter((node) => node.selected);
    const selectedEdges = edges.filter((edge) => edge.selected);
    return [selectedNodes, selectedEdges];
  }

  async function cutNodesAndEdges() {
    const [selectedNodes, selectedEdges] = selectedNodesAndEdges();
    if (selectedNodes.length == 0 && selectedEdges.length == 0) {
      return;
    }
    copiedNodes = selectedNodes.map((node) => serializeAgentFlowNode(node));
    copiedEdges = selectedEdges.map((edge) => serializeAgentFlowEdge(edge));

    nodes = nodes.filter((node) => !node.selected);
    edges = edges.filter((edge) => !edge.selected);
    await checkNodeChange(nodes);
    await checkEdgeChange(edges);
    updateCurrentFlowActivity();
  }

  function copyNodesAndEdges() {
    const [selectedNodes, selectedEdges] = selectedNodesAndEdges();
    if (selectedNodes.length == 0) {
      return;
    }
    copiedNodes = selectedNodes.map((node) => serializeAgentFlowNode(node));
    copiedEdges = selectedEdges.map((edge) => serializeAgentFlowEdge(edge));
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

    let [cnodes, cedges] = await copySubFlow(copiedNodes, copiedEdges);
    if (cnodes.length == 0 && cedges.length == 0) return;

    let new_nodes = [];
    for (const node of cnodes) {
      node.x += 80;
      node.y += 80;
      node.enabled = false;
      await addAgentFlowNode(flowState.id, node);
      const new_node = deserializeAgentFlowNode(node);
      new_node.selected = true;
      new_nodes.push(new_node);
      flows()[flowState.id].nodes.push(new_node);
    }

    let new_edges = [];
    for (const edge of cedges) {
      await addAgentFlowEdge(flowState.id, edge);
      const new_edge = deserializeAgentFlowEdge(edge);
      new_edge.selected = true;
      new_edges.push(new_edge);
      flows()[flowState.id].edges.push(new_edge);
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

  let openFlow = $state(false);
  const key_open_flow = "f";

  $effect(() => {
    hotkeys("ctrl+r", (event) => {
      event.preventDefault();
    });

    hotkeys("ctrl+s", (event) => {
      event.preventDefault();
      onSaveFlow();
    });

    hotkeys(key_open_agent, () => {
      hiddenAgents = !hiddenAgents;
    });

    hotkeys(key_open_flow, () => {
      openFlow = !openFlow;
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
      hotkeys.unbind(key_open_flow);
      hotkeys.unbind("ctrl+x");
      hotkeys.unbind("ctrl+c");
      hotkeys.unbind("ctrl+v");
      hotkeys.unbind("ctrl+a");
    };
  });

  // New Flow

  let newFlowModal = $state(false);
  let newFlowName = $state("");
  let newFlowInput = $state<HTMLInputElement>();

  async function onNewFlow() {
    newFlowName = flowState.name.split("/").slice(0, -1).join("/");
    if (newFlowName !== "") {
      newFlowName += "/";
    }
    newFlowModal = true;
    await tick();
    newFlowInput?.focus();
  }

  async function handleCreateNewFlow() {
    newFlowModal = false;
    if (!newFlowName) return;
    const flow_id = await createNewFlow(newFlowName);
    if (flow_id) {
      await changeFlow(flow_id);
    }
  }

  async function createNewFlow(name: string | null): Promise<string | null> {
    if (!name) return null;
    const flow = await newAgentFlow(name);
    if (!flow) return null;
    flows()[flow.id] = deserializeAgentFlow(flow);
    updateFlowNames();
    updateFlowActivities();
    return flow.id;
  }

  // Rename Flow

  let renameFlowModal = $state(false);
  let renameFlowName = $state("");
  let renameFlowInput = $state<HTMLInputElement>();

  async function onRenameFlow() {
    renameFlowName = flowState.name;
    renameFlowModal = true;
    await tick();
    renameFlowInput?.focus();
  }

  async function handleRenameFlow() {
    renameFlowModal = false;
    if (!renameFlowName || renameFlowName === flowState.name) return;
    const newName = await renameFlow(flowState.id, renameFlowName);
    if (!newName) return;
    // We don't need to sync the current flow.
    // await changeFlowName(newName);
    flowState.name = newName;
    updateNodesAndEdges();
  }

  async function renameFlow(id: string, rename: string): Promise<string | null> {
    if (!id || !rename) return null;
    const newName = await renameAgentFlow(id, rename);
    if (!newName) return null;
    const flow = flows()[id];
    flow.name = newName;
    updateFlowNames();
    updateFlowActivities();
    return newName;
  }

  // Delete Flow

  let deleteFlowModal = $state(false);
  let cannotDeleteToast = $state(false);

  function onDeleteFlow() {
    if (flowState.name === "main") {
      cannotDeleteToast = true;
      return;
    }

    deleteFlowModal = true;
  }

  async function handleDeleteFlow() {
    deleteFlowModal = false;
    await deleteFlow(flowState.id);

    flowState.name = "main";
    Object.entries(flows()).forEach(([id, flow]) => {
      if (flow.name === flowState.name) {
        flowState.id = id;
      }
    });
    updateNodesAndEdges();
  }

  async function deleteFlow(id: string) {
    if (!id) return;
    const flow = flows()[id];
    if (!flow) return;
    await removeAgentFlow(id);
    delete flows()[id];
    updateFlowNames();
    updateFlowActivities();
  }

  async function onSaveFlow() {
    if (flowState.id in flows()) {
      const viewport = getViewport();
      const flow = serializeAgentFlow(flowState.id, flowState.name, nodes, edges, viewport);
      await saveAgentFlow(flow);
      flows()[flowState.id] = deserializeAgentFlow(flow);
    }
  }

  function onExportFlow() {
    const viewport = getViewport();
    const flow = serializeAgentFlow(flowState.id, flowState.name, nodes, edges, viewport);
    const jsonStr = JSON.stringify(flow, null, 2);
    const blob = new Blob([jsonStr], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = flowState.name + ".json";
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }

  async function onImportFlow() {
    const file = await open({ multiple: false, filter: "json" });
    if (!file) return;
    const sflow = await importAgentFlow(file);
    if (!sflow.nodes || !sflow.edges) return;
    const flow = deserializeAgentFlow(sflow);
    flows()[flow.id] = flow;
    updateFlowNames();
    updateFlowActivities();
    await changeFlow(flow.id);
  }

  async function onAddAgent(agent_name: string) {
    const snode = await newAgentFlowNode(agent_name);
    const xy = screenToFlowPosition({
      x: window.innerWidth * 0.45,
      y: window.innerHeight * 0.3,
    });
    snode.x = xy.x;
    snode.y = xy.y;
    await addAgentFlowNode(flowState.id, snode);
    const new_node = deserializeAgentFlowNode(snode);
    flows()[flowState.id].nodes.push(new_node);
    nodes = [...nodes, new_node];
  }

  async function onPlay() {
    const [selectedNodes, selectedEdges] = selectedNodesAndEdges();
    if (selectedNodes.length > 0 || selectedEdges.length > 0) {
      // start only selected agents
      for (const node of selectedNodes) {
        if (!node.data.enabled) {
          updateNodeData(node.id, { enabled: true });
          await startAgent(node.id);
        }
      }
      updateCurrentFlowActivity();
      return;
    }

    // start all agents
    for (const node of nodes) {
      if (!node.data.enabled) {
        updateNodeData(node.id, { enabled: true });
        await startAgent(node.id);
      }
    }
    updateCurrentFlowActivity();
  }

  async function onPause() {
    const [selectedNodes, selectedEdges] = selectedNodesAndEdges();
    if (selectedNodes.length > 0 || selectedEdges.length > 0) {
      // stop only selected agents
      for (const node of selectedNodes) {
        if (node.data.enabled) {
          updateNodeData(node.id, { enabled: false });
          await stopAgent(node.id);
        }
      }
      updateCurrentFlowActivity();
      return;
    }

    // stop all agents
    for (const node of nodes) {
      if (node.data.enabled) {
        updateNodeData(node.id, { enabled: false });
        await stopAgent(node.id);
      }
    }
    updateCurrentFlowActivity();
  }

  let nodeContextMenu: {
    x: number;
    y: number;
  } | null = $state(null);

  function showNodeContextMenu(x: number, y: number) {
    nodeContextMenu = {
      x,
      y,
    };
  }

  function hideNodeContextMenu() {
    nodeContextMenu = null;
  }

  function handleNodeContextMenu({ event, node }: { event: MouseEvent; node: Node }) {
    event.preventDefault();

    const agentNode = node as unknown as TAgentFlowNode;

    const [selectedNodes, _] = selectedNodesAndEdges();
    if (!selectedNodes.some((n) => n.id === agentNode.id)) {
      nodes.forEach((n) => {
        updateNode(n.id, { selected: false });
      });
      updateNode(agentNode.id, { selected: true });
    }

    showNodeContextMenu(event.clientX, event.clientY);
  }

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

<div class="bg-white! dark:bg-black! static">
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
    deleteKey={["Delete"]}
    connectionRadius={38}
    colorMode="dark"
    fitView
    maxZoom={2}
    minZoom={0.1}
    attributionPosition="bottom-left"
    class="relative w-full min-h-screen text-black! !dark:text-white bg-gray-100! dark:bg-black!"
  >
    <Background
      variant={BackgroundVariant.Lines}
      bgColor="#000"
      patternColor="#1a1a1a"
      gap={28}
      lineWidth={1}
    />
    <Controls />
    <MiniMap />
    <ButtonGroup class="absolute bottom-4 z-10 w-full flex justify-center">
      <Button onclick={onPause} pill class="bg-gray-100! dark:bg-gray-900! opacity-80">
        <PauseOutline
          class="w-5 h-5 mb-1/2 text-gray-500 dark:text-gray-400 group-hover:text-primary-600 dark:group-hover:text-primary-500"
        />
      </Button>
      <Button onclick={onPlay} pill class="bg-gray-100! dark:bg-gray-900! opacity-80">
        <PlayOutline
          class="w-5 h-5 mb-1/2 text-gray-500 dark:text-gray-400 group-hover:text-primary-600 dark:group-hover:text-primary-500"
        />
      </Button>
    </ButtonGroup>

    {#if nodeContextMenu}
      <NodeContextMenu
        x={nodeContextMenu.x}
        y={nodeContextMenu.y}
        {hideNodeContextMenu}
        onstart={onPlay}
        onstop={onPause}
        oncut={cutNodesAndEdges}
        oncopy={copyNodesAndEdges}
      />
    {/if}

    <FileMenu
      {onNewFlow}
      {onRenameFlow}
      {onDeleteFlow}
      {onSaveFlow}
      {onExportFlow}
      {onImportFlow}
    />
  </SvelteFlow>
  <div class="absolute top-1 left-0 w-40">
    <FlowList {flowNames} currentFlow={flowState.id} {flowActivities} {changeFlow} />
  </div>
  <div class="absolute right-0 top-1 w-60">
    <AgentList {agentDefs} {onAddAgent} />
  </div>
</div>

{#if newFlowModal}
  <Modal title="New Flow" bind:open={newFlowModal} classBackdrop="bg-transparent">
    <form onsubmit={handleCreateNewFlow} autocomplete="off">
      <div class="flex flex-col">
        <label for="flow_name" class="mb-2 text-sm font-medium text-gray-900 dark:text-white"
          >Flow Name</label
        >
        <input
          bind:this={newFlowInput}
          type="text"
          id="flow_name"
          bind:value={newFlowName}
          class="block p-2 w-full text-sm text-gray-900 bg-gray-50 rounded-lg border border-gray-300 focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500"
          placeholder="Flow Name"
        />
      </div>
      <div class="flex justify-end mt-4">
        <GradientButton color="pinkToOrange">Create</GradientButton>
      </div>
    </form>
  </Modal>
{/if}

{#if renameFlowModal}
  <Modal title="Rename Flow" bind:open={renameFlowModal} classBackdrop="bg-transparent">
    <form onsubmit={handleRenameFlow} autocomplete="off">
      <div class="flex flex-col">
        <label for="flow_name" class="mb-2 text-sm font-medium text-gray-900 dark:text-white"
          >Flow Name</label
        >
        <input
          bind:this={renameFlowInput}
          type="text"
          id="flow_name"
          bind:value={renameFlowName}
          class="block p-2 w-full text-sm text-gray-900 bg-gray-50 rounded-lg border border-gray-300 focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500"
          placeholder="Flow Name"
        />
      </div>
      <div class="flex justify-end mt-4">
        <GradientButton color="pinkToOrange">Rename</GradientButton>
      </div>
    </form>
  </Modal>
{/if}

{#if deleteFlowModal}
  <Modal
    title="Delete Flow"
    bind:open={deleteFlowModal}
    size="xs"
    autoclose
    classBackdrop="bg-transparent"
  >
    <div class="text-center">
      <ExclamationCircleOutline class="mx-auto mb-4 text-gray-400 w-12 h-12 dark:text-gray-200" />
      <h3 class="mb-5 text-lg font-normal text-gray-500 dark:text-gray-400">
        Are you sure you want to delete this flow?
      </h3>
      <Button onclick={handleDeleteFlow} color="red" class="me-2">Delete</Button>
      <Button color="alternative">Cancel</Button>
    </div>
  </Modal>
{/if}

{#if cannotDeleteToast}
  <Toast bind:toastStatus={cannotDeleteToast} class="absolute top-1/2 left-1/2 z-50">
    "main" flow cannot be deleted.
  </Toast>
{/if}

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
    border-image: linear-gradient(to top, #000, var(--resize-control-color)) 1;
  }
  :global(.svelte-flow__resize-control.line.right) {
    border: var(--resize-control-size) solid var(--resize-control-color);
    border-image: linear-gradient(to right, #000, var(--resize-control-color)) 1;
  }
  :global(.svelte-flow__resize-control.line.bottom) {
    border: var(--resize-control-size) solid var(--resize-control-color);
    border-image: linear-gradient(to bottom, #000, var(--resize-control-color)) 1;
  }
  :global(.svelte-flow__resize-control.line.left) {
    border: var(--resize-control-size) solid var(--resize-control-color);
    border-image: linear-gradient(to left, #000, var(--resize-control-color)) 1;
  }
</style>
