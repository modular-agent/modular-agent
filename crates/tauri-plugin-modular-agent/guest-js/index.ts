import { invoke } from "@tauri-apps/api/core";

export type PatchInfo = {
  id: string;
  name: string;
  running: boolean;
};

export type ModuleStatus = "init" | "start" | "stop";

export type ModuleDefinitions = Record<string, ModuleDefinition>;

export type ModuleDefinition = {
  kind: string;
  name: string;
  title?: string | null;
  hide_title?: boolean | null;
  description?: string | null;
  category?: string | null;
  inputs?: string[] | null;
  outputs?: string[] | null;
  configs?: ModuleConfigSpecs | null;
  global_configs?: ModuleGlobalConfigs | null;
  hints?: Record<string, any>;
};

export type ModuleConfigSpecs = Record<string, ModuleConfigSpec>;

export type ModuleGlobalConfigs = Record<string, ModuleConfigSpec>;

export type ModuleConfigSpec = {
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
  modules: ModuleSpec[];
  connections: ConnectionSpec[];
  viewport: Viewport | null;
};

export type ModuleConfigsMap = Record<string, ModuleConfigs>;

export type ModuleGlobalConfigsMap = Record<string, ModuleConfigs>;

export type ModuleConfigs = Record<string, any>;

export type ModuleSpecExtensions = Record<string, any>;

export type ModuleSpec = {
  id?: string | null;
  def_name: string;
  inputs?: string[] | null;
  outputs?: string[] | null;
  configs?: ModuleConfigs | null;
  config_specs?: ModuleConfigSpecs | null;
  disabled?: boolean | null;
} & ModuleSpecExtensions;

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

// module

export async function getModuleDefinition(defName: string): Promise<ModuleDefinition | null> {
  return await invoke<any>("plugin:modular-agent|get_module_definition", { defName });
}

export async function getModuleDefinitions(): Promise<ModuleDefinitions> {
  return await invoke<any>("plugin:modular-agent|get_module_definitions", {});
}

// module spec

export async function getModuleSpec(moduleId: string): Promise<ModuleSpec | null> {
  return await invoke<any>("plugin:modular-agent|get_module_spec", { moduleId });
}

export async function updateModuleSpec(
  moduleId: string,
  value: Partial<ModuleSpec>,
): Promise<void> {
  await invoke<void>("plugin:modular-agent|update_module_spec", {
    moduleId,
    value,
  });
}

// modules

export async function newModuleSpec(defName: string): Promise<ModuleSpec> {
  return await invoke<any>("plugin:modular-agent|new_module_spec", { defName });
}

export async function addModule(patchId: string, spec: ModuleSpec): Promise<string> {
  return await invoke<string>("plugin:modular-agent|add_module", {
    patchId,
    spec,
  });
}

export async function removeModule(patchId: string, moduleId: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|remove_module", {
    patchId,
    moduleId,
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

export async function addModulesAndConnections(
  patchId: string,
  modules: ModuleSpec[],
  connections: ConnectionSpec[],
): Promise<[ModuleSpec[], ConnectionSpec[]]> {
  return await invoke<[ModuleSpec[], ConnectionSpec[]]>(
    "plugin:modular-agent|add_modules_and_connections",
    {
      patchId,
      modules,
      connections,
    },
  );
}

// module

export async function startModule(moduleId: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|start_module", { moduleId });
}

export async function stopModule(moduleId: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|stop_module", { moduleId });
}

export async function getModuleStatuses(patchId: string): Promise<Record<string, ModuleStatus>> {
  return await invoke<any>("plugin:modular-agent|get_module_statuses", { patchId });
}

// external input

export async function writeExternalInput(name: string, message: string): Promise<void> {
  await invoke<void>("plugin:modular-agent|write_external_input", {
    name,
    message,
  });
}

// configs

export async function setModuleConfigs(moduleId: string, configs: ModuleConfigs): Promise<void> {
  await invoke<void>("plugin:modular-agent|set_module_configs", {
    moduleId,
    configs,
  });
}

export async function getGlobalConfigs(defName: string): Promise<ModuleConfigs | null> {
  return await invoke<any>("plugin:modular-agent|get_global_configs", {
    defName,
  });
}

export async function getGlobalConfigsMap(): Promise<ModuleConfigsMap> {
  return await invoke<any>("plugin:modular-agent|get_global_configs_map", {});
}

export async function setGlobalConfigs(defName: string, configs: ModuleConfigs): Promise<void> {
  await invoke<void>("plugin:modular-agent|set_global_configs", {
    defName,
    configs,
  });
}

export async function setGlobalConfigsMap(configs: ModuleConfigsMap): Promise<void> {
  await invoke<void>("plugin:modular-agent|set_global_configs_map", {
    configs,
  });
}
