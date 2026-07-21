import type { PresetEdge, PresetFlow, PresetNode } from "$lib/types";

const KEY_SEP = "\u0000";

/** Stable identity key for a connection tuple (source, sourceHandle, target, targetHandle). */
export function connKey(
  source: string,
  sourceHandle: string | null | undefined,
  target: string,
  targetHandle: string | null | undefined,
): string {
  return (
    source + KEY_SEP + (sourceHandle ?? "") + KEY_SEP + target + KEY_SEP + (targetHandle ?? "")
  );
}

export type ReconcileResult = {
  nodes: PresetNode[];
  edges: PresetEdge[];
  /** Agent ids present in the current flow but absent from the target. */
  removedAgentIds: Set<string>;
  /** connKey()s of edges present in the current flow but absent from the target. */
  removedConnKeys: Set<string>;
  /** False when the current flow already matches the target exactly. */
  changed: boolean;
};

function edgeKey(edge: PresetEdge): string {
  return connKey(edge.source, edge.sourceHandle, edge.target, edge.targetHandle);
}

/**
 * Node data duplicates the spec's x/y/width/height, but local move/resize
 * updates only node.position/width/height (and the backend), leaving the data
 * copy stale. Geometry is compared separately via position/width/height, so
 * exclude it from the data comparison to keep unchanged nodes identical.
 */
function dataWithoutGeometry(data: Record<string, unknown>): Record<string, unknown> {
  const { x: _x, y: _y, width: _w, height: _h, ...rest } = data;
  return rest;
}

/**
 * Merge a freshly loaded target flow (from presetToFlow) into the current
 * canvas nodes/edges, preserving object identity wherever nothing changed.
 *
 * Nodes are keyed by agent id: surviving nodes keep their current array order
 * (SvelteFlow stacking order) and additions are appended. A surviving node is
 * returned as the identical object when its geometry (position/width/height)
 * and data are unchanged; otherwise it is shallow-copied with the target's
 * geometry and data, preserving local-only state such as `selected`.
 *
 * Edges are keyed by their connection tuple: surviving edges keep the existing
 * object (id, selected) and only take the target's `style`; additions are
 * appended as-is. The viewport is not touched.
 */
export function reconcileFlow(
  curNodes: PresetNode[],
  curEdges: PresetEdge[],
  target: PresetFlow,
): ReconcileResult {
  let changed = false;

  // Nodes
  const targetNodes = new Map(target.nodes.map((n) => [n.id, n]));
  const curNodeIds = new Set(curNodes.map((n) => n.id));
  const removedAgentIds = new Set<string>();
  const nodes: PresetNode[] = [];

  for (const cur of curNodes) {
    const tgt = targetNodes.get(cur.id);
    if (!tgt) {
      removedAgentIds.add(cur.id);
      changed = true;
      continue;
    }
    const sameGeometry =
      cur.position.x === tgt.position.x &&
      cur.position.y === tgt.position.y &&
      cur.width === tgt.width &&
      cur.height === tgt.height;
    if (
      sameGeometry &&
      JSON.stringify(dataWithoutGeometry(cur.data)) ===
        JSON.stringify(dataWithoutGeometry(tgt.data))
    ) {
      nodes.push(cur);
    } else {
      nodes.push({
        ...cur,
        position: tgt.position,
        width: tgt.width,
        height: tgt.height,
        data: tgt.data,
      });
      changed = true;
    }
  }
  for (const tgt of target.nodes) {
    if (!curNodeIds.has(tgt.id)) {
      nodes.push(tgt);
      changed = true;
    }
  }

  // Edges
  const targetEdges = new Map(target.edges.map((e) => [edgeKey(e), e]));
  const curEdgeKeys = new Set(curEdges.map(edgeKey));
  const removedConnKeys = new Set<string>();
  const edges: PresetEdge[] = [];

  for (const cur of curEdges) {
    const key = edgeKey(cur);
    const tgt = targetEdges.get(key);
    if (!tgt) {
      removedConnKeys.add(key);
      changed = true;
      continue;
    }
    if (cur.style === tgt.style) {
      edges.push(cur);
    } else {
      edges.push({ ...cur, style: tgt.style });
      changed = true;
    }
  }
  for (const tgt of target.edges) {
    if (!curEdgeKeys.has(edgeKey(tgt))) {
      edges.push(tgt);
      changed = true;
    }
  }

  return {
    nodes: changed ? nodes : curNodes,
    edges: changed ? edges : curEdges,
    removedAgentIds,
    removedConnKeys,
    changed,
  };
}
