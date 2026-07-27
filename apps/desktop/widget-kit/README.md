# @modular-agent/widget-kit

Shared SDK for Modular Agent custom node UIs. Agent packages can bring their
own Svelte 5 components into the desktop app's preset editor:

- **NodeView** — registered per agent type (`def_name`). Replaces the default
  config rendering in the node's contents area. Title, ports, and resizer stay
  in the host node frame.
- **ConfigWidget** — registered per config `type_`. Renders the input/display
  of a single config (`custom_config(type_ = "...")` on the Rust side).
  `type_` names a genuine **value type** (e.g. `color` = `#rrggbb` string) —
  never a presentation variant of a built-in type. A slider for an integer is
  a NodeView concern, not a fake `type_`.

This package is the single source of truth for the props contracts
(`NodeViewProps`, `ConfigWidgetProps`, `AgentEventState`) and shared helper
components (`ConfigHandle`). Both the desktop app and agent UI packages import
from here.

## How UI packages are discovered

An agent crate ships its UI as a sibling `ui/` npm package:

```text
modular-agent-foo/
  src/...                 # Rust (unchanged)
  ui/
    package.json          # name: "modular-agent-foo-ui", own deps (d3, three, ...)
    src/
      FooNodeView.svelte
      index.ts            # manifest export
```

`ui/src/index.ts` exports a manifest:

```ts
import FooColor from "./FooColor.svelte";
import FooNodeView from "./FooNodeView.svelte";

export const ui = {
  nodeViews: { "modular_agent_foo::FooAgent": FooNodeView }, // key: def_name
  configWidgets: { color: FooColor }, // key: config type_
};
```

At build time, the desktop app's `vite-plugin-agent-ui` reads `ma-config.toml`
and, for every agent with a `Path` source, statically imports
`<path>/ui/src/index.ts` when it exists (virtual module `virtual:agent-ui`).
No dynamic loading, no npm registry access: the UI ships with the build the
same way the Rust crate does.

Run `npm install` once inside `ui/` so package-local dependencies land in
`ui/node_modules` (the only manual step).

## UI package conventions

- **Distribute Svelte/TS source as-is.** Point `package.json`'s `svelte` /
  `exports` fields at `src/`; the desktop's `vite-plugin-svelte` compiles it.
  `svelte` must be a **peerDependency**, never a regular dependency — a second
  svelte copy in `ui/node_modules` would break the shared runtime (the desktop
  build also enforces `resolve.dedupe` as a safety net).
- Declare `@modular-agent/widget-kit` as a peerDependency and mark it optional
  via `peerDependenciesMeta` so `npm install` inside `ui/` does not try to
  fetch it from the npm registry — it resolves through a Vite alias to the
  desktop's copy at build time.
- **No Tailwind utility classes.** The desktop's Tailwind v4 does not scan
  external package sources (no `@source` configured), so utility CSS would
  never be generated. Use plain/scoped CSS and the app's CSS vars
  (`--muted`, `--border`, `--muted-foreground`, `--primary`, ...) — these also
  keep components dark-mode aware.
- **Browser-only APIs behind `onMount` guards** (canvas/WebGL/DOM
  measurement). The desktop uses the SvelteKit static adapter, so components
  are also evaluated server-side during the build.
- **NodeViews must place `ConfigHandle`** for every config that should keep
  accepting edge connections. A NodeView replaces the default per-config
  rendering — configs without a `ConfigHandle` have no handle for existing or
  new edges to attach to.

## Props contracts

See `src/types.ts` for the full documented contracts:

- `NodeViewProps` — `configs` (reactive), `configSpecs`, `updateConfig`
  (routes through the app's undo/redo coalescing), `agentEvent`,
  `connectedConfigs`, `running`. Size is intentionally not a prop: measure
  with `bind:clientWidth` / `bind:clientHeight` on your root element.
- `ConfigWidgetProps` — config-local only (`configKey`, `value`,
  `configSpec`, `readonly`, `updateConfig`) so widgets work both in the node
  view and in the sidebar inspector.

Interactive elements **must** carry xyflow's `nodrag` class (inputs, buttons,
switches, scrollbar thumbs, and any internal drag handles), and
internally-scrollable areas **must** carry `nowheel`. The node body is no
longer blanket-`nodrag`, so an interactive element without `nodrag` drags the
whole node when used, and a scroll area without `nowheel` zooms the canvas.
Write back config values on `change` rather than `input` for drag-style
controls to avoid flooding undo history and IPC. If you render agent-provided
data with `{@html}`, sanitize it first.
