import type { ModuleDefinitions } from "tauri-plugin-modular-agent-api";

export type ModuleListItemProps = {
  categories: Record<string, any>;
  moduleDefs: ModuleDefinitions;
  expandAll?: boolean;
  onAddModule?: (moduleName: string) => void;
};
