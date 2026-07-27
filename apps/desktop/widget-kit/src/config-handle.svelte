<!--
  ConfigHandle: the edge-connection Handle for a config, following the
  "config:{name}" id convention used by the default rendering in
  agent-config.svelte. NodeViews that replace the contents area must place
  this themselves for every config that should accept edge connections —
  otherwise existing preset edges to that config have no handle to attach to.

  Must be rendered inside a relatively positioned row element (the default
  rendering uses <div class="flex-none relative flex items-center">) so that
  `top: 50%; left: 2px` anchors to that row.
-->
<script lang="ts">
  import { Handle, Position } from "@xyflow/svelte";

  type Props = {
    /** Config key. Becomes handle id "config:{name}" — must match the default convention. */
    name: string;
    /** Extra inline style appended after the default handle style. */
    style?: string;
  };

  let { name, style = "" }: Props = $props();

  // Keep in sync with CONFIG_HANDLE_STYLE in agent-config.svelte.
  const CONFIG_HANDLE_STYLE =
    "width: 10px; height: 10px; background-color: #000; border: 2px solid #fff;";
</script>

<Handle
  id="config:{name}"
  type="target"
  position={Position.Left}
  style={`top: 50%; left: 2px; ${CONFIG_HANDLE_STYLE}${style}`}
/>
