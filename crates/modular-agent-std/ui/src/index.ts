// UI manifest for modular-agent-std. Imported (by absolute path) from the
// desktop app's virtual:agent-ui module when modular-agent-std is a Path
// source in ma-config.toml.

import type {
  ConfigWidgetProps,
  NodeViewProps,
} from "@modular-agent/widget-kit";

import type { Component } from "svelte";

import ChartNodeView from "./ChartNodeView.svelte";
import ColorWidget from "./ColorWidget.svelte";
import SliderNodeView from "./SliderNodeView.svelte";

export const ui: {
  nodeViews: Record<string, Component<NodeViewProps>>;
  configWidgets: Record<string, Component<ConfigWidgetProps>>;
} = {
  // Keyed on def_name (macro default: module_path::StructName).
  nodeViews: {
    "modular_agent_std::example::ChartDemoAgent": ChartNodeView,
    "modular_agent_std::example::SliderDemoAgent": SliderNodeView,
  },
  // Keyed on config type_. Reserved for genuine value types — here "color",
  // a "#rrggbb" string (like the built-in "image" type is a data-URL string).
  // Alternative input methods for built-in types (e.g. a slider for an
  // integer) must NOT fake a type_: register a NodeView for the agent
  // instead (see SliderNodeView).
  configWidgets: {
    color: ColorWidget,
  },
};
