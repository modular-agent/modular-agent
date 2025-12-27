import type { Edge, Node } from "@xyflow/svelte";
import type { AgentSpec, Viewport } from "tauri-plugin-askit-api";

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
  ch: string;
};

export type AgentSpecUpdatedMessage = {
  agent_id: string;
};

// for SvelteFlow

export type AgentStreamFlow = {
  id: string;
  name: string;
  nodes: AgentStreamNode[];
  edges: AgentStreamEdge[];
  running: boolean;
  run_on_start: boolean;
  viewport: Viewport | null;
};

export type AgentStreamNode = Node & {
  data: AgentSpec;
  extensions?: Record<string, any>;
};

export type AgentStreamEdge = Edge;

// Settings

export type CoreSettings = {
  autostart?: boolean;
  color_mode?: string | null;
  shortcut_keys?: Record<string, string> | null;
};
