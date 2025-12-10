import { invoke } from "@tauri-apps/api/core";

import { getContext, setContext } from "svelte";

import type {
  AgentConfigSpecs,
  AgentConfigs,
  AgentDefinitions,
  AgentDisplayConfigSpec,
  AgentDisplayConfigSpecs,
  AgentFlow,
  AgentFlowEdge,
  AgentFlowNode,
  Viewport,
} from "tauri-plugin-askit-api";

import type {
  TAgentFlow,
  TAgentFlowEdge,
  TAgentFlowNode,
  // TAgentFlowNodeConfigs,
  // TAgentFlowNodeDisplays,
} from "./types";

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

export function deserializeAgentFlow(
  flow: AgentFlow,
  // agent_settings: AgentDefinitions,
): TAgentFlow {
  // Deserialize nodes first
  const nodes = flow.nodes.map((node) => deserializeAgentFlowNode(node));

  // Create a map to retrieve available handles from node IDs
  const nodeHandles = new Map<string, { inputs: string[]; outputs: string[]; configs: string[] }>();

  nodes.forEach((node) => {
    // const def = agentDefs[node.data.name];
    // const inputs = def?.inputs ?? node.spec.inputs ?? [];
    // const outputs = def?.outputs ?? node.spec.outputs ?? [];
    // const configs =
    //   def?.default_configs?.filter(([_, entry]) => entry.hidden !== true).map(([key, _]) => key) ??
    //   Object.keys(node.spec.configs ?? {});
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

export function deserializeAgentFlowNode(
  node: AgentFlowNode,
  // agentDefs: AgentDefinitions,
): TAgentFlowNode {
  const { id, enabled, spec, ...rest } = node as AgentFlowNode & Record<string, any>;

  // const defName = spec?.def_name ?? rest.def_name;
  // const agentDef = defName ? agentDefs[defName] : undefined;

  // const spec: AgentSpec = incomingSpec ?? {
  //   def_name: defName ?? "",
  //   inputs: agentDef?.inputs ?? [],
  //   outputs: agentDef?.outputs ?? [],
  //   configs: (rest.configs as AgentConfigs) ?? null,
  //   display_configs: agentDef?.display_configs ?? null,
  // };

  const { title = null, x = 0, y = 0, width, height, ...extensions } = rest as Record<string, any>;
  // const default_configs = agentDef?.default_configs ?? null;
  // const display_configs = spec.display_configs ?? agentDef?.display_configs ?? null;
  return {
    id,
    type: "agent",
    data: {
      name: spec.def_name,
      // enabled: agentDef !== undefined && enabled,
      enabled,
      title,
      spec,
      display_values: spec.display_config_specs ? {} : null,
      // configs: deserializeAgentConfigs(spec.configs ?? null, default_configs),
      // displays: deserializeAgentDisplayConfigs(display_configs),
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

// export function deserializeAgentConfigs(
//   node_configs: AgentConfigs | null,
//   default_configs: AgentDefaultConfigs | null,
// ): TAgentFlowNodeConfigs {
//   let agent_configs: TAgentFlowNodeConfigs = {};
//   let config_types: Record<string, string | null> = {};

//   if (default_configs) {
//     for (const [key, entry] of Object.entries(default_configs)) {
//       agent_configs[key] = entry.value;
//       config_types[key] = entry.type;
//     }
//   }

//   if (node_configs) {
//     for (const [key, value] of Object.entries(node_configs)) {
//       agent_configs[key] = value;
//     }
//   }

//   for (const [key, value] of Object.entries(agent_configs)) {
//     const t = config_types[key];
//     if (t === null) {
//       continue;
//     } else if (t === "boolean") {
//       agent_configs[key] = value;
//     } else if (t === "integer") {
//       agent_configs[key] = value.toString();
//     } else if (t === "number") {
//       agent_configs[key] = value.toString();
//     } else if (t === "string") {
//       agent_configs[key] = value;
//     } else if (t === "text") {
//       agent_configs[key] = value;
//     } else if (t === "object") {
//       agent_configs[key] = JSON.stringify(value, null, 2);
//     }
//   }

//   return agent_configs;
// }

// export function deserializeAgentDisplayConfigs(
//   display_configs: AgentDisplayConfigs | null,
// ): TAgentFlowNodeDisplays | null {
//   if (!display_configs) {
//     return null;
//   }
//   let display: TAgentFlowNodeDisplays = {};
//   Object.entries(display_configs).forEach(([key, _entry]) => {
//     display[key] = null;
//   });
//   return display;
// }

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
  // const agentDef = agent_defs[node.data.name];
  // const inputs = node.spec?.inputs ?? agentDef?.inputs ?? [];
  // const outputs = node.spec?.outputs ?? agentDef?.outputs ?? [];
  // const displayConfigs = node.spec?.display_configs ?? agentDef?.display_configs ?? null;
  // const spec: AgentSpec = {
  //   def_name: node.data.name,
  //   inputs: inputs ?? [],
  //   outputs: outputs ?? [],
  //   configs: serializeAgentFlowNodeConfigs(node.data.configs, agentDef?.default_configs ?? null),
  //   display_configs: displayConfigs ?? null,
  // };

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

// export function serializeAgentFlowNodeConfigs(
//   node_configs: TAgentFlowNodeConfigs | null,
//   default_configs: AgentDefaultConfigs | null,
// ): AgentConfigs | null {
//   if (node_configs === null) {
//     return null;
//   }

//   let configs: AgentConfigs = {};

//   if (default_configs === null || default_configs === undefined) {
//     // if no default config, just return the node_config as is
//     for (const [key, value] of Object.entries(node_configs)) {
//       configs[key] = value;
//     }
//     return configs;
//   }

//   Object.entries(default_configs).forEach(([key, entry]) => {
//     const t = entry.type;
//     const value = node_configs[key];
//     if (t === "boolean") {
//       configs[key] = value;
//     } else if (t === "integer") {
//       configs[key] = parseInt(value);
//     } else if (t === "number") {
//       configs[key] = parseFloat(value);
//     } else if (t === "string") {
//       configs[key] = value;
//     } else if (t === "text") {
//       configs[key] = value;
//     } else if (t === "object") {
//       configs[key] = JSON.parse(value);
//     } else {
//       configs[key] = value;
//     }
//   });

//   return configs;
// }

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
