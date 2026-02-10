import { toast } from "svelte-sonner";
import {
  addAgent,
  addAgentsAndConnections,
  addConnection,
  newAgentSpec,
  removeAgent,
  removeConnection,
  setAgentConfigs,
  startAgent,
  stopAgent,
  updateAgentSpec,
  type AgentSpec,
} from "tauri-plugin-modular-agent-api";

import { agentSpecToNode, connectionSpecToEdge, edgeToConnectionSpec, getEdgeColor } from "$lib/agent";
import type { PresetNode, PresetEdge } from "$lib/types";

import type { EditorState } from "./context.svelte";

// --- Command interface ---

export interface Command {
  readonly label: string;
  execute(editor: EditorState): Promise<void>;
  undo(editor: EditorState): Promise<void>;
  /** Remap a node/edge ID when another command's execute/undo re-creates entities with new IDs. */
  remapId?(oldId: string, newId: string): void;
}

// --- Helper ---

export function nodeToAgentSpec(node: PresetNode): AgentSpec {
  return {
    ...node.data,
    id: node.id,
    x: node.position.x,
    y: node.position.y,
    width: node.width ?? node.measured?.width,
    height: node.height ?? node.measured?.height,
  };
}

// --- CommandHistory ---

const COALESCE_WINDOW_MS = 500;

export class CommandHistory {
  undoStack = $state.raw<Command[]>([]);
  redoStack = $state.raw<Command[]>([]);
  canUndo = $derived(this.undoStack.length > 0);
  canRedo = $derived(this.redoStack.length > 0);
  /** Index into the undo stack marking the last-saved position. -1 = unreachable (permanently dirty until next save). */
  savedIndex = $state(0);
  dirty = $derived(this.undoStack.length !== this.savedIndex);
  executing = false;
  private lastPushTime = 0;
  private readonly maxLength: number;
  /** Temporary storage for ID remaps reported by commands during execute/undo. */
  pendingRemaps: Array<{ oldId: string; newId: string }> = [];

  constructor(maxLength: number) {
    this.maxLength = maxLength;
  }

  markSaved() {
    this.savedIndex = this.undoStack.length;
  }

  async executeAndPush(editor: EditorState, cmd: Command): Promise<void> {
    if (this.executing) return;
    this.executing = true;
    try {
      await cmd.execute(editor);
      if (this.savedIndex > this.undoStack.length) this.savedIndex = -1;
      this.undoStack = [...this.undoStack, cmd];
      this.trimUndo();
      this.redoStack = [];
      this.lastPushTime = Date.now();
    } finally {
      this.executing = false;
    }
  }

  /** Push a command that was already executed (for operations where SvelteFlow handles the initial execution). */
  push(cmd: Command): void {
    if (this.savedIndex > this.undoStack.length) this.savedIndex = -1;
    this.undoStack = [...this.undoStack, cmd];
    this.trimUndo();
    this.redoStack = [];
    this.lastPushTime = Date.now();
  }

  /** Push with coalescing for config changes. If the last command is an UpdateConfigCommand for the same node+key within the window, merge instead of pushing. */
  pushCoalescing(cmd: UpdateConfigCommand): void {
    const now = Date.now();
    if (
      this.undoStack.length > 0 &&
      now - this.lastPushTime < COALESCE_WINDOW_MS
    ) {
      const last = this.undoStack[this.undoStack.length - 1];
      if (
        last instanceof UpdateConfigCommand &&
        last.nodeId === cmd.nodeId &&
        last.key === cmd.key
      ) {
        // Merge: keep original oldValue/oldConfigs, update newValue/newConfigs
        last.newValue = cmd.newValue;
        last.newConfigs = cmd.newConfigs;
        // Content changed at same stack length — invalidate if it was the save point
        if (this.savedIndex === this.undoStack.length) this.savedIndex = -1;
        this.lastPushTime = now;
        return;
      }
    }
    if (this.savedIndex > this.undoStack.length) this.savedIndex = -1;
    this.undoStack = [...this.undoStack, cmd];
    this.trimUndo();
    this.redoStack = [];
    this.lastPushTime = now;
  }

