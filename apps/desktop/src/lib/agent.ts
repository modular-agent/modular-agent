import { invoke } from "@tauri-apps/api/core";

import { getContext, setContext } from "svelte";

import type {
  AgentDefinitions,
  AgentConfigSpec,
  AgentSpec,
  AgentStream,
  ChannelSpec,
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

export async function saveAgentStream(stream: AgentStream): Promise<void> {
  await invoke("save_agent_stream_cmd", { stream });
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
  // Deserialize agents first
  const nodes = stream.agents.map((agent) => deserializeAgentStreamNode(agent));

  // Create a map to retrieve available handles from node IDs
  const nodeHandles = new Map<string, { inputs: string[]; outputs: string[]; configs: string[] }>();

  nodes.forEach((node) => {
    const inputs = node.data.inputs ?? [];
    const outputs = node.data.outputs ?? [];
    const configs = Object.keys(node.data.configs ?? {});

    nodeHandles.set(node.id, { inputs, outputs, configs });
  });

  // Filter only valid edges
  const validEdges = stream.channels.filter((ch) => {
    const sourceNode = nodeHandles.get(ch.source);
    const targetNode = nodeHandles.get(ch.target);

    if (!sourceNode || !targetNode) return false;

    // Ensure that the source and target handles actually exist
    const isSourceValid = sourceNode.outputs.includes(ch.source_handle ?? "");
    const isTargetValid = ch.target_handle?.startsWith("config:")
      ? targetNode.configs.includes((ch.target_handle ?? "").substring(7))
      : targetNode.inputs.includes(ch.target_handle ?? "");

    return isSourceValid && isTargetValid;
  });

  return {
    id: stream.id,
    name: stream.name,
    nodes: nodes,
    edges: validEdges.map((ch) => deserializeChannelSpec(ch)),
    viewport: stream.viewport,
  };
}

export function deserializeAgentStreamNode(spec: AgentSpec): TAgentStreamNode {
  return {
    id: spec.id ?? crypto.randomUUID(),
    type: "agent",
    data: { ...spec },
    position: {
      x: spec.x ?? 0,
      y: spec.y ?? 0,
    },
    width: spec.width,
    height: spec.height,
  };
}

export function deserializeChannelSpec(channel: ChannelSpec): TAgentStreamEdge {
  return {
    id: channel.id,
    source: channel.source,
    sourceHandle: channel.source_handle,
    target: channel.target,
    targetHandle: channel.target_handle,
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
    agents: nodes.map((node) => serializeAgentStreamNode(node)),
    channels: edges.map((edge) => serializeAgentStreamEdge(edge)),
    viewport,
  };
}

export function serializeAgentStreamNode(node: TAgentStreamNode): AgentSpec {
  return {
    id: node.id,
    def_name: node.data.def_name,
    inputs: node.data.inputs,
    outputs: node.data.outputs,
    configs: node.data.configs,
    config_specs: node.data.config_specs,
    enabled: node.data.enabled,
    // extensions
    x: node.position.x,
    y: node.position.y,
    width: node.width,
    height: node.height,
  };
}

export function serializeAgentStreamEdge(edge: TAgentStreamEdge): ChannelSpec {
  return {
    id: edge.id,
    source: edge.source,
    source_handle: edge.sourceHandle ?? null,
    target: edge.target,
    target_handle: edge.targetHandle ?? null,
  };
}

// display

export function inferTypeForDisplay(spec: AgentConfigSpec | undefined, value: any): string {
  let ty = spec?.type;
  if (ty !== undefined && ty !== null && ty !== "*") {
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
      tys.add(inferTypeForDisplay({} as AgentConfigSpec, v));
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
