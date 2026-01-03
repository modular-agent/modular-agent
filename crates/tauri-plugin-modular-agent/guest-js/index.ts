import { invoke } from "@tauri-apps/api/core";

export type AgentStreamInfo = {
  id: string;
  name: string;
  running: boolean;
  run_on_start: boolean;
};

export type AgentDefinitions = Record<string, AgentDefinition>;

export type AgentDefinition = {
  kind: string;
  name: string;
  title?: string | null;
  description?: string | null;
  category?: string | null;
  inputs?: string[] | null;
  outputs?: string[] | null;
  configs?: AgentConfigSpecs | null;
  global_configs?: AgentGlobalConfigs | null;
  native_thread?: boolean | null;
};

export type AgentConfigSpecs = Record<string, AgentConfigSpec>;

export type AgentGlobalConfigs = Record<string, AgentConfigSpec>;

export type AgentConfigSpec = {
  value: any;
  type: string | null;
  title?: string | null;
  hide_title?: boolean | null;
  description?: string | null;
  hidden?: boolean | null;
  readonly?: boolean | null;
};

export type AgentStreamSpec = {
  agents: AgentSpec[];
  channels: ChannelSpec[];
  run_on_start?: boolean | null;
  viewport: Viewport | null;
};

export type AgentConfigsMap = Record<string, AgentConfigs>;

export type AgentGlobalConfigsMap = Record<string, AgentConfigs>;

export type AgentConfigs = Record<string, any>;

export type AgentSpecExtensions = Record<string, any>;

export type AgentSpec = {
  id?: string | null;
  def_name: string;
  inputs?: string[] | null;
  outputs?: string[] | null;
  configs?: AgentConfigs | null;
  config_specs?: AgentConfigSpecs | null;
  disabled?: boolean | null;
} & AgentSpecExtensions;

export type ChannelSpec = {
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

export async function getAgentStreamInfo(
  id: string
): Promise<AgentStreamInfo | null> {
  return await invoke<any>("plugin:askit|get_agent_stream_info", { id });
}

export async function getAgentStreamInfos(): Promise<AgentStreamInfo[]> {
  return await invoke<any>("plugin:askit|get_agent_stream_infos", {});
}

export async function getAgentStreamSpec(
  id: string
): Promise<AgentStreamSpec | null> {
  return await invoke<any>("plugin:askit|get_agent_stream_spec", { id });
}

export async function setAgentStreamSpec(
  id: string,
  spec: AgentStreamSpec
): Promise<void> {
  await invoke<void>("plugin:askit|set_agent_stream_spec", { id, spec });
}

export async function newAgentStream(name: string): Promise<[string, string]> {
  return await invoke<any>("plugin:askit|new_agent_stream", { name });
}

export async function renameAgentStream(
  id: string,
  name: string
): Promise<string> {
  return await invoke<any>("plugin:askit|rename_agent_stream", {
    id,
    name,
  });
}

export async function uniqueStreamName(name: string): Promise<string> {
  return await invoke<any>("plugin:askit|unique_stream_name", { name });
}

export async function addAgentStream(
  name: string,
  spec: AgentStreamSpec
): Promise<string> {
  return await invoke<any>("plugin:askit|add_agent_stream", { name, spec });
}

export async function removeAgentStream(id: string): Promise<void> {
  await invoke<void>("plugin:askit|remove_agent_stream", { id });
}

export async function addAgentsAndChannels(
  streamId: string,
  agents: AgentSpec[],
  channels: ChannelSpec[]
): Promise<[AgentSpec[], ChannelSpec[]]> {
  return await invoke<[AgentSpec[], ChannelSpec[]]>(
    "plugin:askit|add_agents_and_channels",
    {
      streamId,
      agents,
      channels,
    }
  );
}

export async function startAgentStream(id: string): Promise<void> {
  await invoke<void>("plugin:askit|start_agent_stream", { id });
}

export async function stopAgentStream(id: string): Promise<void> {
  await invoke<void>("plugin:askit|stop_agent_stream", { id });
}

// agents

export async function newAgentSpec(defName: string): Promise<AgentSpec> {
  return await invoke<any>("plugin:askit|new_agent_spec", { defName });
}

export async function addAgent(
  streamId: string,
  spec: AgentSpec
): Promise<string> {
  return await invoke<string>("plugin:askit|add_agent", {
    streamId,
    spec,
  });
}

export async function removeAgent(
  streamId: string,
  agentId: string
): Promise<void> {
  await invoke<void>("plugin:askit|remove_agent", {
    streamId,
    agentId,
  });
}

// channel

export async function addChannel(
  streamId: string,
  channel: ChannelSpec
): Promise<void> {
  await invoke<void>("plugin:askit|add_channel", {
    streamId,
    channel,
  });
}

export async function removeChannel(
  streamId: string,
  channel: ChannelSpec
): Promise<void> {
  await invoke<void>("plugin:askit|remove_channel", {
    streamId,
    channel,
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