  async undo(editor: EditorState): Promise<boolean> {
    if (this.executing || this.undoStack.length === 0) return false;
    this.executing = true;
    const cmd = this.undoStack[this.undoStack.length - 1];
    try {
      this.pendingRemaps = [];
      await cmd.undo(editor);
      this.undoStack = this.undoStack.slice(0, -1);
      this.redoStack = [...this.redoStack, cmd];
      this.propagateRemaps(this.undoStack);
      return true;
    } catch (e) {
      console.error("Undo failed:", cmd.label, e);
      this.clear();
      toast.error("Undo failed", { description: String(e) });
      return false;
    } finally {
      this.executing = false;
    }
  }

  async redo(editor: EditorState): Promise<boolean> {
    if (this.executing || this.redoStack.length === 0) return false;
    this.executing = true;
    const cmd = this.redoStack[this.redoStack.length - 1];
    try {
      this.pendingRemaps = [];
      await cmd.execute(editor);
      this.redoStack = this.redoStack.slice(0, -1);
      this.undoStack = [...this.undoStack, cmd];
      this.trimUndo();
      this.propagateRemaps(this.redoStack);
      return true;
    } catch (e) {
      console.error("Redo failed:", cmd.label, e);
      this.clear();
      toast.error("Redo failed", { description: String(e) });
      return false;
    } finally {
      this.executing = false;
    }
  }

  clear() {
    this.undoStack = [];
    this.redoStack = [];
    this.savedIndex = -1;
  }

  private trimUndo() {
    if (this.undoStack.length > this.maxLength) {
      const removed = this.undoStack.length - this.maxLength;
      if (this.savedIndex < removed) {
        this.savedIndex = -1;
      } else {
        this.savedIndex -= removed;
      }
      this.undoStack = this.undoStack.slice(-this.maxLength);
    }
  }

  private propagateRemaps(stack: Command[]) {
    if (this.pendingRemaps.length === 0) return;
    for (const { oldId, newId } of this.pendingRemaps) {
      for (const c of stack) {
        c.remapId?.(oldId, newId);
      }
    }
  }
}

// --- Command classes ---

// ── AddAgentCommand ──

export class AddAgentCommand implements Command {
  readonly label = "Add Agent";
  private node: PresetNode | null = null;

  constructor(
    private presetId: string,
    private agentName: string,
    private flowPos: { x: number; y: number },
  ) {}

  async execute(editor: EditorState) {
    const prevNodeId = this.node?.id;
    const spec = await newAgentSpec(this.agentName);
    spec.x = this.flowPos.x;
    spec.y = this.flowPos.y;
    const id = await addAgent(this.presetId, spec);
    spec.id = id;
    this.node = agentSpecToNode(spec);
    editor.nodes = [...editor.nodes, this.node];
    if (editor.running && !this.node.data.disabled) {
      await startAgent(this.node.id).catch((e) => {
        console.error("Failed to start agent:", e);
        toast.error("Agent added but failed to start", { description: String(e) });
      });
    }
    // Report ID remap for redo case (backend assigned a new ID)
    if (prevNodeId && prevNodeId !== id) {
      editor.history.pendingRemaps.push({ oldId: prevNodeId, newId: id });
    }
  }

  async undo(editor: EditorState) {
    if (!this.node) return;
    // Remove connected edges first
    const connectedEdges = editor.edges.filter(
      (e) => e.source === this.node!.id || e.target === this.node!.id,
    );
    for (const e of connectedEdges) {
      await removeConnection(this.presetId, edgeToConnectionSpec(e)).catch(() => {});
    }
    await removeAgent(this.presetId, this.node.id);
    editor.nodes = editor.nodes.filter((n) => n.id !== this.node!.id);
    editor.edges = editor.edges.filter(
      (e) => e.source !== this.node!.id && e.target !== this.node!.id,
    );
  }

