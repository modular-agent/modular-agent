import type { Edge, Node } from "@xyflow/svelte";
import type { ModuleSpec, ModuleStatus, PatchInfo, Viewport } from "tauri-plugin-modular-agent-api";

// Messages

export type ModuleConfigUpdatedMessage = {
  origin: string | null;
  module_id: string;
  key: string;
  value: any;
};

export type ModuleErrorMessage = {
  origin: string | null;
  module_id: string;
  message: string;
};

export type ModuleInMessage = {
  origin: string | null;
  module_id: string;
  port: string;
};

export type ModuleSpecUpdatedMessage = {
  origin: string | null;
  module_id: string;
};

export type ModuleStatusChangedMessage = {
  origin: string | null;
  module_id: string;
  status: ModuleStatus;
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
  data: ModuleSpec;
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
