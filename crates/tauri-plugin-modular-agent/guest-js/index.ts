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
  hide_title?: boolean | null;
  description?: string | null;
  hidden?: boolean | null;
  readonly?: boolean | null;
};

export type AgentConfigValueType = string;

export type AgentStreamInfo = {
  id: string;
  name: string;
  running: boolean;
};

// export type AgentStreamSpecs = Record<string, AgentStreamSpec>;

export type AgentStreamSpec = {
  name: string;
  agents: AgentSpec[];
  channels: ChannelSpec[];
  run_on_start?: boolean | null;
  viewport: Viewport | null;
};

export type AgentConfigsMap = Record<string, AgentConfigs>;

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
  /** @deprecated */
  enabled?: boolean | null;
} & AgentSpecExtensions;

export type ChannelSpec = {
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

export async function getAgentStreamInfos(): Promise<AgentStreamInfo[]> {
  return await invoke<any>("plugin:askit|get_agent_stream_infos", {});
}

export async function getAgentStreams(): Promise<AgentStreamSpec[]> {
  return await invoke<any>("plugin:askit|get_agent_streams", {});
}

export async function getRunningAgentStreams(): Promise<string[]> {
  return await invoke<any>("plugin:askit|get_running_agent_streams", {});
}

export async function newAgentStream(streamName: string): Promise<string> {
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

export async function addAgentStream(spec: AgentStreamSpec): Promise<string> {
  return await invoke<any>("plugin:askit|add_agent_stream", { spec });
}

export async function removeAgentStream(id: string): Promise<void> {
  await invoke<void>("plugin:askit|remove_agent_stream", { id });
}

// export async function insertAgentStream(
//   AgentStream: AgentStream
// ): Promise<void> {
//   await invoke<void>("plugin:askit|insert_agent_stream", { AgentStream });
// }

export async function copySubStream(
  agents: AgentSpec[],
  channels: ChannelSpec[]
): Promise<[AgentSpec[], ChannelSpec[]]> {
  return await invoke<any>("plugin:askit|copy_sub_stream", {
    agents,
    channels,
  });
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
): Promise<void> {
  await invoke<void>("plugin:askit|add_agent", {
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
  channelId: string
): Promise<void> {
  await invoke<void>("plugin:askit|remove_channel", {
    streamId,
    channelId,
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