  remapId(oldId: string, newId: string) {
    if (this.node && this.node.id === oldId) {
      this.node = { ...this.node, id: newId, data: { ...this.node.data, id: newId } };
    }
  }
}

// ── DeleteCommand ──

export class DeleteCommand implements Command {
  label: string;
  private deletedNodes: PresetNode[];
  private deletedEdges: PresetEdge[];

  constructor(
    private presetId: string,
    deletedNodes: PresetNode[],
    deletedEdges: PresetEdge[],
    label?: string,
  ) {
    this.label = label ?? "Delete";
    // Deep copy to prevent stale references
    this.deletedNodes = deletedNodes.map((n) => ({
      ...n,
      data: { ...n.data },
      position: { ...n.position },
    }));
    this.deletedEdges = deletedEdges.map((e) => ({ ...e }));
  }

  async execute(editor: EditorState) {
    // Remove edges first, then nodes
    for (const e of this.deletedEdges) {
      await removeConnection(this.presetId, edgeToConnectionSpec(e)).catch(() => {});
    }
    for (const n of this.deletedNodes) {
      await removeAgent(this.presetId, n.id).catch(() => {});
    }
    const nodeIds = new Set(this.deletedNodes.map((n) => n.id));
    const edgeIds = new Set(this.deletedEdges.map((e) => e.id));
    editor.nodes = editor.nodes.filter((n) => !nodeIds.has(n.id));
    editor.edges = editor.edges.filter((e) => !edgeIds.has(e.id));
  }

  async undo(editor: EditorState) {
    const specs = this.deletedNodes.map(nodeToAgentSpec);
    const connSpecs = this.deletedEdges.map(edgeToConnectionSpec);
    const [addedAgents, addedConns] = await addAgentsAndConnections(
      this.presetId,
      specs,
      connSpecs,
    );

    // Build ID mapping (backend preserves array order)
    const oldToNewId = new Map<string, string>();
    for (let i = 0; i < specs.length && i < addedAgents.length; i++) {
      if (specs[i].id && addedAgents[i].id) {
        oldToNewId.set(specs[i].id!, addedAgents[i].id!);
      }
    }

    // Report ID remaps for other commands in the stack
    for (const [oldId, newId] of oldToNewId) {
      if (oldId !== newId) {
        editor.history.pendingRemaps.push({ oldId, newId });
      }
    }

    // Rebuild nodes/edges from returned specs
    const newNodes = addedAgents.map(agentSpecToNode);
    const newEdges = addedConns.map(connectionSpecToEdge);

    editor.nodes = [...editor.nodes, ...newNodes];
    editor.edges = [...editor.edges, ...newEdges];

    // Update internal references for future redo
    this.deletedNodes = newNodes.map((n) => ({
      ...n,
      data: { ...n.data },
      position: { ...n.position },
    }));
    this.deletedEdges = newEdges.map((e) => ({ ...e }));

    // Start agents if preset is running
    if (editor.running) {
      for (const n of newNodes) {
        if (!n.data.disabled) {
          await startAgent(n.id).catch(() => {});
        }
      }
    }
  }

  remapId(oldId: string, newId: string) {
    this.deletedNodes = this.deletedNodes.map((n) =>
      n.id === oldId
        ? { ...n, id: newId, data: { ...n.data, id: newId }, position: { ...n.position } }
        : n,
    );
    this.deletedEdges = this.deletedEdges.map((e) => {
      const updated = { ...e };
      let changed = false;
      if (e.source === oldId) { updated.source = newId; changed = true; }
      if (e.target === oldId) { updated.target = newId; changed = true; }
      return changed ? updated : e;
    });
  }
}

// ── CutCommand ──

export class CutCommand extends DeleteCommand {
  constructor(
    presetId: string,
    deletedNodes: PresetNode[],
    deletedEdges: PresetEdge[],
  ) {
    super(presetId, deletedNodes, deletedEdges, "Cut");
  }
}

// ── AddConnectionCommand ──

export class AddConnectionCommand implements Command {
  readonly label = "Add Connection";
  private edge: PresetEdge;

