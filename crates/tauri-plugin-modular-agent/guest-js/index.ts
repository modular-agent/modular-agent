import { invoke } from "@tauri-apps/api/core";

export type AgentDefinitions = Record<string, AgentDefinition>;

export type AgentGlobalConfigsMap = Record<string, AgentConfigs>;

export type AgentDefinition = {
  kind: string;
  name: string;
  title?: string | null;
  description?: string | null;
  category?: string | null;
  inputs?: string[] | null;
  outputs?: string[] | null;
  configs?: AgentConfigSpecs | null;
  global_configs?: AgentGlobalConfigSpecs | null;
  native_thread?: boolean | null;
};

export type AgentConfigSpecs = Record<string, AgentConfigSpec>;

export type AgentGlobalConfigSpecs = Record<string, AgentConfigSpec>;

export type AgentConfigSpec = {
  value: any;
  type: AgentConfigValueType | null;
  title?: string | null;
  hideTitle?: boolean | null;
  description?: string | null;
  hidden?: boolean | null;
  readonly?: boolean | null;
};

export type AgentConfigValueType = string;

export type AgentStreams = Record<string, AgentStream>;

export type AgentStream = {
  id: string;
  name: string;
  nodes: AgentStreamNode[];
  edges: AgentStreamEdge[];
  viewport: Viewport | null;
};

export type AgentConfigsMap = Record<string, AgentConfigs>;

export type AgentConfigs = Record<string, any>;

export type AgentSpec = {
  def_name: string;
  inputs?: string[] | null;
  outputs?: string[] | null;
  configs?: AgentConfigs | null;
  config_specs?: AgentConfigSpecs | null;
};

export type AgentStreamNodeExtensions = Record<string, any>;

export type AgentStreamNode = AgentStreamNodeExtensions & {
  id: string;
  enabled: boolean;
  spec: AgentSpec;
};

export type AgentStreamEdge = {
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
  agent_streams: AgentStream[];
};

// emit

export type BoardMessage = {
  key: string;
  value: any;
};

// agent definition

export async function getAgentDefinition(): Promise<AgentDefinition | null> {
  return await invoke<any>("plugin:askit|get_agent_definition", {});
}

export async function getAgentDefinitions(): Promise<AgentDefinitions> {
  return await invoke<any>("plugin:askit|get_agent_definitions", {});
}

// agent spec

export async function getAgentSpec(agentId: string): Promise<AgentSpec | null> {
  return await invoke<any>("plugin:askit|get_agent_spec", { agentId });
}

// stream

export async function getAgentStreams(): Promise<AgentStreams> {
  return await invoke<any>("plugin:askit|get_agent_streams", {});
}

export async function newAgentStream(streamName: string): Promise<AgentStream> {
  return await invoke<any>("plugin:askit|new_agent_stream", { streamName });
}

export async function renameAgentStream(
  streamId: string,
  newName: string
): Promise<string> {
  return await invoke<any>("plugin:askit|rename_agent_stream", {
    streamId,
    newName,
  });
}

export async function uniqueStreamName(name: string): Promise<string> {
  return await invoke<any>("plugin:askit|unique_stream_name", { name });
}

export async function addAgentStream(AgentStream: AgentStream): Promise<void> {
  await invoke<void>("plugin:askit|add_agent_stream", { AgentStream });
}

export async function removeAgentStream(id: string): Promise<void> {
  await invoke<void>("plugin:askit|remove_agent_stream", { id });
}

export async function insertAgentStream(
  AgentStream: AgentStream
): Promise<void> {
  await invoke<void>("plugin:askit|insert_agent_stream", { AgentStream });
}

export async function copySubStream(
  nodes: AgentStreamNode[],
  edges: AgentStreamEdge[]
): Promise<[AgentStreamNode[], AgentStreamEdge[]]> {
  return await invoke<any>("plugin:askit|copy_sub_stream", { nodes, edges });
}

export async function startAgentStream(id: string): Promise<void> {
  await invoke<void>("plugin:askit|start_agent_stream", { id });
}

export async function stopAgentStream(id: string): Promise<void> {
  await invoke<void>("plugin:askit|stop_agent_stream", { id });
}

// nodes

export async function newAgentStreamNode(
  defName: string
): Promise<AgentStreamNode> {
  return await invoke<any>("plugin:askit|new_agent_stream_node", { defName });
}

export async function addAgentStreamNode(
  streamId: string,
  node: AgentStreamNode
): Promise<void> {
  await invoke<void>("plugin:askit|add_agent_stream_node", { streamId, node });
}

export async function removeAgentStreamNode(
  streamId: string,
  nodeId: string
): Promise<void> {
  await invoke<void>("plugin:askit|remove_agent_stream_node", {
    streamId,
    nodeId,
  });
}

// edge

export async function addAgentStreamEdge(
  streamId: string,
  edge: AgentStreamEdge
): Promise<void> {
  await invoke<void>("plugin:askit|add_agent_stream_edge", { streamId, edge });
}

export async function removeAgentStreamEdge(
  streamId: string,
  edgeId: string
): Promise<void> {
  await invoke<void>("plugin:askit|remove_agent_stream_edge", {
    streamId,
    edgeId,
  });
}

// agent

export async function startAgent(agentId: string): Promise<void> {
  await invoke<void>("plugin:askit|start_agent", { agentId });
}

export async function stopAgent(agentId: string): Promise<void> {
  await invoke<void>("plugin:askit|stop_agent", { agentId });
}

// board

export async function writeBoard(
  board: string,
  message: string
): Promise<void> {
  await invoke<void>("plugin:askit|write_board", { board, message });
}

// configs

export async function setAgentConfigs(
  agentId: string,
  configs: AgentConfigs
): Promise<void> {
  await invoke<void>("plugin:askit|set_agent_configs", { agentId, configs });
}

export async function getGlobalConfigs(
  defName: string
): Promise<AgentConfigs | null> {
  return await invoke<any>("plugin:askit|get_global_configs", { defName });
}

export async function getGlobalConfigsMap(): Promise<AgentConfigsMap> {
  return await invoke<any>("plugin:askit|get_global_configs_map", {});
}

export async function setGlobalConfigs(
  defName: string,
  configs: AgentConfigs
): Promise<void> {
  await invoke<void>("plugin:askit|set_global_configs", { defName, configs });
}

export async function setGlobalConfigsMap(
  configs: AgentConfigsMap
): Promise<void> {
  await invoke<void>("plugin:askit|set_global_configs_map", { configs });
}

export async function getAgentConfigSpecs(
  defName: string
): Promise<AgentConfigSpecs | null> {
  return await invoke<any>("plugin:askit|get_agent_config_specs", {
    defName,
  });
}
