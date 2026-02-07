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
  getCoreSettings,
  setCoreSettings,
  importPreset as importPresetAPI,
  savePreset as savePresetAPI,
  newPresetWithName,
} from "$lib/agent";
import type { PresetFlow, PresetNode, PresetEdge } from "$lib/types";
import { titlebarState } from "$lib/titlebar-state.svelte";

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
  openPaneContextMenu = $state(false);
  paneContextMenuX = $state(0);
  paneContextMenuY = $state(0);
  openAgentList = $state(false);
  agentListX = $state(0);
  agentListY = $state(0);
  agentListOriginX = $state(0);
  agentListOriginY = $state(0);

  // Grid/Snap state
  snapEnabled = $state(true);
  snapGridSize = $state(12);
  showGrid = $state(true);
  gridGap = $state(24);
  modifierPressed = $state(false);

  effectiveSnapGrid = $derived.by(() => {
    const active = this.snapEnabled !== this.modifierPressed;
    return active ? ([this.snapGridSize, this.snapGridSize] as [number, number]) : undefined;
  });

  // Dialog state (shared between menubar and pane context menu)
  openNewPresetDialog = $state(false);
  openSaveAsDialog = $state(false);
  saveAsName = $state("");

  // Derived
  preset_id = $derived.by(() => this.props.preset_id());
  name = $derived.by(() => this.props.flow().name);
  bgColor = $derived(this.running ? BG_COLORS[0] : BG_COLORS[1]);
  selectedCount = $derived(this.nodes.filter((n) => n.selected).length);

  constructor(props: EditorStateProps) {
    this.props = props;

    // Load grid settings from CoreSettings
    const settings = getCoreSettings();
    this.snapEnabled = settings.snap_enabled ?? true;
    this.snapGridSize = settings.snap_grid_size ?? 12;
    this.showGrid = settings.show_grid ?? true;
    this.gridGap = settings.grid_gap ?? 24;

    // Sync nodes/edges from page data (separated from running to avoid overwriting optimistic updates)
    $effect.pre(() => {
      const flow = this.props.flow();
      this.nodes = [...flow.nodes];
      this.edges = [...flow.edges];
    });

    // Sync running only when preset_id changes (not on every flow update)
    $effect.pre(() => {
      const _id = this.props.preset_id();
      this.running = this.props.flow().running ?? false;
    });

    // Sync titlebar state
    $effect.pre(() => {
      const flow = this.props.flow();
      titlebarState.title = flow.name;
      titlebarState.showActions = true;
      titlebarState.showMenubar = true;
      titlebarState.presetId = this.props.preset_id();
      titlebarState.presetName = flow.name;
      titlebarState.onStart = () => this.startPreset();
      titlebarState.onStop = () => this.stopPreset();
      titlebarState.onShowNewDialog = () => { this.showNewPresetDialog(); };
      titlebarState.onSavePreset = () => { this.savePreset(); };
      titlebarState.onShowSaveAsDialog = () => { this.showSaveAsDialog(); };
      titlebarState.onImportPreset = () => { this.importPresetAndNavigate(); };
      titlebarState.onExportPreset = () => { this.exportPreset(); };
    });

    $effect(() => {
      titlebarState.running = this.running;
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
    this.openPaneContextMenu = false;
    this.openAgentList = false;
    this.nodeContextMenuX = x;
    this.nodeContextMenuY = y;
    this.openNodeContextMenu = true;
  }

  hideNodeContextMenu() {
    this.openNodeContextMenu = false;
  }

  showPaneContextMenu(x: number, y: number) {
    this.openNodeContextMenu = false;
    this.openAgentList = false;
    this.paneContextMenuX = x;
    this.paneContextMenuY = y;
    this.openPaneContextMenu = true;
  }

  hidePaneContextMenu() {
    this.openPaneContextMenu = false;
  }

  showAgentList(x: number, y: number) {
    this.hideNodeContextMenu();
    this.hidePaneContextMenu();
    this.agentListOriginX = x;
    this.agentListOriginY = y;
    const POPUP_W = 256;
    const POPUP_H = 320;
    const HEADER_H = 40;
    const cx = x - POPUP_W / 2;
    const cy = y - HEADER_H / 2;
    this.agentListX = Math.max(0, Math.min(cx, window.innerWidth - POPUP_W));
    this.agentListY = Math.max(0, Math.min(cy, window.innerHeight - POPUP_H));
    this.openAgentList = true;
  }

  hideAgentList() {
    this.openAgentList = false;
  }

  // --- Dialogs ---

  showNewPresetDialog() {
    this.openNewPresetDialog = true;
  }

  showSaveAsDialog() {
    this.saveAsName = this.name;
    this.openSaveAsDialog = true;
  }

  // --- Grid/Snap ---

  toggleSnap() {
    this.snapEnabled = !this.snapEnabled;
    this.saveGridSettings();
  }

  toggleGrid() {
    this.showGrid = !this.showGrid;
    this.saveGridSettings();
  }

  private async saveGridSettings() {
    const settings = getCoreSettings();
    settings.snap_enabled = this.snapEnabled;
    settings.snap_grid_size = this.snapGridSize;
    settings.show_grid = this.showGrid;
    settings.grid_gap = this.gridGap;
    await setCoreSettings(settings);
  }

  // --- Alignment ---

  async alignNodes(direction: "left" | "center" | "right" | "top" | "middle" | "bottom") {
    const selectedNodes = this.nodes.filter((n) => n.selected);
    if (selectedNodes.length < 2) return;

    const updates: { id: string; x: number; y: number }[] = [];

    switch (direction) {
      case "left": {
        const target = Math.min(...selectedNodes.map((n) => n.position.x));
        for (const n of selectedNodes) {
          updates.push({ id: n.id, x: target, y: n.position.y });
        }
        break;
      }
      case "right": {
        const target = Math.max(
          ...selectedNodes.map((n) => n.position.x + (n.measured?.width ?? n.width ?? 200)),
        );
        for (const n of selectedNodes) {
          const w = n.measured?.width ?? n.width ?? 200;
          updates.push({ id: n.id, x: target - w, y: n.position.y });
        }
        break;
      }
      case "center": {
        const positions = selectedNodes.map((n) => {
          const w = n.measured?.width ?? n.width ?? 200;
          return n.position.x + w / 2;
        });
        const target = (Math.min(...positions) + Math.max(...positions)) / 2;
        for (const n of selectedNodes) {
          const w = n.measured?.width ?? n.width ?? 200;
          updates.push({ id: n.id, x: target - w / 2, y: n.position.y });
        }
        break;
      }
      case "top": {
        const target = Math.min(...selectedNodes.map((n) => n.position.y));
        for (const n of selectedNodes) {
          updates.push({ id: n.id, x: n.position.x, y: target });
        }
        break;
      }
      case "bottom": {
        const target = Math.max(
          ...selectedNodes.map((n) => n.position.y + (n.measured?.height ?? n.height ?? 100)),
        );
        for (const n of selectedNodes) {
          const h = n.measured?.height ?? n.height ?? 100;
          updates.push({ id: n.id, x: n.position.x, y: target - h });
        }
        break;
      }
      case "middle": {
        const positions = selectedNodes.map((n) => {
          const h = n.measured?.height ?? n.height ?? 100;
          return n.position.y + h / 2;
        });
        const target = (Math.min(...positions) + Math.max(...positions)) / 2;
        for (const n of selectedNodes) {
          const h = n.measured?.height ?? n.height ?? 100;
          updates.push({ id: n.id, x: n.position.x, y: target - h / 2 });
        }
        break;
      }
    }

    for (const u of updates) {
      this.svelteFlow.updateNode(u.id, { position: { x: u.x, y: u.y } });
    }
    await Promise.all(updates.map((u) => updateAgentSpec(u.id, { x: u.x, y: u.y })));
  }

  async distributeNodes(direction: "horizontal" | "vertical") {
    const selectedNodes = this.nodes.filter((n) => n.selected);
    if (selectedNodes.length < 3) return;

    const updates: { id: string; x: number; y: number }[] = [];

    if (direction === "horizontal") {
      const sorted = [...selectedNodes].sort((a, b) => a.position.x - b.position.x);
      const first = sorted[0].position.x;
      const lastNode = sorted[sorted.length - 1];
      const last = lastNode.position.x + (lastNode.measured?.width ?? lastNode.width ?? 200);
      const totalNodeWidth = sorted.reduce(
        (sum, n) => sum + (n.measured?.width ?? n.width ?? 200),
        0,
      );
      const gap = (last - first - totalNodeWidth) / (sorted.length - 1);

      let x = first;
      for (const n of sorted) {
        const w = n.measured?.width ?? n.width ?? 200;
        updates.push({ id: n.id, x, y: n.position.y });
        x += w + gap;
      }
    } else {
      const sorted = [...selectedNodes].sort((a, b) => a.position.y - b.position.y);
      const first = sorted[0].position.y;
      const lastNode = sorted[sorted.length - 1];
      const last = lastNode.position.y + (lastNode.measured?.height ?? lastNode.height ?? 100);
      const totalNodeHeight = sorted.reduce(
        (sum, n) => sum + (n.measured?.height ?? n.height ?? 100),
        0,
      );
      const gap = (last - first - totalNodeHeight) / (sorted.length - 1);

      let y = first;
      for (const n of sorted) {
        const h = n.measured?.height ?? n.height ?? 100;
        updates.push({ id: n.id, x: n.position.x, y });
        y += h + gap;
      }
    }

    for (const u of updates) {
      this.svelteFlow.updateNode(u.id, { position: { x: u.x, y: u.y } });
    }
    await Promise.all(updates.map((u) => updateAgentSpec(u.id, { x: u.x, y: u.y })));
  }

  // --- Navigate helpers ---

  async importPresetAndNavigate() {
    const id = await this.importPreset();
    if (id) {
      const { goto } = await import("$app/navigation");
      goto(`/preset_editor/${id}`, { invalidateAll: true });
    }
  }

  async newPresetAndNavigate(name: string) {
    const id = await this.newPreset(name);
    if (id) {
      const { goto } = await import("$app/navigation");
      goto(`/preset_editor/${id}`, { invalidateAll: true });
    }
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