  constructor(
    private presetId: string,
    edge: PresetEdge,
  ) {
    this.edge = { ...edge };
  }

  async execute(editor: EditorState) {
    // Redo: create edge with new ID + backend call + add to edges
    const color = getEdgeColor(this.edge.sourceHandle);
    const newEdge: PresetEdge = {
      id: crypto.randomUUID(),
      source: this.edge.source,
      sourceHandle: this.edge.sourceHandle,
      target: this.edge.target,
      targetHandle: this.edge.targetHandle,
      ...(color ? { style: `stroke: ${color};` } : {}),
    };
    await addConnection(this.presetId, edgeToConnectionSpec(newEdge));
    editor.edges = [...editor.edges, newEdge];
    this.edge = { ...newEdge };
  }

  async undo(editor: EditorState) {
    await removeConnection(this.presetId, edgeToConnectionSpec(this.edge));
    editor.edges = editor.edges.filter((e) => e.id !== this.edge.id);
  }

  remapId(oldId: string, newId: string) {
    const updated = { ...this.edge };
    let changed = false;
    if (this.edge.source === oldId) { updated.source = newId; changed = true; }
    if (this.edge.target === oldId) { updated.target = newId; changed = true; }
    if (changed) this.edge = updated;
  }
}

// ── MoveNodesCommand ──

export type NodePositionDelta = {
  id: string;
  oldPosition: { x: number; y: number };
  newPosition: { x: number; y: number };
};

export class MoveNodesCommand implements Command {
  readonly label: string;

  constructor(
    private deltas: NodePositionDelta[],
    label?: string,
  ) {
    this.label = label ?? "Move";
  }

  async execute(editor: EditorState) {
    for (const d of this.deltas) {
      editor.props.svelteFlow.updateNode(d.id, {
        position: { x: d.newPosition.x, y: d.newPosition.y },
      });
      await updateAgentSpec(d.id, { x: d.newPosition.x, y: d.newPosition.y });
    }
  }

  async undo(editor: EditorState) {
    for (const d of this.deltas) {
      editor.props.svelteFlow.updateNode(d.id, {
        position: { x: d.oldPosition.x, y: d.oldPosition.y },
      });
      await updateAgentSpec(d.id, { x: d.oldPosition.x, y: d.oldPosition.y });
    }
  }

  remapId(oldId: string, newId: string) {
    this.deltas = this.deltas.map((d) =>
      d.id === oldId ? { ...d, id: newId } : d,
    );
  }
}

// ── ResizeNodeCommand ──

export class ResizeNodeCommand implements Command {
  readonly label = "Resize";

  constructor(
    private nodeId: string,
    private oldWidth: number | undefined,
    private oldHeight: number | undefined,
    private newWidth: number,
    private newHeight: number,
  ) {}

  async execute(editor: EditorState) {
    editor.props.svelteFlow.updateNode(this.nodeId, {
      width: this.newWidth,
      height: this.newHeight,
    });
    await updateAgentSpec(this.nodeId, { width: this.newWidth, height: this.newHeight });
  }

  async undo(editor: EditorState) {
    editor.props.svelteFlow.updateNode(this.nodeId, {
      width: this.oldWidth,
      height: this.oldHeight,
    });
    await updateAgentSpec(this.nodeId, {
      width: this.oldWidth ?? null,
      height: this.oldHeight ?? null,
    });
  }

  remapId(oldId: string, newId: string) {
    if (this.nodeId === oldId) this.nodeId = newId;
  }
}

// ── PasteCommand ──

export class PasteCommand implements Command {
  readonly label = "Paste";
  private pastedNodes: PresetNode[];
  private pastedEdges: PresetEdge[];

  constructor(
    private presetId: string,
    private agentSpecs: AgentSpec[],
    private connectionSpecs: { source: string; source_handle: string | null; target: string; target_handle: string | null }[],
  ) {
    this.pastedNodes = [];
    this.pastedEdges = [];
  }

