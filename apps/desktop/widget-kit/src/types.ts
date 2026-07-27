import type { AgentConfigSpec } from "tauri-plugin-modular-agent-api";

/**
 * Reactive per-agent event state passed to NodeViews.
 *
 * Defined structurally here because widget-kit cannot depend on
 * desktop-internal modules. KEEP IN SYNC with `AgentEventState` in
 * modular-agent-desktop/src/lib/shared.svelte.ts (the runtime source is the
 * $state proxy managed by SharedAgentEvents there).
 */
export type AgentEventState = {
  configUpdated: { key: string; value: unknown; seq: number };
  error: { message: string; seq: number };
  input: { port: string; seq: number };
  specUpdated: number;
};

/**
 * Props contract for a NodeView: a component registered per agent type
 * (def_name) that replaces the default config iteration in the node's
 * contents area. Title, ports, and resizer stay in node-base.svelte.
 *
 * Size is intentionally NOT part of the contract: the node's saved
 * width/height props are undefined until first resize and stale during
 * resize drags. NodeViews measure themselves with bind:clientWidth /
 * bind:clientHeight on their root element (CSS width: 100%) instead — this
 * follows both live resize and auto sizing naturally.
 *
 * Dark mode is available via CSS vars / mode-watcher; no prop needed.
 */
export interface NodeViewProps {
  nodeId: string;
  defName: string;
  /** = data.configs (reactive) */
  configs: Record<string, unknown>;
  configSpecs: Record<string, AgentConfigSpec>;
  /** Routes through the existing setAgentConfigs path (undo/redo pushCoalescing applies). */
  updateConfig: (key: string, value: unknown) => void;
  /** The agent-node's existing agentEvent ($state proxy, reactive through props). */
  agentEvent: AgentEventState;
  /** Config keys currently connected by an edge (disable inputs while connected). */
  connectedConfigs: string[];
  running: boolean;
}

/**
 * Props contract for a ConfigWidget: a component registered per config
 * type_ that renders the input/display of a single config.
 *
 * Deliberately minimal (does not extend NodeViewProps): the sidebar
 * (inspector) has no node context such as agentEvent, so widgets must
 * depend only on config-local information to be reusable in both the
 * node view and the sidebar.
 */
export interface ConfigWidgetProps {
  /** The config key this widget is responsible for. */
  configKey: string;
  /** = configs[configKey] */
  value: unknown;
  /** Spec of this config. */
  configSpec: AgentConfigSpec;
  /** True when used at the display position of a readonly config. */
  readonly: boolean;
  updateConfig: (key: string, value: unknown) => void;
}
