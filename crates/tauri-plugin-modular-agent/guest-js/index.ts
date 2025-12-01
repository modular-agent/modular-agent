import { invoke } from '@tauri-apps/api/core'

export type AgentDefinitions = Record<string, AgentDefinition>;
export type AgentGlobalConfigsMap = Record<string, AgentConfigs>;

export type AgentDefinition = {
  kind: string;
  name: string;
  title: string | null;
  description: string | null;
  category: string | null;
  path: string;
  inputs: string[] | null;
  outputs: string[] | null;
  default_configs: AgentDefaultConfigs | null;
  global_configs: AgentGlobalConfigs | null;
  display_configs: AgentDisplayConfigs | null;
};

export type AgentDefaultConfigs = [string, AgentConfigEntry][];
export type AgentGlobalConfigs = [string, AgentConfigEntry][];

export type AgentConfigEntry = {
  value: any;
  type: AgentConfigValueType | null;
  title?: string | null;
  description?: string | null;
  hidden?: boolean | null;
};

export type AgentConfigValueType =
  | "unit"
  | "boolean"
  | "integer"
  | "number"
  | "string"
  | "password"
  | "text"
  | "object";

export type AgentDisplayConfigs = [string, AgentDisplayConfigEntry][];

export type AgentDisplayConfigEntry = {
  type: AgentDisplayConfigType | null;
  title?: string | null;
  description?: string | null;
  hideTitle?: boolean | null;
};

export type AgentDisplayConfigType =
  | "*"
  | "boolean"
  | "integer"
  | "number"
  | "string"
  | "text"
  | "object"
  | "messages";

export type AgentFlows = Record<string, AgentFlow>;

export type AgentFlow = {
  id: string;
  name: string;
  nodes: AgentFlowNode[];
  edges: AgentFlowEdge[];
  viewport: Viewport | null;
};

export type AgentConfigsMap = Record<string, AgentConfigs>;
export type AgentConfigs = Record<string, any>;

export type AgentFlowNode = {
  id: string;
  def_name: string;
  enabled: boolean;
  configs: AgentConfigs | null;
  title: string | null;
  x: number;
  y: number;
  width?: number;
  height?: number;
};

export type AgentFlowEdge = {
  id: string;
  source: string;
  source_handle: string | null;
  target: string;
  target_handle: string | null;
};

export type Viewport = {
  x: number;
  y: number;
  zoom: number;
};

// settings

export type CoreSettings = {
  autostart: boolean;
  shortcut_keys: Record<string, string>;
};

export type Settings = {
  core: CoreSettings;
  agents: Record<string, AgentDefinition>;
  agent_flows: AgentFlow[];
};

// emit

export type ConfigMessage = {
  agent_id: string;
  key: string;
  value: any;
};

export type DisplayMessage = {
  agent_id: string;
  key: string;
  value: any;
};

export type ErrorMessage = {
  agent_id: string;
  message: string;
};

export type InputMessage = {
  agent_id: string;
  ch: string;
};

// agent definition

export async function getAgentDefinition(): Promise<AgentDefinition | null> {
  return await invoke<any>('plugin:askit|get_agent_definition', {})
}

export async function getAgentDefinitions(): Promise<AgentDefinitions> {
  return await invoke<any>('plugin:askit|get_agent_definitions', {})
}

// flow

export async function getAgentFlows(): Promise<AgentFlows> {
  return await invoke<any>('plugin:askit|get_agent_flows', {})
}

export async function newAgentFlow(flowName: string): Promise<AgentFlow> {
  return await invoke<any>('plugin:askit|new_agent_flow', { flowName })
}

export async function renameAgentFlow(flowId: string, newName: string): Promise<string> {
  return await invoke<any>('plugin:askit|rename_agent_flow', { flowId, newName })
}

export async function uniqueFlowName(name: string): Promise<string> {
  return await invoke<any>('plugin:askit|unique_flow_name', { name })
}

export async function addAgentFlow(agentFlow: AgentFlow): Promise<void> {
  await invoke<void>('plugin:askit|add_agent_flow', { agentFlow })
}

export async function removeAgentFlow(id: string): Promise<void> {
  await invoke<void>('plugin:askit|remove_agent_flow', { id })
}

export async function insertAgentFlow(agentFlow: AgentFlow): Promise<void> {
  await invoke<void>('plugin:askit|insert_agent_flow', { agentFlow })
}

export async function copySubFlow(nodes: AgentFlowNode[], edges: AgentFlowEdge[]): Promise<[AgentFlowNode[], AgentFlowEdge[]]> {
  return await invoke<any>('plugin:askit|copy_sub_flow', { nodes, edges })
}

export async function startAgentFlow(id: string): Promise<void> {
  await invoke<void>('plugin:askit|start_agent_flow', { id })
}

export async function stopAgentFlow(id: string): Promise<void> {
  await invoke<void>('plugin:askit|stop_agent_flow', { id })
}

// nodes

export async function newAgentFlowNode(
  defName: string
): Promise<AgentFlowNode> {
  return await invoke<any>('plugin:askit|new_agent_flow_node', { defName })
}

export async function addAgentFlowNode(
  flowId: string,
  node: AgentFlowNode
): Promise<void> {
  await invoke<void>('plugin:askit|add_agent_flow_node', { flowId, node })
}

export async function removeAgentFlowNode(
  flowId: string,
  nodeId: string
): Promise<void> {
  await invoke<void>('plugin:askit|remove_agent_flow_node', { flowId, nodeId })
}

// edge

export async function addAgentFlowEdge(
  flowId: string,
  edge: AgentFlowEdge
): Promise<void> {
  await invoke<void>('plugin:askit|add_agent_flow_edge', { flowId, edge })
}

export async function removeAgentFlowEdge(
  flowId: string,
  edgeId: string
): Promise<void> {
  await invoke<void>('plugin:askit|remove_agent_flow_edge', { flowId, edgeId })
}

// agent

export async function startAgent(agentId: string): Promise<void> {
  await invoke<void>('plugin:askit|start_agent', { agentId })
}

export async function stopAgent(agentId: string): Promise<void> {
  await invoke<void>('plugin:askit|stop_agent', { agentId })
}

// board

export async function writeBoard(board: string, message: string): Promise<void> {
  await invoke<void>('plugin:askit|write_board', { board, message })
}

// configs

export async function setAgentConfigs(
  agentId: string,
  configs: Record<string, any>
): Promise<void> {
  await invoke<void>('plugin:askit|set_agent_configs', { agentId, configs })
}

export async function getGlobalConfigs(defName: string): Promise<AgentConfigs | null> {
  return await invoke<any>('plugin:askit|get_global_configs', { defName })
}

export async function getGlobalConfigsMap(): Promise<AgentConfigsMap> {
  return await invoke<any>('plugin:askit|get_global_configs_map', {})
}

export async function setGlobalConfigs(defName: string, configs: AgentConfigs): Promise<void> {
  await invoke<void>('plugin:askit|set_global_configs', { defName, configs })
}

export async function setGlobalConfigsMap(configs: AgentConfigsMap): Promise<void> {
  await invoke<void>('plugin:askit|set_global_configs_map', { configs })
}

export async function getAgentDefaultConfigs(defName: string): Promise<AgentDefaultConfigs | null> {
  return await invoke<any>('plugin:askit|get_agent_default_configs', { defName })
}
