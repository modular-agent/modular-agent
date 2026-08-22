import { invoke } from "@tauri-apps/api/core";

export type PatchInfo = {
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

export type PatchSpec = {
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

// patch

export async function newPatch(): Promise<[string, string]> {
  return await invoke<any>("plugin:modular-agent|new_patch", {});
}

export async function addPatch(spec: PatchSpec): Promise<string> {
  return await invoke<any>("plugin:modular-agent|add_patch", { spec });
}

export async function addPatchWithName(spec: PatchSpec, name: string): Promise<string> {
  return await invoke<any>("plugin:modular-agent|add_patch_with_name", {
    spec,
    name,
  });
}

export async function removePatch(id: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|remove_patch", { id });
}

export async function startPatch(id: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|start_patch", { id });
}

export async function stopPatch(id: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|stop_patch", { id });
}

export async function openPatchFromFile(path: string, name?: string | null): Promise<string> {
  return await invoke<any>("plugin:modular-agent|open_patch_from_file", {
    path,
    name,
  });
}

export async function savePatch(id: string, path: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|save_patch", { id, path });
}

export async function getPatchSpec(id: string): Promise<PatchSpec | null> {
  return await invoke<any>("plugin:modular-agent|get_patch_spec", { id });
}

export async function updatePatchSpec(id: string, value: Partial<PatchSpec>): Promise<void> {
  await invoke<void>("plugin:modular-agent|update_patch_spec", { id, value });
}

export async function getPatchInfo(id: string): Promise<PatchInfo | null> {
  return await invoke<any>("plugin:modular-agent|get_patch_info", { id });
}

export async function getPatchInfos(): Promise<PatchInfo[]> {
  return await invoke<any>("plugin:modular-agent|get_patch_infos", {});
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

export async function updateAgentSpec(agentId: string, value: Partial<AgentSpec>): Promise<void> {
  await invoke<void>("plugin:modular-agent|update_agent_spec", {
    agentId,
    value,
  });
}

// agents

export async function newAgentSpec(defName: string): Promise<AgentSpec> {
  return await invoke<any>("plugin:modular-agent|new_agent_spec", { defName });
}

export async function addAgent(patchId: string, spec: AgentSpec): Promise<string> {
  return await invoke<string>("plugin:modular-agent|add_agent", {
    patchId,
    spec,
  });
}

export async function removeAgent(patchId: string, agentId: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|remove_agent", {
    patchId,
    agentId,
  });
}

// connection

export async function addConnection(patchId: string, connection: ConnectionSpec): Promise<void> {
  await invoke<void>("plugin:modular-agent|add_connection", {
    patchId,
    connection,
  });
}

export async function removeConnection(patchId: string, connection: ConnectionSpec): Promise<void> {
  await invoke<void>("plugin:modular-agent|remove_connection", {
    patchId,
    connection,
  });
}

export async function addAgentsAndConnections(
  patchId: string,
  agents: AgentSpec[],
  connections: ConnectionSpec[],
): Promise<[AgentSpec[], ConnectionSpec[]]> {
  return await invoke<[AgentSpec[], ConnectionSpec[]]>(
    "plugin:modular-agent|add_agents_and_connections",
    {
      patchId,
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

export async function writeExternalInput(name: string, message: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|write_external_input", {
    name,
    message,
  });
}

// configs

export async function setAgentConfigs(agentId: string, configs: AgentConfigs): Promise<void> {
  await invoke<void>("plugin:modular-agent|set_agent_configs", {
    agentId,
    configs,
  });
}

export async function getGlobalConfigs(defName: string): Promise<AgentConfigs | null> {
  return await invoke<any>("plugin:modular-agent|get_global_configs", {
    defName,
  });
}

export async function getGlobalConfigsMap(): Promise<AgentConfigsMap> {
  return await invoke<any>("plugin:modular-agent|get_global_configs_map", {});
}

export async function setGlobalConfigs(defName: string, configs: AgentConfigs): Promise<void> {
  await invoke<void>("plugin:modular-agent|set_global_configs", {
    defName,
    configs,
  });
}

export async function setGlobalConfigsMap(configs: AgentConfigsMap): Promise<void> {
  await invoke<void>("plugin:modular-agent|set_global_configs_map", {
    configs,
  });
}
