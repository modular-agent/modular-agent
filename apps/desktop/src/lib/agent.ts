import { invoke } from "@tauri-apps/api/core";

import { getContext, setContext } from "svelte";

import type {
  AgentDefinitions,
  AgentDisplayConfigSpec,
  AgentFlow,
  AgentFlowEdge,
  AgentFlowNode,
  Viewport,
} from "tauri-plugin-askit-api";

import type { TAgentFlow, TAgentFlowEdge, TAgentFlowNode } from "./types";

export async function importAgentFlow(path: string): Promise<AgentFlow> {
  return await invoke("import_agent_flow_cmd", { path });
}

export async function renameAgentFlow(flowId: string, newName: string): Promise<string> {
  return await invoke("rename_agent_flow_cmd", { flowId, newName });
}

export async function removeAgentFlow(flowId: string): Promise<void> {
  await invoke("remove_agent_flow_cmd", { flowId });
}

export async function saveAgentFlow(agentFlow: AgentFlow): Promise<void> {
  await invoke("save_agent_flow_cmd", { agentFlow });
}

const agentDefinitionsKey = Symbol("agentDefinitions");

export function setAgentDefinitionsContext(defs: AgentDefinitions): void {
  setContext(agentDefinitionsKey, defs);
}

export function getAgentDefinitionsContext(): AgentDefinitions {
  return getContext(agentDefinitionsKey);
}

// Agent Flow

// deserialize: SAgentFlow -> AgentFlow

export function deserializeAgentFlow(flow: AgentFlow): TAgentFlow {
  // Deserialize nodes first
  const nodes = flow.nodes.map((node) => deserializeAgentFlowNode(node));

  // Create a map to retrieve available handles from node IDs
  const nodeHandles = new Map<string, { inputs: string[]; outputs: string[]; configs: string[] }>();

  nodes.forEach((node) => {
    const inputs = node.data.spec.inputs ?? [];
    const outputs = node.data.spec.outputs ?? [];
    const configs = Object.keys(node.data.spec.configs ?? {});

    nodeHandles.set(node.id, { inputs, outputs, configs });
  });

  // Filter only valid edges
  const validEdges = flow.edges.filter((edge) => {
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
    id: flow.id,
    name: flow.name,
    nodes: nodes,
    edges: validEdges.map((edge) => deserializeAgentFlowEdge(edge)),
    viewport: flow.viewport,
  };
}

export function deserializeAgentFlowNode(node: AgentFlowNode): TAgentFlowNode {
  const { id, enabled, spec, ...rest } = node as AgentFlowNode & Record<string, any>;

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

export function deserializeAgentFlowEdge(edge: AgentFlowEdge): TAgentFlowEdge {
  return {
    id: edge.id,
    source: edge.source,
    sourceHandle: edge.source_handle,
    target: edge.target,
    targetHandle: edge.target_handle,
  };
}

// serialize: AgentFlow -> SAgentFlow

export function serializeAgentFlow(
  id: string,
  name: string,
  nodes: TAgentFlowNode[],
  edges: TAgentFlowEdge[],
  viewport: Viewport,
): AgentFlow {
  return {
    id,
    name,
    nodes: nodes.map((node) => serializeAgentFlowNode(node)),
    edges: edges.map((edge) => serializeAgentFlowEdge(edge)),
    viewport,
  };
}

export function serializeAgentFlowNode(node: TAgentFlowNode): AgentFlowNode {
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

export function serializeAgentFlowEdge(edge: TAgentFlowEdge): AgentFlowEdge {
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
