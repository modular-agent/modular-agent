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
  configs: TAgentFlowNodeConfigs | null;
  displays: TAgentFlowNodeDisplays | null;
};

export type TAgentFlowNodeConfigs = Record<string, any>;
export type TAgentFlowNodeDisplays = Record<string, any>;

export type TAgentFlowEdge = Edge;
