import { writeText, readText } from "@tauri-apps/plugin-clipboard-manager";
import { open } from "@tauri-apps/plugin-dialog";

import { goto } from "$app/navigation";
import { getContext, setContext } from "svelte";

import type { useSvelteFlow } from "@xyflow/svelte";
import { toast } from "svelte-sonner";
import {
  getAgentSpec,
  getPresetSpec,
  setAgentConfigs,
  startPreset as startPresetAPI,
  stopPreset as stopPresetAPI,
  updateAgentSpec,
  updatePresetSpec,
  type AgentSpec,
  type ConnectionSpec,
} from "tauri-plugin-modular-agent-api";

import {
  edgeToConnectionSpec,
  getCoreSettings,
  setCoreSettings,
  importPreset as importPresetAPI,
  savePreset as savePresetAPI,
  newPresetWithName,
} from "$lib/agent";
import { tabStore } from "$lib/tab-store.svelte";
import { titlebarState } from "$lib/titlebar-state.svelte";
import type { PresetFlow, PresetNode, PresetEdge } from "$lib/types";

import {
  AddAgentCommand,
  DeleteCommand,
  CutCommand,
  AddConnectionCommand,
  MoveNodesCommand,
  ResizeNodeCommand,
  PasteCommand,
  UpdateConfigCommand,
  UpdateTitleCommand,
  ToggleDisabledCommand,
  ToggleShowErrCommand,
  getOrCreateHistory,
  type NodePositionDelta,
} from "./history.svelte";

async function withErrorToast<T>(fn: () => Promise<T>, message: string): Promise<T | undefined> {
  try {
    return await fn();
  } catch (e) {
    console.error(message, e);
    toast.error(message, { description: String(e) });
    return undefined;
  }
}

async function withErrorLog<T>(fn: () => Promise<T>, message: string): Promise<T | undefined> {
  try {
    return await fn();
  } catch (e) {
    console.error(message, e);
    return undefined;
  }
}

const BG_COLORS = ["bg-background dark:bg-background", "bg-muted dark:bg-muted"];

export type EditorStateProps = {
  preset_id: () => string;
  flow: () => PresetFlow;
  svelteFlow: ReturnType<typeof useSvelteFlow>;
};

export class EditorState {
  readonly props: EditorStateProps;
  readonly history;

  // Whether this editor's tab is currently active (visible)
  active = $state(false);

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

  // Drag state for undo
  private dragStartPositions: Map<string, { x: number; y: number }> | null = null;

  constructor(props: EditorStateProps) {
    this.props = props;
    this.history = getOrCreateHistory(props.preset_id());

    // Load grid settings from CoreSettings
    const settings = getCoreSettings();
    this.snapEnabled = settings.snap_enabled ?? true;
    this.snapGridSize = settings.snap_grid_size ?? 12;
    this.showGrid = settings.show_grid ?? true;
    this.gridGap = settings.grid_gap ?? 24;

    // Sync nodes/edges from flow data
    $effect.pre(() => {
      const flow = this.props.flow();
      this.nodes = [...flow.nodes];
      this.edges = [...flow.edges];
    });

    // Sync running state
    $effect.pre(() => {
      const _id = this.props.preset_id();
      this.running = this.props.flow().running ?? false;
    });

    // Titlebar callbacks — only active editor controls the titlebar
    $effect(() => {
      if (!this.active) return;
      titlebarState.onStart = () => this.startPreset();
      titlebarState.onStop = () => this.stopPreset();
      titlebarState.onShowNewDialog = () => this.showNewPresetDialog();
      titlebarState.onSavePreset = () => this.savePreset();
      titlebarState.onShowSaveAsDialog = () => this.showSaveAsDialog();
      titlebarState.onImportPreset = () => this.importPresetAndNavigate();
      titlebarState.onExportPreset = () => this.exportPreset();
    });

    // Sync titlebar data — only when active
    $effect.pre(() => {
      if (!this.active) return;
      const flow = this.props.flow();
      titlebarState.title = flow.name;
      titlebarState.showActions = true;
      titlebarState.showMenubar = true;
      titlebarState.presetId = this.props.preset_id();
      titlebarState.presetName = flow.name;
    });

    $effect(() => {
      if (!this.active) return;
      titlebarState.running = this.running;
    });

    // Sync tab name when preset name changes (e.g. Save As)
    $effect(() => {
      tabStore.updateName(this.preset_id, this.name);
    });
  }