  async execute(editor: EditorState) {
    const prevPastedNodes = this.pastedNodes;
    const isFirstPaste = prevPastedNodes.length === 0;

    const [addedAgents, addedConns] = await addAgentsAndConnections(
      this.presetId,
      this.agentSpecs,
      this.connectionSpecs,
    );

    if (addedAgents.length === 0 && addedConns.length === 0) return;

    // Deselect existing nodes/edges
    const { updateNode, updateEdge } = editor.props.svelteFlow;
    editor.nodes.forEach((n) => {
      if (n.selected) updateNode(n.id, { selected: false });
    });
    editor.edges.forEach((e) => {
      if (e.selected) updateEdge(e.id, { selected: false });
    });

    const newNodes: PresetNode[] = [];
    for (const a of addedAgents) {
      if (isFirstPaste) {
        a.x = (a.x ?? 0) + 80;
        a.y = (a.y ?? 0) + 80;
      }
      const node = agentSpecToNode(a);
      node.selected = true;
      newNodes.push(node);
    }

    const newEdges: PresetEdge[] = [];
    for (const conn of addedConns) {
      const edge = connectionSpecToEdge(conn);
      edge.selected = true;
      newEdges.push(edge);
    }

    editor.nodes = [...editor.nodes, ...newNodes];
    editor.edges = [...editor.edges, ...newEdges];

    // Update internal references for undo
    this.pastedNodes = newNodes;
    this.pastedEdges = newEdges;

    // Report ID remaps (redo case: IDs differ from previous execution)
    for (let i = 0; i < prevPastedNodes.length && i < newNodes.length; i++) {
      if (prevPastedNodes[i].id !== newNodes[i].id) {
        editor.history.pendingRemaps.push({
          oldId: prevPastedNodes[i].id,
          newId: newNodes[i].id,
        });
      }
    }

    // Update specs for next redo (with new IDs)
    this.agentSpecs = newNodes.map(nodeToAgentSpec);
    this.connectionSpecs = newEdges.map(edgeToConnectionSpec);

    if (editor.running) {
      for (const node of newNodes) {
        if (!node.data.disabled) {
          await startAgent(node.id).catch((e) => {
            console.error("Failed to start pasted agent:", e);
          });
        }
      }
    }
  }

  async undo(editor: EditorState) {
    for (const e of this.pastedEdges) {
      await removeConnection(this.presetId, edgeToConnectionSpec(e)).catch(() => {});
    }
    for (const n of this.pastedNodes) {
      await removeAgent(this.presetId, n.id).catch(() => {});
    }
    const nodeIds = new Set(this.pastedNodes.map((n) => n.id));
    const edgeIds = new Set(this.pastedEdges.map((e) => e.id));
    editor.nodes = editor.nodes.filter((n) => !nodeIds.has(n.id));
    editor.edges = editor.edges.filter((e) => !edgeIds.has(e.id));
  }

  remapId(oldId: string, newId: string) {
    this.pastedNodes = this.pastedNodes.map((n) =>
      n.id === oldId
        ? { ...n, id: newId, data: { ...n.data, id: newId }, position: { ...n.position } }
        : n,
    );
    this.pastedEdges = this.pastedEdges.map((e) => {
      const updated = { ...e };
      let changed = false;
      if (e.source === oldId) { updated.source = newId; changed = true; }
      if (e.target === oldId) { updated.target = newId; changed = true; }
      return changed ? updated : e;
    });
    this.agentSpecs = this.agentSpecs.map((s) =>
      s.id === oldId ? { ...s, id: newId } : s,
    );
    this.connectionSpecs = this.connectionSpecs.map((c) => {
      const updated = { ...c };
      let changed = false;
      if (c.source === oldId) { updated.source = newId; changed = true; }
      if (c.target === oldId) { updated.target = newId; changed = true; }
      return changed ? updated : c;
    });
  }
}

// ── UpdateConfigCommand ──

export class UpdateConfigCommand implements Command {
  readonly label = "Update Config";

  constructor(
    public nodeId: string,
    public readonly key: string,
    public readonly oldValue: unknown,
    public newValue: unknown,
    public readonly oldConfigs: Record<string, unknown>,
    public newConfigs: Record<string, unknown>,
  ) {}

