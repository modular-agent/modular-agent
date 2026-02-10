import type { Edge, Node } from "@xyflow/svelte";
import type { AgentSpec, PresetInfo, Viewport } from "tauri-plugin-modular-agent-api";

// Messages

export type AgentConfigUpdatedMessage = {
  agent_id: string;
  key: string;
  value: any;
};

export type AgentErrorMessage = {
  agent_id: string;
  message: string;
};

export type AgentInMessage = {
  agent_id: string;
  port: string;
};

export type AgentSpecUpdatedMessage = {
  agent_id: string;
};

// for SvelteFlow

export type PresetFlow = {
  id: string;
  name: string;
  nodes: PresetNode[];
  edges: PresetEdge[];
  running: boolean;
  viewport: Viewport | null;
};

export type PresetNode = Node & {
  data: AgentSpec;
  extensions?: Record<string, any>;
};

export type PresetEdge = Edge;

// Settings

export type CoreSettings = {
  autostart?: boolean;
  auto_start_presets: string[];
  color_mode?: string | null;
  run_in_background: boolean;
  shortcut_keys?: Record<string, string> | null;
  snap_enabled?: boolean;
  snap_grid_size?: number;
  show_grid?: boolean;
  grid_gap?: number;
  max_history_length?: number;
};

export type PresetInfoExt = PresetInfo & {
  run_on_start?: boolean;
};
