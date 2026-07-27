import { invoke } from "@tauri-apps/api/core";

import {
  type AgentConfigSpec,
  type AgentConfigsMap,
  type AgentDefinition,
  type AgentDefinitions,
  type AgentSpec,
  type PresetInfo,
  type PresetSpec,
  type ConnectionSpec,
  getAgentDefinitions as getAgentDefinitionsAPI,
  getGlobalConfigsMap as getGlobalConfigsMapAPI,
  getPresetInfos,
} from "tauri-plugin-modular-agent-api";

import { coreSettingsStore } from "./core-settings-store.svelte";
import {
  getCoreSettings as getCoreSettingsUtils,
  setCoreSettings as setCoreSettingsUtils,
} from "./modular_agent";
import type { PresetFlow, PresetEdge, PresetNode, CoreSettings, PresetInfoExt } from "./types";

export async function newPresetWithName(name: string): Promise<string> {
  return await invoke("new_preset_with_name_cmd", { name });
}

export async function savePreset(name: string, spec: PresetSpec): Promise<void> {
  await invoke("save_preset_cmd", { name, spec });
}

export async function saveAsPreset(name: string, spec: PresetSpec): Promise<string> {
  return await invoke("save_as_preset_cmd", { name, spec });
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

  const nodeDataMap = new Map(nodes.map((n) => [n.id, n.data]));

  return {
    id: info.id,
    name: info.name,
    nodes: nodes,
    edges: validConnections.map((conn) =>
      connectionSpecToEdge(conn, nodeDataMap.get(conn.source)?.port_colors),
    ),
    running: info.running,
    viewport: spec.viewport,
  };
}

// Default node size from definition hints (grid-unit multipliers) × grid size.
// Unknown definitions or missing hints fall back to one grid unit.
export function defaultNodeSize(
  defName: string,
  gridSize: number,
): { width: number; height: number } {
  const hints = _agentDefinitions?.[defName]?.hints;
  const hintWidth = typeof hints?.width === "number" && hints.width > 0 ? hints.width : 1;
  const hintHeight = typeof hints?.height === "number" && hints.height > 0 ? hints.height : 1;
  return { width: hintWidth * gridSize, height: hintHeight * gridSize };
}

export function agentSpecToNode(spec: AgentSpec): PresetNode {
  // Specs created outside the editor (MCP, hand-written JSON) may lack a size;
  // give them the same hints-based default as GUI-added nodes.
  const fallback =
    !spec.width || !spec.height
      ? defaultNodeSize(spec.def_name, coreSettingsStore.snapGridSize)
      : null;
  return {
    id: spec.id ?? crypto.randomUUID(),
    type: "agent",
    data: { ...spec },
    position: {
      x: spec.x ?? 0,
      y: spec.y ?? 0,
    },
    width: spec.width || fallback?.width,
    height: spec.height || fallback?.height,
  };
}

// Node color palette: raw value (number 1-7) → CSS color variable
export const NODE_COLOR_VALUES: Record<number, string> = {
  1: "var(--color-agent-1)",
  2: "var(--color-agent-2)",
  3: "var(--color-agent-3)",
  4: "var(--color-agent-4)",
  5: "var(--color-agent-5)",
  6: "var(--color-agent-6)",
  7: "var(--color-agent-7)",
};

// Kind-based default color: agent kind → CSS color variable
export const KIND_COLOR_VALUES: Record<string, string> = {
  default: "var(--color-agent-4)",
  External: "var(--color-agent-5)",
  Local: "var(--color-agent-5)",
  Display: "var(--color-agent-3)",
  Input: "var(--color-agent-6)",
  UI: "var(--color-agent-2)",
};

// Kind-based default color: agent kind → palette index (numeric)
export const KIND_COLOR_DEFAULTS: Record<string, number> = {
  default: 4,
  External: 5,
  Local: 5,
  Display: 3,
  Input: 6,
  UI: 2,
};

