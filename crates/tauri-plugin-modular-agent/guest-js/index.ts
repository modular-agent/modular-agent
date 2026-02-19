import { invoke } from "@tauri-apps/api/core";

export type PresetInfo = {
  id: string;
  name: string;
  running: boolean;
};

export type AgentDefinitions = Record<string, AgentDefinition>;

export type AgentDefinition = {
  kind: string;
  name: string;
  title?: string | null;
  hide_title?: boolean | null;
  description?: string | null;
  category?: string | null;
  inputs?: string[] | null;
  outputs?: string[] | null;
  configs?: AgentConfigSpecs | null;
  global_configs?: AgentGlobalConfigs | null;
  hints?: Record<string, any>;
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
  detail?: boolean | null;
};

export type PresetSpec = {
  agents: AgentSpec[];
  connections: ConnectionSpec[];
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

export type ConnectionSpec = {
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

// preset

export async function newPreset(): Promise<[string, string]> {
  return await invoke<any>("plugin:modular-agent|new_preset", {});
}

export async function addPreset(spec: PresetSpec): Promise<string> {
  return await invoke<any>("plugin:modular-agent|add_preset", { spec });
}

export async function addPresetWithName(
  spec: PresetSpec,
  name: string,
): Promise<string> {
  return await invoke<any>("plugin:modular-agent|add_preset_with_name", {
    spec,
    name,
  });
}

export async function removePreset(id: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|remove_preset", { id });
}

export async function startPreset(id: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|start_preset", { id });
}

export async function stopPreset(id: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|stop_preset", { id });
}

export async function openPresetFromFile(
  path: string,
  name?: string | null,
): Promise<string> {
  return await invoke<any>("plugin:modular-agent|open_preset_from_file", {
    path,
    name,
  });
}

export async function savePreset(id: string, path: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|save_preset", { id, path });
}

export async function getPresetSpec(id: string): Promise<PresetSpec | null> {
  return await invoke<any>("plugin:modular-agent|get_preset_spec", { id });
}

export async function updatePresetSpec(
  id: string,
  value: Partial<PresetSpec>,
): Promise<void> {
  await invoke<void>("plugin:modular-agent|update_preset_spec", { id, value });
}

export async function getPresetInfo(id: string): Promise<PresetInfo | null> {
  return await invoke<any>("plugin:modular-agent|get_preset_info", { id });
}

export async function getPresetInfos(): Promise<PresetInfo[]> {
  return await invoke<any>("plugin:modular-agent|get_preset_infos", {});
}

// agent

export async function getAgentDefinition(): Promise<AgentDefinition | null> {
  return await invoke<any>("plugin:modular-agent|get_agent_definition", {});
}

export async function getAgentDefinitions(): Promise<AgentDefinitions> {
  return await invoke<any>("plugin:modular-agent|get_agent_definitions", {});
}

// agent spec

export async function getAgentSpec(agentId: string): Promise<AgentSpec | null> {
  return await invoke<any>("plugin:modular-agent|get_agent_spec", { agentId });
}

export async function updateAgentSpec(
  agentId: string,
  value: Partial<AgentSpec>,
): Promise<void> {
  await invoke<void>("plugin:modular-agent|update_agent_spec", {
    agentId,
    value,
  });
}

// agents

export async function newAgentSpec(defName: string): Promise<AgentSpec> {
  return await invoke<any>("plugin:modular-agent|new_agent_spec", { defName });
}

export async function addAgent(
  presetId: string,
  spec: AgentSpec,
): Promise<string> {
  return await invoke<string>("plugin:modular-agent|add_agent", {
    presetId,
    spec,
  });
}

export async function removeAgent(
  presetId: string,
  agentId: string,
): Promise<void> {
  await invoke<void>("plugin:modular-agent|remove_agent", {
    presetId,
    agentId,
  });
}

// connection

export async function addConnection(
  presetId: string,
  connection: ConnectionSpec,
): Promise<void> {
  await invoke<void>("plugin:modular-agent|add_connection", {
    presetId,
    connection,
  });
}

export async function removeConnection(
  presetId: string,
  connection: ConnectionSpec,
): Promise<void> {
  await invoke<void>("plugin:modular-agent|remove_connection", {
    presetId,
    connection,
  });
}

export async function addAgentsAndConnections(
  presetId: string,
  agents: AgentSpec[],
  connections: ConnectionSpec[],
): Promise<[AgentSpec[], ConnectionSpec[]]> {
  return await invoke<[AgentSpec[], ConnectionSpec[]]>(
    "plugin:modular-agent|add_agents_and_connections",
    {
      presetId,
      agents,
      connections,
    },
  );
}

// agent

export async function startAgent(agentId: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|start_agent", { agentId });
}

export async function stopAgent(agentId: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|stop_agent", { agentId });
}

// external input

export async function writeExternalInput(
  name: string,
  message: string,
): Promise<void> {
  await invoke<void>("plugin:modular-agent|write_external_input", {
    name,
    message,
  });
}

// configs

export async function setAgentConfigs(
  agentId: string,
  configs: AgentConfigs,
): Promise<void> {
  await invoke<void>("plugin:modular-agent|set_agent_configs", {
    agentId,
    configs,
  });
}

export async function getGlobalConfigs(
  defName: string,
): Promise<AgentConfigs | null> {
  return await invoke<any>("plugin:modular-agent|get_global_configs", {
    defName,
  });
}

export async function getGlobalConfigsMap(): Promise<AgentConfigsMap> {
  return await invoke<any>("plugin:modular-agent|get_global_configs_map", {});
}

export async function setGlobalConfigs(
  defName: string,
  configs: AgentConfigs,
): Promise<void> {
  await invoke<void>("plugin:modular-agent|set_global_configs", {
    defName,
    configs,
  });
}

export async function setGlobalConfigsMap(
  configs: AgentConfigsMap,
): Promise<void> {
  await invoke<void>("plugin:modular-agent|set_global_configs_map", {
    configs,
  });
}
