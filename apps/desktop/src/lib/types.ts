import type { Edge, Node } from "@xyflow/svelte";
import type { Viewport } from "tauri-plugin-askit-api";

export type TAgentFlow = {
  nodes: TAgentFlowNode[];
  edges: TAgentFlowEdge[];
  name: string;
  viewport: Viewport | null;
};

export type TAgentFlowNode = Node & {
  data: TAgentFlowNodeData;
};

export type TAgentFlowNodeData = {
  name: string;
  enabled: boolean;
  title: string | null;
  config: TAgentFlowNodeConfig | null;
  display: TAgentFlowNodeDisplay | null;
};

export type TAgentFlowNodeConfig = Record<string, any>;
export type TAgentFlowNodeDisplay = Record<string, any>;

export type TAgentFlowEdge = Edge;