/** Convert a raw color value (palette number or hex string) to a CSS color string. */
export function resolveColorCss(value: number | string | null | undefined): string | null {
  if (value == null) return null;
  if (typeof value === "number") return NODE_COLOR_VALUES[value] ?? null;
  if (typeof value === "string" && /^#[0-9a-fA-F]{6}$/.test(value)) return value;
  return null;
}

/** Resolve a node's title color as a CSS value (instance > hint > kind default). */
export function resolveNodeColor(data: AgentSpec, agentDef: AgentDefinition | null): string {
  const c = resolveColorCss(data.color);
  if (c) return c;
  const h = agentDef?.hints?.color;
  if (typeof h === "number" && NODE_COLOR_VALUES[h]) return NODE_COLOR_VALUES[h];
  return KIND_COLOR_VALUES[agentDef?.kind ?? "default"] ?? KIND_COLOR_VALUES.default;
}

// Connection color mapping by source handle name (type-aware ports only)
const EDGE_COLOR_MAP: Record<string, string> = {
  default: "var(--color-connection-default)",
  // unit
  unit: "var(--color-agent-2)",
  // boolean
  boolean: "var(--color-agent-3)",
  // number
  integer: "var(--color-agent-6)",
  number: "var(--color-agent-6)",
  // string-like
  string: "var(--color-agent-5)",
  text: "var(--color-agent-5)",
  // object-like
  object: "var(--color-agent-4)",
  doc: "var(--color-agent-4)",
  message: "var(--color-agent-4)",
  // image
  image: "var(--color-agent-4)",
  // error
  err: "var(--color-agent-1)",
};

export function getEdgeColor(sourceHandle: string | null | undefined): string | null {
  if (!sourceHandle) return null;
  let color = EDGE_COLOR_MAP[sourceHandle];
  if (color) return color;
  // Plural fallback: "messages" -> "message", "strings" -> "string"
  if (sourceHandle.endsWith("s")) {
    color = EDGE_COLOR_MAP[sourceHandle.slice(0, -1)];
  }
  return color ?? EDGE_COLOR_MAP["default"];
}

export function connectionSpecToEdge(
  connection: ConnectionSpec,
  sourcePortColors?: Record<string, number | string> | null,
): PresetEdge {
  let color: string | null = null;
  if (sourcePortColors && connection.source_handle && connection.source_handle !== "err") {
    color = resolveColorCss(sourcePortColors[connection.source_handle]);
  }
  if (!color) color = getEdgeColor(connection.source_handle);
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
    // A real Message always carries `role` (core serializer writes it
    // unconditionally); `content` alone is not enough — search results,
    // scraped pages, etc. also have a `content` field.
    if (
      value?.content !== undefined &&
      (typeof value.role === "string" || typeof value.type === "string")
    ) {
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
  if (!_coreSettings) {
    throw new Error("getCoreSettings() called before initGlobals()");
  }
  return _coreSettings;
}

export async function setCoreSettings(newSettings: Partial<CoreSettings>) {
  _coreSettings = { ..._coreSettings!, ...newSettings };
  coreSettingsStore.update(_coreSettings);
  await setCoreSettingsUtils(newSettings);
}

export function getAgentDefinitions(): AgentDefinitions {
  if (!_agentDefinitions) {
    throw new Error("getAgentDefinitions() called before initGlobals()");
  }
  return _agentDefinitions;
}

export function getGlobalConfigsMap(): AgentConfigsMap {
  if (!_globalConfigsMap) {
    throw new Error("getGlobalConfigsMap() called before initGlobals()");
  }
  return _globalConfigsMap;
}

export async function loadPresetInfos(): Promise<PresetInfoExt[]> {
  const presetInfos = (await getPresetInfos()) as PresetInfoExt[];
  const coreSettings = getCoreSettings();
  const auto_start_presets = coreSettings.auto_start_presets || [];
  return presetInfos.map((s) =>
    auto_start_presets.includes(s.name) ? { ...s, run_on_start: true } : s,
  );
}

export async function initGlobals() {
  _coreSettings = await getCoreSettingsUtils();
  _agentDefinitions = await getAgentDefinitionsAPI();
  _globalConfigsMap = await getGlobalConfigsMapAPI();
  coreSettingsStore.update(_coreSettings);
}
