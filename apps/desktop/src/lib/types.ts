import type { Edge, Node } from "@xyflow/svelte";
import type { AgentSpec, Viewport } from "tauri-plugin-askit-api";

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
  display_values: Record<string, any> | null;
};

export type TAgentStreamNodeDisplays = Record<string, any>;

export type TAgentStreamEdge = Edge;
