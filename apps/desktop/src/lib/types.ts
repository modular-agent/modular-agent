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

// Agent Stream Serialized Types

export type TAgentStream = {
  id: string;
  name: string;
  nodes: TAgentStreamNode[];
  edges: TAgentStreamEdge[];
  viewport: Viewport | null;
};

export type TAgentStreamNode = Node & {
  data: TAgentStreamNodeData;
  extensions?: Record<string, any>;
};

export type TAgentStreamNodeData = {
  name: string;
  enabled: boolean;
  title: string | null;
  spec: AgentSpec;
};

export type TAgentStreamEdge = Edge;