  // --- SvelteFlow helpers ---

  private get svelteFlow() {
    return this.props.svelteFlow;
  }

  // --- Undo/Redo ---

  async undo() {
    await this.history.undo(this);
  }

  async redo() {
    await this.history.redo(this);
  }

  // --- Preset operations ---

  async savePreset() {
    await withErrorToast(async () => {
      const s = await getPresetSpec(this.preset_id);
      if (!s) return;
      await savePresetAPI(this.name, s);
    }, "Failed to save preset");
  }

  async startPreset() {
    await withErrorToast(async () => {
      await startPresetAPI(this.preset_id);
      this.running = true;
    }, "Failed to start preset");
  }

  async stopPreset() {
    await withErrorToast(async () => {
      await stopPresetAPI(this.preset_id);
      this.running = false;
    }, "Failed to stop preset");
  }

  async exportPreset() {
    await withErrorToast(async () => {
      const s = await getPresetSpec(this.preset_id);
      if (!s) return;
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
    }, "Failed to export preset");
  }

  async importPreset(): Promise<string | null> {
    const file = await open({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!file) return null;

    const result = await withErrorToast(async () => {
      const name = this.name;
      const lastSlash = name.lastIndexOf("/");
      const targetDir = lastSlash >= 0 ? name.substring(0, lastSlash) : "";
      return await importPresetAPI(file as string, targetDir);
    }, "Failed to import preset");
    return result ?? null;
  }

  async newPreset(name: string): Promise<string | null> {
    const result = await withErrorToast(async () => {
      return await newPresetWithName(name);
    }, "Failed to create new preset");
    return result || null;
  }

  // --- Node/Edge operations ---

  async addAgent(agentName: string, position?: { x: number; y: number }) {
    const flowPos =
      position !== undefined
        ? this.svelteFlow.screenToFlowPosition(position)
        : this.svelteFlow.screenToFlowPosition({
            x: window.innerWidth * 0.45,
            y: window.innerHeight * 0.3,
          });
    const cmd = new AddAgentCommand(this.preset_id, agentName, flowPos);
    await withErrorToast(() => this.history.executeAndPush(this, cmd), "Failed to add agent");
  }

  async handleOnDelete({
    nodes: deletedNodes,
    edges: deletedEdges,
  }: {
    nodes?: PresetNode[];
    edges?: PresetEdge[];
  }) {
    const dn = deletedNodes ?? [];
    const de = deletedEdges ?? [];
    if (dn.length === 0 && de.length === 0) return;

    // SvelteFlow already removed items from arrays. Create command for backend sync + undo.
    const cmd = new DeleteCommand(this.preset_id, dn, de);
    // Execute backend deletion, then push to history
    await withErrorToast(async () => {
      await cmd.execute(this);
      this.history.push(cmd);
    }, "Failed to delete");
  }

  async handleOnConnect(connection: {
    source: string;
    target: string;
    sourceHandle?: string | null;
    targetHandle?: string | null;
  }) {
    // SvelteFlow already added the edge via handleBeforeConnect.
    // Find the edge that SvelteFlow just added.
    const edge = this.edges.find(
      (e) =>
        e.source === connection.source &&
        e.target === connection.target &&
        e.sourceHandle === connection.sourceHandle &&
        e.targetHandle === connection.targetHandle,
    );
    if (!edge) return;

    const cmd = new AddConnectionCommand(this.preset_id, edge);
    // Backend call only (edge already in UI). Push without execute.
    const result = await withErrorToast(
      () =>
        import("tauri-plugin-modular-agent-api").then((m) =>
          m.addConnection(this.preset_id, edgeToConnectionSpec(edge)),
        ),
      "Failed to add connection",
    );
    if (result !== undefined) {
      this.history.push(cmd);
    }
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

    // Copy to clipboard first
    try {
      await this.copySelected();
    } catch (e) {
      console.error("Failed to copy:", e);
      toast.error("Failed to copy", { description: String(e) });
      return;
    }

    // Also capture connected edges that will be orphaned
    const nodeIds = new Set(selectedNodes.map((n) => n.id));
    const selectedEdgeIds = new Set(selectedEdges.map((e) => e.id));
    const allAffectedEdges = this.edges.filter(
      (e) => selectedEdgeIds.has(e.id) || nodeIds.has(e.source) || nodeIds.has(e.target),
    );

    const cmd = new CutCommand(this.preset_id, selectedNodes, allAffectedEdges);
    await withErrorToast(() => this.history.executeAndPush(this, cmd), "Failed to cut");
  }

  async copyNodesAndEdges() {
    const [selectedNodes, selectedEdges] = this.selectedNodesAndEdges();
    if (selectedNodes.length === 0 && selectedEdges.length === 0) {
      return;
    }

    await withErrorToast(() => this.copySelected(), "Failed to copy");
  }

  async pasteNodesAndEdges() {
    const [copiedAgents, copiedConnections] = await this.readCopied();
    if (copiedAgents.length === 0) {
      return;
    }

    const cmd = new PasteCommand(this.preset_id, copiedAgents, copiedConnections);
    await withErrorToast(() => this.history.executeAndPush(this, cmd), "Failed to paste");
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
    const targets = selectedNodes.filter((n) => n.data.disabled);
    if (targets.length === 0) return;

    const deltas = targets.map((n) => ({ id: n.id, wasDisabled: true }));
    const cmd = new ToggleDisabledCommand(deltas, false);
    await withErrorToast(() => this.history.executeAndPush(this, cmd), "Failed to enable agent(s)");
  }

  async disable() {
    const [selectedNodes] = this.selectedNodesAndEdges();
    const targets = selectedNodes.filter((n) => !n.data.disabled);
    if (targets.length === 0) return;

    const deltas = targets.map((n) => ({ id: n.id, wasDisabled: false }));
    const cmd = new ToggleDisabledCommand(deltas, true);
    await withErrorToast(() => this.history.executeAndPush(this, cmd), "Failed to disable agent(s)");
  }

  toggleErr() {
    const [selectedNodes] = this.selectedNodesAndEdges();
    if (selectedNodes.length === 0) return;

    const deltas = selectedNodes.map((n) => ({
      id: n.id,
      wasShowErr: n.data.show_err ?? false,
    }));
    const cmd = new ToggleShowErrCommand(deltas);
    // ToggleShowErr is synchronous (no backend call), so execute directly
    cmd.execute(this);
    this.history.push(cmd);
  }

  // --- Node drag/move handlers ---

  handleNodeDragStart(nodes: PresetNode[]) {
    this.dragStartPositions = new Map(
      nodes.filter((n) => n.selected).map((n) => [n.id, { x: n.position.x, y: n.position.y }]),
    );
  }

  async handleNodeDragStop(targetNode: PresetNode | null) {
    if (!targetNode) return;

    // Build deltas from stored start positions
    const deltas: NodePositionDelta[] = [];
    if (this.dragStartPositions) {
      const oldPos = this.dragStartPositions.get(targetNode.id);
      if (oldPos && (oldPos.x !== targetNode.position.x || oldPos.y !== targetNode.position.y)) {
        deltas.push({
          id: targetNode.id,
          oldPosition: oldPos,
          newPosition: { x: targetNode.position.x, y: targetNode.position.y },
        });
      }
    }

    // Persist to backend
    await withErrorLog(
      () =>
        updateAgentSpec(targetNode.id, { x: targetNode.position.x, y: targetNode.position.y }),
      "Failed to update node position",
    );

    if (deltas.length > 0) {
      // Push only (SvelteFlow already moved the node). Don't re-execute.
      this.history.push(new MoveNodesCommand(deltas));
    }
    this.dragStartPositions = null;
  }

  async handleSelectionDragStop(draggedNodes: PresetNode[]) {
    const deltas: NodePositionDelta[] = [];

    for (const node of draggedNodes) {
      const oldPos = this.dragStartPositions?.get(node.id);
      if (oldPos && (oldPos.x !== node.position.x || oldPos.y !== node.position.y)) {
        deltas.push({
          id: node.id,
          oldPosition: oldPos,
          newPosition: { x: node.position.x, y: node.position.y },
        });
      }
      try {
        await updateAgentSpec(node.id, { x: node.position.x, y: node.position.y });
      } catch (e) {
        console.error("Failed to update node position:", node.id, e);
      }
    }

    if (deltas.length > 0) {
      this.history.push(new MoveNodesCommand(deltas));
    }
    this.dragStartPositions = null;
  }

  async handleOnMoveEnd(viewport: { x: number; y: number; zoom: number }) {
    await withErrorLog(
      () => updatePresetSpec(this.preset_id, { viewport }),
      "Failed to update viewport",
    );
  }

  // --- Resize handler ---

  async handleResizeEnd(
    nodeId: string,
    oldWidth: number | undefined,
    oldHeight: number | undefined,
    newWidth: number,
    newHeight: number,
  ) {
    const cmd = new ResizeNodeCommand(nodeId, oldWidth, oldHeight, newWidth, newHeight);
    // SvelteFlow already resized the node. Backend persist + push.
    await withErrorLog(
      () => updateAgentSpec(nodeId, { width: newWidth, height: newHeight }),
      "Failed to update node size",
    );
    this.history.push(cmd);
  }

  // --- Config/Title updates (for undo support) ---

  async updateNodeConfig(nodeId: string, key: string, oldValue: unknown, newValue: unknown) {
    const node = this.nodes.find((n) => n.id === nodeId);
    if (!node) return;

    const oldConfigs = { ...node.data.configs };
    const newConfigs = { ...oldConfigs, [key]: newValue };

    this.svelteFlow.updateNodeData(nodeId, { ...node.data, configs: newConfigs });
    await setAgentConfigs(nodeId, newConfigs);

    const cmd = new UpdateConfigCommand(nodeId, key, oldValue, newValue, oldConfigs, newConfigs);
    this.history.pushCoalescing(cmd);
  }

  updateNodeTitle(nodeId: string, oldTitle: string | null, newTitle: string | null) {
    this.svelteFlow.updateNodeData(nodeId, { title: newTitle });
    const cmd = new UpdateTitleCommand(nodeId, oldTitle, newTitle);
    this.history.push(cmd);
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
    await withErrorLog(async () => {
      const settings = getCoreSettings();
      settings.snap_enabled = this.snapEnabled;
      settings.snap_grid_size = this.snapGridSize;
      settings.show_grid = this.showGrid;
      settings.grid_gap = this.gridGap;
      await setCoreSettings(settings);
    }, "Failed to save grid settings");
  }

  // --- Alignment ---

  async alignNodes(direction: "left" | "center" | "right" | "top" | "middle" | "bottom") {
    const selectedNodes = this.nodes.filter((n) => n.selected);
    if (selectedNodes.length < 2) return;

    // Capture old positions
    const oldPositions = new Map(selectedNodes.map((n) => [n.id, { ...n.position }]));

    // Compute new positions
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

    // Build deltas
    const deltas: NodePositionDelta[] = updates
      .map((u) => ({
        id: u.id,
        oldPosition: oldPositions.get(u.id)!,
        newPosition: { x: u.x, y: u.y },
      }))
      .filter(
        (d) => d.oldPosition.x !== d.newPosition.x || d.oldPosition.y !== d.newPosition.y,
      );

    if (deltas.length === 0) return;

    const cmd = new MoveNodesCommand(deltas, "Align");
    await withErrorToast(() => this.history.executeAndPush(this, cmd), "Failed to align nodes");
  }

  async distributeNodes(direction: "horizontal" | "vertical") {
    const selectedNodes = this.nodes.filter((n) => n.selected);
    if (selectedNodes.length < 3) return;

    // Capture old positions
    const oldPositions = new Map(selectedNodes.map((n) => [n.id, { ...n.position }]));

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

    // Build deltas
    const deltas: NodePositionDelta[] = updates
      .map((u) => ({
        id: u.id,
        oldPosition: oldPositions.get(u.id)!,
        newPosition: { x: u.x, y: u.y },
      }))
      .filter(
        (d) => d.oldPosition.x !== d.newPosition.x || d.oldPosition.y !== d.newPosition.y,
      );

    if (deltas.length === 0) return;

    const cmd = new MoveNodesCommand(deltas, "Distribute");
    await withErrorToast(() => this.history.executeAndPush(this, cmd), "Failed to distribute nodes");
  }

  // --- Navigate helpers ---

  async importPresetAndNavigate() {
    const id = await this.importPreset();
    if (id) {
      // Derive imported preset name from current preset's directory
      const lastSlash = this.name.lastIndexOf("/");
      const dir = lastSlash >= 0 ? this.name.substring(0, lastSlash + 1) : "";
      tabStore.openTab(id, dir + id);
      goto(`/preset_editor/${id}`, { noScroll: true });
    }
  }

  async newPresetAndNavigate(name: string) {
    const id = await this.newPreset(name);
    if (id) {
      tabStore.openTab(id, name);
      goto(`/preset_editor/${id}`, { noScroll: true });
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
