import type { Edge, Node } from "@xyflow/svelte";
import type { AgentSpec, Viewport } from "tauri-plugin-askit-api";

export type TAgentFlow = {
  id: string;
  name: string;
  nodes: TAgentFlowNode[];
  edges: TAgentFlowEdge[];
  viewport: Viewport | null;
};

export type TAgentFlowNode = Node & {
  data: TAgentFlowNodeData;
  extensions?: Record<string, any>;
};

export type TAgentFlowNodeData = {
  name: string;
  enabled: boolean;
  title: string | null;
  spec: AgentSpec;
  // configs: TAgentFlowNodeConfigs | null;
  display_values: Record<string, any> | null;
};

// export type TAgentFlowNodeConfigs = Record<string, any>;
export type TAgentFlowNodeDisplays = Record<string, any>;

export type TAgentFlowEdge = Edge;
