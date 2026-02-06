import type { AgentDefinitions } from "tauri-plugin-modular-agent-api";

export type AgentListItemProps = {
  categories: Record<string, any>;
  agentDefs: AgentDefinitions;
  expandAll?: boolean;
  onDragAgentStart?: (event: DragEvent, agentName: string) => void;
};