  async execute(editor: EditorState) {
    const node = editor.nodes.find((n) => n.id === this.nodeId);
    if (node) {
      editor.props.svelteFlow.updateNodeData(this.nodeId, {
        ...node.data,
        configs: this.newConfigs,
      });
    }
    await setAgentConfigs(this.nodeId, this.newConfigs);
  }

  async undo(editor: EditorState) {
    const node = editor.nodes.find((n) => n.id === this.nodeId);
    if (node) {
      editor.props.svelteFlow.updateNodeData(this.nodeId, {
        ...node.data,
        configs: this.oldConfigs,
      });
    }
    await setAgentConfigs(this.nodeId, this.oldConfigs);
  }

  remapId(oldId: string, newId: string) {
    if (this.nodeId === oldId) this.nodeId = newId;
  }
}

// ── UpdateTitleCommand ──

export class UpdateTitleCommand implements Command {
  readonly label = "Update Title";

  constructor(
    private nodeId: string,
    private oldTitle: string | null,
    private newTitle: string | null,
  ) {}

  async execute(editor: EditorState) {
    editor.props.svelteFlow.updateNodeData(this.nodeId, { title: this.newTitle });
  }

  async undo(editor: EditorState) {
    editor.props.svelteFlow.updateNodeData(this.nodeId, { title: this.oldTitle });
  }

  remapId(oldId: string, newId: string) {
    if (this.nodeId === oldId) this.nodeId = newId;
  }
}

// ── ToggleDisabledCommand ──

type DisabledDelta = { id: string; wasDisabled: boolean };

export class ToggleDisabledCommand implements Command {
  readonly label: string;

  constructor(
    private deltas: DisabledDelta[],
    private setDisabled: boolean,
  ) {
    this.label = setDisabled ? "Disable" : "Enable";
  }

  async execute(editor: EditorState) {
    for (const d of this.deltas) {
      editor.props.svelteFlow.updateNodeData(d.id, { disabled: this.setDisabled });
      if (editor.running) {
        if (this.setDisabled) {
          await stopAgent(d.id).catch(() => {});
        } else {
          await startAgent(d.id).catch(() => {});
        }
      }
    }
  }

  async undo(editor: EditorState) {
    for (const d of this.deltas) {
      editor.props.svelteFlow.updateNodeData(d.id, { disabled: d.wasDisabled });
      if (editor.running) {
        if (d.wasDisabled) {
          await stopAgent(d.id).catch(() => {});
        } else {
          await startAgent(d.id).catch(() => {});
        }
      }
    }
  }

  remapId(oldId: string, newId: string) {
    this.deltas = this.deltas.map((d) =>
      d.id === oldId ? { ...d, id: newId } : d,
    );
  }
}

// ── ToggleShowErrCommand ──

type ShowErrDelta = { id: string; wasShowErr: boolean };

export class ToggleShowErrCommand implements Command {
  readonly label = "Toggle Error Display";

  constructor(
    private deltas: ShowErrDelta[],
  ) {}

  async execute(editor: EditorState) {
    for (const d of this.deltas) {
      editor.props.svelteFlow.updateNodeData(d.id, { show_err: !d.wasShowErr });
    }
  }

  async undo(editor: EditorState) {
    for (const d of this.deltas) {
      editor.props.svelteFlow.updateNodeData(d.id, { show_err: d.wasShowErr });
    }
  }

  remapId(oldId: string, newId: string) {
    this.deltas = this.deltas.map((d) =>
      d.id === oldId ? { ...d, id: newId } : d,
    );
  }
}

// --- History cache (per preset ID) ---

const historyCache = new Map<string, CommandHistory>();

export function getOrCreateHistory(presetId: string, maxLength: number): CommandHistory {
  let h = historyCache.get(presetId);
  if (!h) {
    h = new CommandHistory(maxLength);
    historyCache.set(presetId, h);
  }
  return h;
}

export function removeHistory(presetId: string): void {
  historyCache.delete(presetId);
}
