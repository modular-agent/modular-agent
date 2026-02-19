import type { AgentConfigSpec, AgentDefinition } from "tauri-plugin-modular-agent-api";

/** Per-instance visual/metadata attributes stored in AgentSpec extensions. */
export const EXTENSION_KEYS = ["color"] as const;

export class InspectorState {
  // Display data (synced by EditorState)
  nodeId = $state<string | null>(null);
  defName = $state("");
  title = $state<string | null>(null);
  agentDef = $state<AgentDefinition | null>(null);
  disabled = $state(false);
  showErr = $state(false);
  configs = $state<Record<string, any>>({});
  configSpecs = $state<Record<string, AgentConfigSpec>>({});
  connectedConfigs = $state<string[]>([]);
  inputs = $state<string[]>([]);
  outputs = $state<string[]>([]);
  selectedCount = $state(0);
  extensions = $state<Record<string, any>>({});

  // Action callbacks (set by EditorState)
  onUpdateConfig: ((key: string, value: any) => void) | null = $state(null);
  onUpdateExtension: ((key: string, value: any) => void) | null = $state(null);

  get hasSelection(): boolean {
    return this.nodeId !== null;
  }

  get displayTitle(): string {
    return this.title ?? this.agentDef?.title ?? this.defName;
  }
}
