import type { Edge, Node } from "@xyflow/svelte";
import type { AgentSpec, PresetInfo, Viewport } from "tauri-plugin-modular-agent-api";

// Messages

export type AgentConfigUpdatedMessage = {
  origin: string | null;
  agent_id: string;
  key: string;
  value: any;
};

export type AgentErrorMessage = {
  origin: string | null;
  agent_id: string;
  message: string;
};

export type AgentInMessage = {
  origin: string | null;
  agent_id: string;
  port: string;
};

export type AgentSpecUpdatedMessage = {
  origin: string | null;
  agent_id: string;
};

export type PresetStructureChangedMessage = {
  origin: string | null;
  preset_id: string;
};

export type PresetRemovedMessage = {
  origin: string | null;
  preset_id: string;
  name: string | null;
};

export type PresetRenamedMessage = {
  origin: string | null;
  id: string;
  oldName: string | null;
  newName: string;
};

// for SvelteFlow

export type PresetFlow = {
  id: string;
  name: string;
  nodes: PresetNode[];
  edges: PresetEdge[];
  running: boolean;
  viewport: Viewport | null;
  /**
   * Structure-change seq observed just before this flow was fetched. The
   * editor uses it as the merge baseline so a change landing during the
   * fetch still triggers a merge after mount.
   */
  baseStructureSeq?: number;
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
  connection_opacity?: number;
  mcp_server_enabled?: boolean;
  mcp_server_port?: number;
  // Read-only on the frontend: the backend generates and persists the token
  // and ignores any token echoed back through set_core_settings.
  mcp_server_token?: string | null;
};

export type PresetInfoExt = PresetInfo & {
  run_on_start?: boolean;
};
