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
    type NodeEventWithPointer,
    type OnDelete,
    type NodeTypes,
  } from "@xyflow/svelte";
  // 👇 this is important! You need to import the styles for Svelte Flow to work
  import "@xyflow/svelte/dist/style.css";
  import { Button, ButtonGroup, GradientButton, Modal, Toast } from "flowbite-svelte";
  import { ExclamationCircleOutline, PauseOutline, PlayOutline } from "flowbite-svelte-icons";
  import hotkeys from "hotkeys-js";
  import {
    addAgent,
    addChannel,
    newAgentSpec,
    removeAgent,
    removeChannel,
    startAgent,
    stopAgent,
    newAgentStream,
    copySubStream,
  } from "tauri-plugin-askit-api";
  import type { AgentSpec, ChannelSpec } from "tauri-plugin-askit-api";

  import {
    deserializeAgentStream,
    deserializeChannelSpec,
    deserializeAgentStreamNode,
    importAgentStream,
    removeAgentStream,
    renameAgentStream,
    saveAgentStream,
    serializeAgentStream,
    serializeAgentStreamEdge,
    serializeAgentStreamNode,
    setAgentDefinitionsContext,
  } from "@/lib/agent";
  import { streamState } from "@/lib/shared.svelte";
  import type { TAgentStreamNode, TAgentStreamEdge, TAgentStream } from "@/lib/types";

  import AgentList from "./AgentList.svelte";
  import AgentNode from "./AgentNode.svelte";
  import FileMenu from "./FileMenu.svelte";
  import NodeContextMenu from "./NodeContextMenu.svelte";
  import StreamList from "./StreamList.svelte";

  let { data } = $props();

  const { getViewport, screenToFlowPosition, setViewport, updateEdge, updateNode, updateNodeData } =
    $derived(useSvelteFlow());

  $effect.pre(() => {
    setAgentDefinitionsContext(data.agentDefs);
  });

  const nodeTypes: NodeTypes = {
    agent: AgentNode,
  };

  let nodes = $state.raw<TAgentStreamNode[]>([]);
  let edges = $state.raw<TAgentStreamEdge[]>([]);

  const agentDefs = $derived(data.agentDefs);
  const streams = getContext<() => Record<string, TAgentStream>>("AgentStreams");

  let streamNames = $state.raw<{ id: string; name: string }[]>([]);

  // id -> stream activity
  let streamActivities = $state<Record<string, boolean>>({});

  function updateNodesAndEdges() {
    nodes = [...streams()[streamState.id].nodes];
    edges = [...streams()[streamState.id].edges];
    const viewport = streams()[streamState.id].viewport;
    if (viewport) {
      setViewport(viewport);
    }
  }

  function updateStreamNames() {
    streamNames = Object.entries(streams())
      .map(([id, stream]) => ({ id, name: stream.name }))
      .sort((a, b) => a.name.localeCompare(b.name));
  }

  function updateStreamActivities() {
    streamActivities = Object.fromEntries(
      Object.entries(streams()).map(([id, stream]) => [
        id,
        stream.nodes.some((node) => node.data.enabled),
      ]),
    );
  }

  function updateCurrentStreamActivity() {
    streamActivities[streamState.id] = nodes.some((node) => node.data.enabled);
  }

  onMount(() => {
    if (streamState.id === "") {
      Object.entries(streams()).forEach(([id, stream]) => {
        if (stream.name === streamState.name) {
          streamState.id = id;
        }
      });
    }
    updateNodesAndEdges();
    updateStreamNames();
    updateStreamActivities();
    // return async () => {
    //   await syncStream();
    // };
  });

  async function changeStream(id: string) {
    streamState.id = id;
    streamState.name = streams()[id].name;
    // await syncStream();
    updateNodesAndEdges();
  }

  // async function changeStreamByName(name: string) {
  //   Object.entries(streams()).forEach(([id, stream]) => {
  //     if (stream.name === name) {
  //       streamState.id = id;
  //       streamState.name = name;
  //     }
  //   });
  //   await syncStream();
  //   updateNodesAndEdges();
  // }

  const handleOnDelete: OnDelete<TAgentStreamNode, TAgentStreamEdge> = async ({
    nodes: deletedNodes,
    edges: deletedEdges,
  }) => {
    if (deletedEdges && deletedEdges.length > 0) {
      await checkEdgeChange(edges);
    }
    if (deletedNodes && deletedNodes.length > 0) {
      await checkNodeChange(nodes);
      updateCurrentStreamActivity();
    }
  };

  async function handleOnConnect() {
    await checkEdgeChange(edges);
  }

  async function checkNodeChange(nodes: TAgentStreamNode[]) {
    const nodeIds = new Set(nodes.map((node) => node.id));

    const deletedNodes = streams()[streamState.id]?.nodes.filter((node) => !nodeIds.has(node.id));
    if (deletedNodes) {
      for (const node of deletedNodes) {
        await removeAgent(streamState.id, node.id);
        streams()[streamState.id].nodes = streams()[streamState.id].nodes.filter(
          (n) => n.id !== node.id,
        );
      }
    }
  }

  async function checkEdgeChange(edges: TAgentStreamEdge[]) {
    const edgeIds = new Set(edges.map((edge) => edge.id));

    const deletedEdges = streams()[streamState.id]?.edges.filter((edge) => !edgeIds.has(edge.id));
    if (deletedEdges) {
      for (const edge of deletedEdges) {
        await removeChannel(streamState.id, edge.id);
        streams()[streamState.id].edges = streams()[streamState.id].edges.filter(
          (e) => e.id !== edge.id,
        );
      }
    }

    const addedEdges = edges.filter(
      (edge) => !streams()[streamState.id].edges.some((e) => e.id === edge.id),
    );
    for (const edge of addedEdges) {
      await addChannel(streamState.id, serializeAgentStreamEdge(edge));
      streams()[streamState.id].edges.push(edge);
    }
  }

  // async function syncStream() {
  //   const viewport = getViewport();
  //   const stream = serializeAgentStream(
  //     streamState.id,
  //     streamState.name,
  //     nodes,
  //     edges,
  //     agentDefs,
  //     viewport,
  //   );
  //   streams()[streamState.id] = deserializeAgentStream(stream, agentDefs);
  //   await insertAgentStream(stream);
  // }

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
    updateCurrentStreamActivity();
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
      node.enabled = false;
      await addAgent(streamState.id, node);
      const new_node = deserializeAgentStreamNode(node);
      new_node.selected = true;
      new_nodes.push(new_node);
      streams()[streamState.id].nodes.push(new_node);
    }

    let new_edges = [];
    for (const edge of cedges) {
      await addChannel(streamState.id, edge);
      const new_edge = deserializeChannelSpec(edge);
      new_edge.selected = true;
      new_edges.push(new_edge);
      streams()[streamState.id].edges.push(new_edge);
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

  // New Stream

  let newStreamModal = $state(false);
  let newStreamName = $state("");
  let newStreamInput = $state<HTMLInputElement>();

  async function onNewStream() {
    newStreamName = streamState.name.split("/").slice(0, -1).join("/");
    if (newStreamName !== "") {
      newStreamName += "/";
    }
    newStreamModal = true;
    await tick();
    newStreamInput?.focus();
  }

  async function handleCreateNewStream() {
    newStreamModal = false;
    if (!newStreamName) return;
    const stream_id = await createNewStream(newStreamName);
    if (stream_id) {
      await changeStream(stream_id);
    }
  }

  async function createNewStream(name: string | null): Promise<string | null> {
    if (!name) return null;
    const stream = await newAgentStream(name);
    if (!stream) return null;
    streams()[stream.id] = deserializeAgentStream(stream);
    updateStreamNames();
    updateStreamActivities();
    return stream.id;
  }

  // Rename Stream

  let renameStreamModal = $state(false);
  let renameStreamName = $state("");
  let renameStreamInput = $state<HTMLInputElement>();

  async function onRenameStream() {
    renameStreamName = streamState.name;
    renameStreamModal = true;
    await tick();
    renameStreamInput?.focus();
  }

  async function handleRenameStream() {
    renameStreamModal = false;
    if (!renameStreamName || renameStreamName === streamState.name) return;
    const newName = await renameStream(streamState.id, renameStreamName);
    if (!newName) return;
    // We don't need to sync the current stream.
    // await changeStreamName(newName);
    streamState.name = newName;
    updateNodesAndEdges();
  }

  async function renameStream(id: string, rename: string): Promise<string | null> {
    if (!id || !rename) return null;
    const newName = await renameAgentStream(id, rename);
    if (!newName) return null;
    const stream = streams()[id];
    stream.name = newName;
    updateStreamNames();
    updateStreamActivities();
    return newName;
  }

  // Delete Stream

  let deleteStreamModal = $state(false);
  let cannotDeleteToast = $state(false);

  function onDeleteStream() {
    if (streamState.name === "main") {
      cannotDeleteToast = true;
      return;
    }

    deleteStreamModal = true;
  }

  async function handleDeleteStream() {
    deleteStreamModal = false;
    await deleteStream(streamState.id);

    streamState.name = "main";
    Object.entries(streams()).forEach(([id, stream]) => {
      if (stream.name === streamState.name) {
        streamState.id = id;
      }
    });
    updateNodesAndEdges();
  }

  async function deleteStream(id: string) {
    if (!id) return;
    const stream = streams()[id];
    if (!stream) return;
    await removeAgentStream(id);
    delete streams()[id];
    updateStreamNames();
    updateStreamActivities();
  }

  async function onSaveStream() {
    if (streamState.id in streams()) {
      const viewport = getViewport();
      const stream = serializeAgentStream(streamState.id, streamState.name, nodes, edges, viewport);
      await saveAgentStream(stream);
      streams()[streamState.id] = deserializeAgentStream(stream);
    }
  }

  function onExportStream() {
    const viewport = getViewport();
    const stream = serializeAgentStream(streamState.id, streamState.name, nodes, edges, viewport);
    const jsonStr = JSON.stringify(stream, null, 2);
    const blob = new Blob([jsonStr], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = streamState.name + ".json";
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }

  async function onImportStream() {
    const file = await open({ multiple: false, filter: "json" });
    if (!file) return;
    const sStream = await importAgentStream(file);
    if (!sStream.agents || !sStream.channels) return;
    const stream = deserializeAgentStream(sStream);
    streams()[stream.id] = stream;
    updateStreamNames();
    updateStreamActivities();
    await changeStream(stream.id);
  }

  async function onAddAgent(agent_name: string) {
    const snode = await newAgentSpec(agent_name);
    const xy = screenToFlowPosition({
      x: window.innerWidth * 0.45,
      y: window.innerHeight * 0.3,
    });
    snode.x = xy.x;
    snode.y = xy.y;
    await addAgent(streamState.id, snode);
    const new_node = deserializeAgentStreamNode(snode);
    streams()[streamState.id].nodes.push(new_node);
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
      updateCurrentStreamActivity();
      return;
    }

    // start all agents
    for (const node of nodes) {
      if (!node.data.enabled) {
        updateNodeData(node.id, { enabled: true });
        await startAgent(node.id);
      }
    }
    updateCurrentStreamActivity();
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
      updateCurrentStreamActivity();
      return;
    }

    // stop all agents
    for (const node of nodes) {
      if (node.data.enabled) {
        updateNodeData(node.id, { enabled: false });
        await stopAgent(node.id);
      }
    }
    updateCurrentStreamActivity();
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
      {onNewStream}
      {onRenameStream}
      {onDeleteStream}
      {onSaveStream}
      {onExportStream}
      {onImportStream}
    />
  </SvelteFlow>
  <div class="absolute top-1 left-0 w-40">
    <StreamList {streamNames} currentStream={streamState.id} {streamActivities} {changeStream} />
  </div>
  <div class="absolute right-0 top-1 w-60">
    <AgentList {agentDefs} {onAddAgent} />
  </div>
</div>

{#if newStreamModal}
  <Modal title="New Stream" bind:open={newStreamModal} classBackdrop="bg-transparent">
    <form onsubmit={handleCreateNewStream} autocomplete="off">
      <div class="flex flex-col">
        <label for="stream_name" class="mb-2 text-sm font-medium text-gray-900 dark:text-white"
          >Stream Name</label
        >
        <input
          bind:this={newStreamInput}
          type="text"
          id="stream_name"
          bind:value={newStreamName}
          class="block p-2 w-full text-sm text-gray-900 bg-gray-50 rounded-lg border border-gray-300 focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500"
          placeholder="Stream Name"
        />
      </div>
      <div class="flex justify-end mt-4">
        <GradientButton color="pinkToOrange">Create</GradientButton>
      </div>
    </form>
  </Modal>
{/if}

{#if renameStreamModal}
  <Modal title="Rename Stream" bind:open={renameStreamModal} classBackdrop="bg-transparent">
    <form onsubmit={handleRenameStream} autocomplete="off">
      <div class="flex flex-col">
        <label for="stream_name" class="mb-2 text-sm font-medium text-gray-900 dark:text-white"
          >Stream Name</label
        >
        <input
          bind:this={renameStreamInput}
          type="text"
          id="stream_name"
          bind:value={renameStreamName}
          class="block p-2 w-full text-sm text-gray-900 bg-gray-50 rounded-lg border border-gray-300 focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500"
          placeholder="Stream Name"
        />
      </div>
      <div class="flex justify-end mt-4">
        <GradientButton color="pinkToOrange">Rename</GradientButton>
      </div>
    </form>
  </Modal>
{/if}

{#if deleteStreamModal}
  <Modal
    title="Delete Stream"
    bind:open={deleteStreamModal}
    size="xs"
    autoclose
    classBackdrop="bg-transparent"
  >
    <div class="text-center">
      <ExclamationCircleOutline class="mx-auto mb-4 text-gray-400 w-12 h-12 dark:text-gray-200" />
      <h3 class="mb-5 text-lg font-normal text-gray-500 dark:text-gray-400">
        Are you sure you want to delete this stream?
      </h3>
      <Button onclick={handleDeleteStream} color="red" class="me-2">Delete</Button>
      <Button color="alternative">Cancel</Button>
    </div>
  </Modal>
{/if}

{#if cannotDeleteToast}
  <Toast bind:toastStatus={cannotDeleteToast} class="absolute top-1/2 left-1/2 z-50">
    "main" stream cannot be deleted.
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
