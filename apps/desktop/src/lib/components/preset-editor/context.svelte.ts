import { getCurrentWindow } from "@tauri-apps/api/window";
import { writeText, readText } from "@tauri-apps/plugin-clipboard-manager";
import { open } from "@tauri-apps/plugin-dialog";

import { getContext, setContext } from "svelte";

import {
  addAgent,
  addAgentsAndConnections,
  addConnection,
  getAgentSpec,
  getPresetSpec,
  newAgentSpec,
  removeAgent,
  removeConnection,
  startAgent,
  startPreset as startPresetAPI,
  stopAgent,
  stopPreset as stopPresetAPI,
  updateAgentSpec,
  updatePresetSpec,
  type AgentSpec,
  type ConnectionSpec,
} from "tauri-plugin-modular-agent-api";
import type { useSvelteFlow } from "@xyflow/svelte";

import {
  agentSpecToNode,
  connectionSpecToEdge,
  edgeToConnectionSpec,
  importPreset as importPresetAPI,
  savePreset as savePresetAPI,
  newPresetWithName,
} from "$lib/agent";
import type { PresetFlow, PresetNode, PresetEdge } from "$lib/types";

const BG_COLORS = ["bg-background dark:bg-background", "bg-muted dark:bg-muted"];

export type EditorStateProps = {
  preset_id: () => string;
  flow: () => PresetFlow;
  svelteFlow: ReturnType<typeof useSvelteFlow>;
};

export class EditorState {
  readonly props: EditorStateProps;

  // Reactive state
  running = $state(false);
  nodes = $state.raw<PresetNode[]>([]);
  edges = $state.raw<PresetEdge[]>([]);
  openNodeContextMenu = $state(false);
  nodeContextMenuX = $state(0);
  nodeContextMenuY = $state(0);

  // Derived
  preset_id = $derived.by(() => this.props.preset_id());
  name = $derived.by(() => this.props.flow().name);
  bgColor = $derived(this.running ? BG_COLORS[0] : BG_COLORS[1]);

  constructor(props: EditorStateProps) {
    this.props = props;

    // Sync nodes/edges from page data (separated from running to avoid overwriting optimistic updates)
    $effect.pre(() => {
      const flow = this.props.flow();
      this.nodes = [...flow.nodes];
      this.edges = [...flow.edges];
      getCurrentWindow().setTitle(flow.name + " - Modular Agent");
    });

    // Sync running only when preset_id changes (not on every flow update)
    $effect.pre(() => {
      const _id = this.props.preset_id();
      this.running = this.props.flow().running ?? false;
    });
  }

  // --- SvelteFlow helpers ---

  private get svelteFlow() {
    return this.props.svelteFlow;
  }

  // --- Preset operations ---

  async savePreset() {
    const s = await getPresetSpec(this.preset_id);
    if (!s) return;
    await savePresetAPI(this.name, s);
  }

  async startPreset() {
    await startPresetAPI(this.preset_id);
    this.running = true;
  }

  async stopPreset() {
    await stopPresetAPI(this.preset_id);
    this.running = false;
  }

