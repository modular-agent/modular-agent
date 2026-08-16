import type { Edge, Node } from "@xyflow/svelte";
import type { AgentSpec, PatchInfo, Viewport } from "tauri-plugin-modular-agent-api";

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

export type PatchStructureChangedMessage = {
  origin: string | null;
  patch_id: string;
};

export type PatchRemovedMessage = {
  origin: string | null;
  patch_id: string;
  name: string | null;
};

export type PatchRunningChangedMessage = {
  origin: string | null;
  patch_id: string;
  running: boolean;
};

export type PatchRenamedMessage = {
  origin: string | null;
  id: string;
  oldName: string | null;
  newName: string;
};

// for SvelteFlow

export type PatchFlow = {
  id: string;
  name: string;
  nodes: PatchNode[];
  edges: PatchEdge[];
  running: boolean;
  viewport: Viewport | null;
  /**
   * Structure-change seq observed just before this flow was fetched. The
   * editor uses it as the merge baseline so a change landing during the
   * fetch still triggers a merge after mount.
   */
  baseStructureSeq?: number;
};

export type PatchNode = Node & {
  data: AgentSpec;
  extensions?: Record<string, any>;
};

export type PatchEdge = Edge;

// Settings

export type CoreSettings = {
  autostart?: boolean;
  auto_start_patches: string[];
  color_mode?: string | null;
  run_in_background: boolean;
  shortcut_keys?: Record<string, string> | null;
  snap_enabled?: boolean;
  snap_grid_size?: number;
  grid_gap?: number;
  max_history_length?: number;
  connection_opacity?: number;
  mcp_server_enabled?: boolean;
  mcp_server_port?: number;
  // Read-only on the frontend: the backend generates and persists the token
  // and ignores any token echoed back through set_core_settings.
  mcp_server_token?: string | null;
};

export type PatchInfoExt = PatchInfo & {
  run_on_start?: boolean;
};
