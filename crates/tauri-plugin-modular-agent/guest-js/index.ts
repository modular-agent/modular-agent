import { invoke } from '@tauri-apps/api/core'

export type AgentDefinitions = Record<string, AgentDefinition>;
export type AgentGlobalConfigs = Record<string, AgentConfigs>;

export type AgentDefinition = {
  kind: string;
  name: string;
  title: string | null;
  description: string | null;
  category: string | null;
  path: string;
  inputs: string[] | null;
  outputs: string[] | null;
  default_config: AgentDefaultConfig | null;
  global_config: AgentGlobalConfig | null;
  display_config: AgentDisplayConfig | null;
};

export type AgentDefaultConfig = [string, AgentConfigEntry][];
export type AgentGlobalConfig = [string, AgentConfigEntry][];

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

export type AgentDisplayConfig = [string, AgentDisplayConfigEntry][];

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
  nodes: AgentFlowNode[];
  edges: AgentFlowEdge[];
  name: string;
  viewport: Viewport | null;
};

export type AgentConfigs = Record<string, AgentConfig>;
export type AgentConfig = Record<string, any>;

export type AgentFlowNode = {
  id: string;
  def_name: string;
  enabled: boolean;
  config: AgentConfig | null;
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

export type DisplayMessage = {
  agent_id: string;
  key: string;
  data: any;
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

export async function renameAgentFlow(oldName: string, newName: string): Promise<string> {
  return await invoke<any>('plugin:askit|rename_agent_flow', { oldName, newName })
}

export async function uniqueFlowName(name: string): Promise<string> {
  return await invoke<any>('plugin:askit|unique_flow_name', { name })
}

export async function addAgentFlow(agentFlow: AgentFlow): Promise<void> {
  await invoke<void>('plugin:askit|add_agent_flow', { agentFlow })
}

export async function removeAgentFlow(flowName: string): Promise<void> {
  await invoke<void>('plugin:askit|remove_agent_flow', { flowName })
}

export async function insertAgentFlow(agentFlow: AgentFlow): Promise<void> {
  await invoke<void>('plugin:askit|insert_agent_flow', { agentFlow })
}

export async function copySubFlow(nodes: AgentFlowNode[], edges: AgentFlowEdge[]): Promise<[AgentFlowNode[], AgentFlowEdge[]]> {
  return await invoke<any>('plugin:askit|copy_sub_flow', { nodes, edges })
}

export async function startAgentFlow(flowName: string): Promise<void> {
  await invoke<void>('plugin:askit|start_agent_flow', { flowName })
}

export async function stopAgentFlow(flowName: string): Promise<void> {
  await invoke<void>('plugin:askit|stop_agent_flow', { flowName })
}

// nodes

export async function newAgentFlowNode(
  defName: string
): Promise<void> {
  await invoke<void>('plugin:askit|new_agent_flow_node', { defName })
}

export async function addAgentFlowNode(
  flowName: string,
  node: AgentFlowNode
): Promise<void> {
  await invoke<void>('plugin:askit|add_agent_flow_node', { flowName, node })
}

export async function removeAgentFlowNode(
  flowName: string,
  nodeId: string
): Promise<void> {
  await invoke<void>('plugin:askit|remove_agent_flow_node', { flowName, nodeId })
}

// edge

export async function addAgentFlowEdge(
  flowName: string,
  edge: AgentFlowEdge
): Promise<void> {
  await invoke<void>('plugin:askit|add_agent_flow_edge', { flowName, edge })
}

export async function removeAgentFlowEdge(
  flowName: string,
  edgeId: string
): Promise<void> {
  await invoke<void>('plugin:askit|remove_agent_flow_edge', { flowName, edgeId })
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

// config

export async function setAgentConfig(
  agentId: string,
  config: Record<string, any>
): Promise<void> {
  await invoke<void>('plugin:askit|set_agent_config', { agentId, config })
}

export async function getGlobalConfig(defName: string): Promise<AgentConfig | null> {
  return await invoke<any>('plugin:askit|get_global_config', { defName })
}

export async function getGlobalConfigs(): Promise<AgentConfigs> {
  return await invoke<any>('plugin:askit|get_global_configs', {})
}

export async function setGlobalConfig(defName: string, config: AgentConfig): Promise<void> {
  await invoke<void>('plugin:askit|set_global_config', { defName, config })
}

export async function setGlobalConfigs(configs: AgentConfigs): Promise<void> {
  await invoke<void>('plugin:askit|set_global_configs', { configs })
}

export async function getAgentDefaultConfig(defName: string): Promise<AgentDefaultConfig | null> {
  return await invoke<any>('plugin:askit|get_agent_default_config', { defName })
}
