import { invoke } from "@tauri-apps/api/core";

import { getContext, setContext } from "svelte";

import type {
  AgentDefinitions,
  AgentDisplayConfigSpec,
  AgentStream,
  AgentStreamEdge,
  AgentStreamNode,
  Viewport,
} from "tauri-plugin-askit-api";

import type { TAgentStream, TAgentStreamEdge, TAgentStreamNode } from "./types";

export async function importAgentStream(path: string): Promise<AgentStream> {
  return await invoke("import_agent_stream_cmd", { path });
}

export async function renameAgentStream(streamId: string, newName: string): Promise<string> {
  return await invoke("rename_agent_stream_cmd", { streamId, newName });
}

export async function removeAgentStream(streamId: string): Promise<void> {
  await invoke("remove_agent_stream_cmd", { streamId });
}

export async function saveAgentStream(AgentStream: AgentStream): Promise<void> {
  await invoke("save_agent_stream_cmd", { AgentStream });
}

const agentDefinitionsKey = Symbol("agentDefinitions");

export function setAgentDefinitionsContext(defs: AgentDefinitions): void {
  setContext(agentDefinitionsKey, defs);
}

export function getAgentDefinitionsContext(): AgentDefinitions {
  return getContext(agentDefinitionsKey);
}

// Agent Stream

// deserialize: SAgentStream -> AgentStream

export function deserializeAgentStream(stream: AgentStream): TAgentStream {
  // Deserialize nodes first
  const nodes = stream.nodes.map((node) => deserializeAgentStreamNode(node));

  // Create a map to retrieve available handles from node IDs
  const nodeHandles = new Map<string, { inputs: string[]; outputs: string[]; configs: string[] }>();

  nodes.forEach((node) => {
    const inputs = node.data.spec.inputs ?? [];
    const outputs = node.data.spec.outputs ?? [];
    const configs = Object.keys(node.data.spec.configs ?? {});

    nodeHandles.set(node.id, { inputs, outputs, configs });
  });

  // Filter only valid edges
  const validEdges = stream.edges.filter((edge) => {
    const sourceNode = nodeHandles.get(edge.source);
    const targetNode = nodeHandles.get(edge.target);

    if (!sourceNode || !targetNode) return false;

    // Ensure that the source and target handles actually exist
    const isSourceValid = sourceNode.outputs.includes(edge.source_handle ?? "");
    const isTargetValid = edge.target_handle?.startsWith("config:")
      ? targetNode.configs.includes((edge.target_handle ?? "").substring(7))
      : targetNode.inputs.includes(edge.target_handle ?? "");

    return isSourceValid && isTargetValid;
  });

  return {
    id: stream.id,
    name: stream.name,
    nodes: nodes,
    edges: validEdges.map((edge) => deserializeAgentStreamEdge(edge)),
    viewport: stream.viewport,
  };
}

export function deserializeAgentStreamNode(node: AgentStreamNode): TAgentStreamNode {
  const { id, enabled, spec, ...rest } = node as AgentStreamNode & Record<string, any>;

  const { title = null, x = 0, y = 0, width, height, ...extensions } = rest as Record<string, any>;
  return {
    id,
    type: "agent",
    data: {
      name: spec.def_name,
      enabled,
      title,
      spec,
      display_values: spec.display_config_specs ? {} : null,
    },
    position: {
      x,
      y,
    },
    width,
    height,
    extensions,
  };
}

export function deserializeAgentStreamEdge(edge: AgentStreamEdge): TAgentStreamEdge {
  return {
    id: edge.id,
    source: edge.source,
    sourceHandle: edge.source_handle,
    target: edge.target,
    targetHandle: edge.target_handle,
  };
}

// serialize: AgentStream -> SAgentStream

export function serializeAgentStream(
  id: string,
  name: string,
  nodes: TAgentStreamNode[],
  edges: TAgentStreamEdge[],
  viewport: Viewport,
): AgentStream {
  return {
    id,
    name,
    nodes: nodes.map((node) => serializeAgentStreamNode(node)),
    edges: edges.map((edge) => serializeAgentStreamEdge(edge)),
    viewport,
  };
}

export function serializeAgentStreamNode(node: TAgentStreamNode): AgentStreamNode {
  return {
    id: node.id,
    enabled: node.data.enabled,
    spec: node.data.spec,
    title: node.data.title,
    x: node.position.x,
    y: node.position.y,
    width: node.width,
    height: node.height,
    ...(node.extensions ?? {}),
  };
}

export function serializeAgentStreamEdge(edge: TAgentStreamEdge): AgentStreamEdge {
  return {
    id: edge.id,
    source: edge.source,
    source_handle: edge.sourceHandle ?? null,
    target: edge.target,
    target_handle: edge.targetHandle ?? null,
  };
}

// display

export function inferTypeForDisplay(spec: AgentDisplayConfigSpec | undefined, value: any): string {
  let ty = spec?.type;
  if (ty !== undefined && ty !== null && ty === "*") {
    return ty;
  }

  if (value === undefined) {
    return "undefined";
  } else if (value === null) {
    return "null";
  } else if (typeof value === "boolean") {
    return "boolean";
  } else if (Number.isInteger(value)) {
    return "integer";
  } else if (typeof value === "number") {
    return "number";
  } else if (typeof value === "string") {
    if (value.startsWith("data:image/")) {
      return "image";
    } else if (value.includes("\n")) {
      return "text";
    } else {
      return "string";
    }
  } else if (Array.isArray(value)) {
    let tys = new Set<string>();
    for (const v of value) {
      tys.add(inferTypeForDisplay({} as AgentDisplayConfigSpec, v));
    }
    if (tys.size === 1) {
      return tys.values().next().value ?? "object";
    }
    if (tys.has("message")) {
      return "messages";
    }
    if (tys.has("text")) {
      return "text";
    }
    return tys.values().next().value ?? "object";
  } else if (typeof value === "object") {
    if (value?.content !== undefined) {
      return "message";
    } else {
      return "object";
    }
  }
  return "object";
}
