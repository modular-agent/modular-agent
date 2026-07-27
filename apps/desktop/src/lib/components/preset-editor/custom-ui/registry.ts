import type { Component } from "svelte";

import type { ConfigWidgetProps, NodeViewProps } from "@modular-agent/widget-kit";

// Plain non-reactive Maps are sufficient: registration happens once at app
// initialization (module eval, before any node renders) and never changes.
const nodeViews = new Map<string, Component<NodeViewProps>>(); // key: def_name (contents area)
const configWidgets = new Map<string, Component<ConfigWidgetProps>>(); // key: config type_ (single config)

export function registerNodeView(defName: string, comp: Component<NodeViewProps>) {
  nodeViews.set(defName, comp);
}

export function getNodeView(defName: string): Component<NodeViewProps> | undefined {
  return nodeViews.get(defName);
}

export function registerConfigWidget(typeName: string, comp: Component<ConfigWidgetProps>) {
  configWidgets.set(typeName, comp);
}

export function getConfigWidget(
  typeName: string | null | undefined,
): Component<ConfigWidgetProps> | undefined {
  return typeName ? configWidgets.get(typeName) : undefined;
}