  async exportPreset() {
    const s = await getPresetSpec(this.preset_id);
    const jsonStr = JSON.stringify(s, null, 2);
    const blob = new Blob([jsonStr], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = this.name + ".json";
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }

  async importPreset(): Promise<string | null> {
    const file = await open({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!file) return null;

    const name = this.name;
    const lastSlash = name.lastIndexOf("/");
    const targetDir = lastSlash >= 0 ? name.substring(0, lastSlash) : "";

    const id = await importPresetAPI(file as string, targetDir);
    return id;
  }

  async newPreset(name: string): Promise<string | null> {
    const new_id = await newPresetWithName(name);
    return new_id || null;
  }

  // --- Node/Edge operations ---

  async addAgent(agentName: string, position?: { x: number; y: number }) {
    const snode = await newAgentSpec(agentName);
    const xy =
      position !== undefined
        ? this.svelteFlow.screenToFlowPosition(position)
        : this.svelteFlow.screenToFlowPosition({
            x: window.innerWidth * 0.45,
            y: window.innerHeight * 0.3,
          });
    snode.x = xy.x;
    snode.y = xy.y;
    const id = await addAgent(this.preset_id, snode);
    snode.id = id;
    const new_node = agentSpecToNode(snode);
    this.nodes = [...this.nodes, new_node];

    if (this.running) {
      await startAgent(new_node.id);
    }
  }

  async deleteNodes(deletedNodes: PresetNode[]) {
    for (const n of deletedNodes) {
      await removeAgent(this.preset_id, n.id);
    }
  }

  async deleteEdges(deletedEdges: PresetEdge[]) {
    for (const e of deletedEdges) {
      const ch = edgeToConnectionSpec(e);
      await removeConnection(this.preset_id, ch);
    }
  }

  async handleOnDelete({
    nodes: deletedNodes,
    edges: deletedEdges,
  }: {
    nodes?: PresetNode[];
    edges?: PresetEdge[];
  }) {
    if (deletedEdges && deletedEdges.length > 0) {
      await this.deleteEdges(deletedEdges);
    }
    if (deletedNodes && deletedNodes.length > 0) {
      await this.deleteNodes(deletedNodes);
    }
  }

  async handleOnConnect(connection: { source: string; target: string; sourceHandle?: string | null; targetHandle?: string | null }) {
    const edge = {
      id: crypto.randomUUID(),
      ...connection,
    } as PresetEdge;

    await addConnection(this.preset_id, edgeToConnectionSpec(edge));
  }

  // --- Selection helpers ---

  selectedNodesAndEdges(): [PresetNode[], PresetEdge[]] {
    const selectedNodes = this.nodes.filter((node) => node.selected);
    const selectedEdges = this.edges.filter((edge) => edge.selected);
    return [selectedNodes, selectedEdges];
  }

  // --- Clipboard operations ---

  private async copySelected() {
    const [selectedNodes, selectedEdges] = this.selectedNodesAndEdges();

    const agents = (
      await Promise.all(selectedNodes.map(async (node) => await getAgentSpec(node.id)))
    ).filter((spec) => spec !== null);
    const connections = selectedEdges.map((edge) => edgeToConnectionSpec(edge));

    const clipboardData = { agents, connections };
    await writeText(JSON.stringify(clipboardData));
  }

  private async readCopied(): Promise<[AgentSpec[], ConnectionSpec[]]> {
    const text = await readText();
    if (!text) {
      return [[], []];
    }
    try {
      const clipboardData = JSON.parse(text);
      return [clipboardData.agents || [], clipboardData.connections || []];
    } catch {
      return [[], []];
    }
  }

  async cutNodesAndEdges() {
    const [selectedNodes, selectedEdges] = this.selectedNodesAndEdges();
    if (selectedNodes.length === 0 && selectedEdges.length === 0) {
      return;
    }

    await this.copySelected();

    for (const edge of selectedEdges) {
      const ch = edgeToConnectionSpec(edge);
      await removeConnection(this.preset_id, ch);
    }
    for (const node of selectedNodes) {
      await removeAgent(this.preset_id, node.id);
    }
    this.nodes = this.nodes.filter((node) => !node.selected);
    this.edges = this.edges.filter((edge) => !edge.selected);
  }

  async copyNodesAndEdges() {
    const [selectedNodes, selectedEdges] = this.selectedNodesAndEdges();
    if (selectedNodes.length === 0 && selectedEdges.length === 0) {
      return;
    }

    await this.copySelected();
  }

  async pasteNodesAndEdges() {
    const { updateNode, updateEdge } = this.svelteFlow;

    this.nodes.forEach((node) => {
      if (node.selected) {
        updateNode(node.id, { selected: false });
      }
    });
    this.edges.forEach((edge) => {
      if (edge.selected) {
        updateEdge(edge.id, { selected: false });
      }
    });

    const [copiedAgents, copiedConnections] = await this.readCopied();

    if (copiedAgents.length === 0) {
      return;
    }

    const [added_agents, added_connections] = await addAgentsAndConnections(
      this.preset_id,
      copiedAgents,
      copiedConnections,
    );

    if (added_agents.length === 0 && added_connections.length === 0) return;

    const new_nodes: PresetNode[] = [];
    for (const a of added_agents) {
      a.x += 80;
      a.y += 80;
      const new_node = agentSpecToNode(a);
      new_node.selected = true;
      new_nodes.push(new_node);
    }

    const new_edges: PresetEdge[] = [];
    for (const conn of added_connections) {
      const new_edge = connectionSpecToEdge(conn);
      new_edge.selected = true;
      new_edges.push(new_edge);
    }

    this.nodes = [...this.nodes, ...new_nodes];
    this.edges = [...this.edges, ...new_edges];

    if (this.running) {
      for (const node of new_nodes) {
        if (node.data.disabled) continue;
        await startAgent(node.id);
      }
    }
  }

  selectAllNodesAndEdges() {
    const { updateNode, updateEdge } = this.svelteFlow;
    this.nodes.forEach((node) => {
      updateNode(node.id, { selected: true });
    });
    this.edges.forEach((edge) => {
      updateEdge(edge.id, { selected: true });
    });
  }

  // --- Enable/Disable/ToggleErr ---

  async enable() {
    const [selectedNodes] = this.selectedNodesAndEdges();
    for (const node of selectedNodes) {
      if (node.data.disabled) {
        this.svelteFlow.updateNodeData(node.id, { disabled: false });
        await startAgent(node.id);
      }
    }
  }

  async disable() {
    const [selectedNodes] = this.selectedNodesAndEdges();
    for (const node of selectedNodes) {
      if (!node.data.disabled) {
        this.svelteFlow.updateNodeData(node.id, { disabled: true });
        await stopAgent(node.id);
      }
    }
  }

  toggleErr() {
    const [selectedNodes] = this.selectedNodesAndEdges();
    if (selectedNodes.length === 0) return;
    selectedNodes.forEach((node) => {
      this.svelteFlow.updateNodeData(node.id, { show_err: !node.data.show_err });
    });
  }

  // --- Node drag/move handlers ---

  async handleNodeDragStop(targetNode: PresetNode | null) {
    if (!targetNode) return;
    await updateAgentSpec(targetNode.id, {
      x: targetNode.position.x,
      y: targetNode.position.y,
    });
  }

  async handleSelectionDragStop(draggedNodes: PresetNode[]) {
    for (const node of draggedNodes) {
      await updateAgentSpec(node.id, {
        x: node.position.x,
        y: node.position.y,
      });
    }
  }

  async handleOnMoveEnd(viewport: { x: number; y: number; zoom: number }) {
    await updatePresetSpec(this.preset_id, { viewport });
  }

  // --- Context menu ---

  showNodeContextMenu(x: number, y: number) {
    this.nodeContextMenuX = x;
    this.nodeContextMenuY = y;
    this.openNodeContextMenu = true;
  }

  hideNodeContextMenu() {
    this.openNodeContextMenu = false;
  }
}

// --- Context API ---

const SYMBOL_KEY = "preset-editor";

export function setEditor(props: EditorStateProps): EditorState {
  return setContext(Symbol.for(SYMBOL_KEY), new EditorState(props));
}

export function useEditor(): EditorState {
  return getContext<EditorState>(Symbol.for(SYMBOL_KEY));
}
