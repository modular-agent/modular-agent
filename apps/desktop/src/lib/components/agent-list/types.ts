import type { AgentDefinitions } from "tauri-plugin-modular-agent-api";

export type AgentListItemProps = {
  categories: Record<string, any>;
  agentDefs: AgentDefinitions;
  expandAll?: boolean;
  onAddAgent?: (agentName: string) => void;
};
