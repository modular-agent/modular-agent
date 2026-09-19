// UI manifest for modular-agent-std. Imported (by absolute path) from the
// desktop app's virtual:module-ui module when modular-agent-std is a Path
// source in ma-config.toml.

import type {
  ConfigWidgetProps,
  NodeStyle,
  NodeViewProps,
} from "@modular-agent/widget-kit";

import type { Component } from "svelte";

import ChartNodeView from "./ChartNodeView.svelte";
import ColorWidget from "./ColorWidget.svelte";
import SliderNodeView from "./SliderNodeView.svelte";

export const ui: {
  nodeViews: Record<string, Component<NodeViewProps>>;
  configWidgets: Record<string, Component<ConfigWidgetProps>>;
  nodeStyles: Record<string, NodeStyle>;
} = {
  // Keyed on def_name (macro default: module_path::StructName).
  nodeViews: {
    "modular_agent_std::example::ChartDemoModule": ChartNodeView,
    "modular_agent_std::example::SliderDemoModule": SliderNodeView,
  },
  // Keyed on config type_. Reserved for genuine value types — here "color",
  // a "#rrggbb" string (like the built-in "image" type is a data-URL string).
  // Alternative input methods for built-in types (e.g. a slider for an
  // integer) must NOT fake a type_: register a NodeView for the module
  // instead (see SliderNodeView).
  configWidgets: {
    color: ColorWidget,
  },
  // Keyed on def_name. Frame presentation overrides for the host node.
  nodeStyles: {
    // Sticky-note look: translucent body so the canvas shows through.
    "modular_agent_std::ui::NoteModule": {
      bodyBackground: (color) =>
        `color-mix(in srgb, ${color} 85%, transparent)`,
    },
  },
};
