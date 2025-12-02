import { invoke } from "@tauri-apps/api/core";

import { getContext, setContext } from "svelte";

import type {
  AgentConfigs,
  AgentDefaultConfigs,
  AgentDefinitions,
  AgentDisplayConfigs,
  AgentDisplayConfigEntry,
  AgentFlow,
  AgentFlowEdge,
  AgentFlowNode,
  Viewport,
} from "tauri-plugin-askit-api";

import type {
  TAgentFlow,
  TAgentFlowEdge,
  TAgentFlowNode,
  TAgentFlowNodeConfigs,
  TAgentFlowNodeDisplays,
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
  agent_settings: AgentDefinitions,
): TAgentFlow {
  // Deserialize nodes first
  const nodes = flow.nodes.map((node) => deserializeAgentFlowNode(node, agent_settings));

  // Create a map to retrieve available handles from node IDs
  const nodeHandles = new Map<string, { inputs: string[]; outputs: string[]; configs: string[] }>();

  nodes.forEach((node) => {
    const def = agent_settings[node.data.name];
    if (def) {
      nodeHandles.set(node.id, {
        inputs: def.inputs || [],
        outputs: def.outputs || [],
        configs: (def.default_configs || [])
          .filter(([_, entry]) => entry.hidden !== true)
          .map(([key, _]) => key),
      });
    }
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
  agentDefs: AgentDefinitions,
): TAgentFlowNode {
  const agentDef = agentDefs[node.def_name];
  const default_configs = agentDef?.default_configs;
  const display_configs = agentDef?.display_configs;
  return {
    id: node.id,
    type: "agent",
    data: {
      name: node.def_name,
      enabled: agentDef !== undefined && node.enabled,
      title: node.title,
      configs: deserializeAgentConfigs(node.configs, default_configs),
      displays: deserializeAgentDisplayConfigs(display_configs),
    },
    position: {
      x: node.x,
      y: node.y,
    },
    width: node.width,
    height: node.height,
  };
}

export function deserializeAgentConfigs(
  node_configs: AgentConfigs | null,
  default_configs: AgentDefaultConfigs | null,
): TAgentFlowNodeConfigs {
  let agent_configs: TAgentFlowNodeConfigs = {};
  let config_types: Record<string, string | null> = {};

  if (default_configs) {
    default_configs.forEach(([key, entry]) => {
      agent_configs[key] = entry.value;
      config_types[key] = entry.type;
    });
  }

  if (node_configs) {
    for (const [key, value] of Object.entries(node_configs)) {
      agent_configs[key] = value;
    }
  }

  for (const [key, value] of Object.entries(agent_configs)) {
    const t = config_types[key];
    if (t === null) {
      continue;
    } else if (t === "boolean") {
      agent_configs[key] = value;
    } else if (t === "integer") {
      agent_configs[key] = value.toString();
    } else if (t === "number") {
      agent_configs[key] = value.toString();
    } else if (t === "string") {
      agent_configs[key] = value;
    } else if (t === "text") {
      agent_configs[key] = value;
    } else if (t === "object") {
      agent_configs[key] = JSON.stringify(value, null, 2);
    }
  }

  return agent_configs;
}

export function deserializeAgentDisplayConfigs(
  display_configs: AgentDisplayConfigs | null,
): TAgentFlowNodeDisplays | null {
  if (!display_configs) {
    return null;
  }
  let display: TAgentFlowNodeDisplays = {};
  display_configs.forEach(([key, _entry]) => {
    display[key] = null;
  });
  return display;
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
  agent_defs: AgentDefinitions,
  viewport: Viewport,
): AgentFlow {
  return {
    id,
    name,
    nodes: nodes.map((node) => serializeAgentFlowNode(node, agent_defs)),
    edges: edges.map((edge) => serializeAgentFlowEdge(edge)),
    viewport,
  };
}

export function serializeAgentFlowNode(
  node: TAgentFlowNode,
  agent_defs: AgentDefinitions,
): AgentFlowNode {
  return {
    id: node.id,
    def_name: node.data.name,
    enabled: node.data.enabled,
    configs: serializeAgentFlowNodeConfigs(
      node.data.configs,
      agent_defs[node.data.name]?.default_configs,
    ),
    title: node.data.title,
    x: node.position.x,
    y: node.position.y,
    width: node.width,
    height: node.height,
  };
}

export function serializeAgentFlowNodeConfigs(
  node_configs: TAgentFlowNodeConfigs | null,
  default_configs: AgentDefaultConfigs | null,
): AgentConfigs | null {
  if (node_configs === null) {
    return null;
  }

  let configs: AgentConfigs = {};

  if (default_configs === null || default_configs === undefined) {
    // if no default config, just return the node_config as is
    for (const [key, value] of Object.entries(node_configs)) {
      configs[key] = value;
    }
    return configs;
  }

  default_configs.forEach(([key, entry]) => {
    const t = entry.type;
    const value = node_configs[key];
    if (t === "boolean") {
      configs[key] = value;
    } else if (t === "integer") {
      configs[key] = parseInt(value);
    } else if (t === "number") {
      configs[key] = parseFloat(value);
    } else if (t === "string") {
      configs[key] = value;
    } else if (t === "text") {
      configs[key] = value;
    } else if (t === "object") {
      configs[key] = JSON.parse(value);
    } else {
      configs[key] = value;
    }
  });

  return configs;
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

export function inferTypeForDisplay(config: AgentDisplayConfigEntry, value: any): string {
  let ty = config?.type;
  if (ty === undefined || ty === null || ty === "*") {
    if (value === null || value === undefined) {
      return "object";
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
        tys.add(inferTypeForDisplay({} as AgentDisplayConfigEntry, v));
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
  return ty;
}
