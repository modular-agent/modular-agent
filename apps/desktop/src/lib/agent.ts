import { invoke } from "@tauri-apps/api/core";

import {
  type AgentConfigSpec,
  type AgentConfigsMap,
  type AgentDefinitions,
  type AgentSpec,
  type PresetInfo,
  type PresetSpec,
  type ConnectionSpec,
  getAgentDefinitions as getAgentDefinitionsAPI,
  getGlobalConfigsMap as getGlobalConfigsMapAPI,
  getPresetInfos,
} from "tauri-plugin-modular-agent-api";

import {
  getCoreSettings as getCoreSettingsUtils,
  setCoreSettings as setCoreSettingsUtils,
} from "./modular_agent";
import type { PresetFlow, PresetEdge, PresetNode, CoreSettings, PresetInfoExt } from "./types";

export async function newPresetWithName(name: string): Promise<string> {
  return await invoke("new_preset_with_name_cmd", { name });
}

export async function renamePreset(id: string, name: string): Promise<string> {
  return await invoke("rename_preset_cmd", { id, name });
}

export async function removePreset(id: string): Promise<void> {
  await invoke("remove_preset_cmd", { id });
}

export async function savePreset(name: string, spec: PresetSpec): Promise<void> {
  await invoke("save_preset_cmd", { name, spec });
}

export async function importPreset(path: string, targetDir: string): Promise<string> {
  return await invoke("import_preset_cmd", { path, targetDir });
}

export async function startPreset(id: string): Promise<void> {
  await invoke("start_preset_cmd", { id });
}

export async function stopPreset(id: string): Promise<void> {
  await invoke("stop_preset_cmd", { id });
}

// Preset

export function presetToFlow(info: PresetInfo, spec: PresetSpec): PresetFlow {
  // Deserialize agents first
  const nodes = spec.agents.map((agent) => agentSpecToNode(agent));

  // Create a map to retrieve available handles from node IDs
  const nodeHandles = new Map<string, { inputs: string[]; outputs: string[]; configs: string[] }>();

  nodes.forEach((node) => {
    const inputs = node.data.inputs ?? [];
    const outputs = node.data.outputs ?? [];
    const configs = Object.keys(node.data.configs ?? {});

    nodeHandles.set(node.id, { inputs, outputs, configs });
  });

  // Filter only valid connections
  const validConnections = spec.connections.filter((conn) => {
    const sourceNode = nodeHandles.get(conn.source);
    const targetNode = nodeHandles.get(conn.target);

    if (!sourceNode || !targetNode) return false;

    // Ensure that the source and target handles actually exist
    const isSourceValid =
      conn.source_handle === "err" || sourceNode.outputs.includes(conn.source_handle ?? "");
    const isTargetValid = conn.target_handle?.startsWith("config:")
      ? targetNode.configs.includes((conn.target_handle ?? "").substring(7))
      : targetNode.inputs.includes(conn.target_handle ?? "");

    return isSourceValid && isTargetValid;
  });

  return {
    id: info.id,
    name: info.name,
    nodes: nodes,
    edges: validConnections.map((conn) => connectionSpecToEdge(conn)),
    running: info.running,
    viewport: spec.viewport,
  };
}

export function agentSpecToNode(spec: AgentSpec): PresetNode {
  return {
    id: spec.id ?? crypto.randomUUID(),
    type: "agent",
    data: { ...spec },
    position: {
      x: spec.x ?? 0,
      y: spec.y ?? 0,
    },
    width: spec.width,
    height: spec.height,
  };
}

// Connection color mapping by source handle name (type-aware ports only)
const EDGE_COLOR_MAP: Record<string, string> = {
  // unit
  unit: "var(--color-connection-unit)",
  // boolean
  boolean: "var(--color-connection-boolean)",
  // number
  integer: "var(--color-connection-number)",
  number: "var(--color-connection-number)",
  // string-like
  string: "var(--color-connection-string)",
  text: "var(--color-connection-string)",
  // object-like
  doc: "var(--color-connection-string)",
  object: "var(--color-connection-object)",
  message: "var(--color-connection-object)",
  // image
  image: "var(--color-connection-image)",
  // error
  err: "var(--color-connection-err)",
};

export function getEdgeColor(sourceHandle: string | null | undefined): string | null {
  if (!sourceHandle) return null;
  const color = EDGE_COLOR_MAP[sourceHandle];
  if (color) return color;
  // Plural fallback: "messages" -> "message", "strings" -> "string"
  if (sourceHandle.endsWith("s")) {
    return EDGE_COLOR_MAP[sourceHandle.slice(0, -1)] ?? null;
  }
  return null;
}

export function connectionSpecToEdge(connection: ConnectionSpec): PresetEdge {
  const color = getEdgeColor(connection.source_handle);
  return {
    id: crypto.randomUUID(),
    source: connection.source,
    sourceHandle: connection.source_handle,
    target: connection.target,
    targetHandle: connection.target_handle,
    ...(color ? { style: `stroke: ${color};` } : {}),
  };
}

export function edgeToConnectionSpec(edge: PresetEdge): ConnectionSpec {
  return {
    source: edge.source,
    source_handle: edge.sourceHandle ?? null,
    target: edge.target,
    target_handle: edge.targetHandle ?? null,
  };
}

// display

export function inferTypeForDisplay(spec: AgentConfigSpec | undefined, value: any): string {
  let ty = spec?.type;
  if (ty !== undefined && ty !== null && ty !== "*") {
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
      tys.add(inferTypeForDisplay({} as AgentConfigSpec, v));
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

// Globals

let _coreSettings: CoreSettings | null = null;
let _agentDefinitions: AgentDefinitions | null = null;
let _globalConfigsMap: AgentConfigsMap | null = null;

export function getCoreSettings(): CoreSettings {
  return _coreSettings!;
}

export async function setCoreSettings(newSettings: CoreSettings) {
  _coreSettings = newSettings;
  await setCoreSettingsUtils(newSettings);
}

export function getAgentDefinitions(): AgentDefinitions {
  return _agentDefinitions!;
}

export function getGlobalConfigsMap(): AgentConfigsMap {
  return _globalConfigsMap!;
}

export async function loadPresetInfos(): Promise<PresetInfoExt[]> {
  const presetInfos = (await getPresetInfos()) as PresetInfoExt[];
  const coreSettings = getCoreSettings();
  const auto_start_presets = coreSettings.auto_start_presets || [];
  presetInfos.forEach((s) => {
    if (auto_start_presets.includes(s.name)) {
      s.run_on_start = true;
    }
  });
  return presetInfos;
}

export async function initGlobals() {
  _coreSettings = await getCoreSettingsUtils();
  _agentDefinitions = await getAgentDefinitionsAPI();
  _globalConfigsMap = await getGlobalConfigsMapAPI();
}
