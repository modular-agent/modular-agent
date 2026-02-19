<script lang="ts" module>
  const bgColors = [
    "bg-muted dark:bg-muted",
    "bg-agent-background dark:bg-agent-background",
    "bg-destructive dark:bg-destructive",
  ];

  const highlightColor = "var(--color-agent-highlight)";

  const DEFAULT_HANDLE_STYLE = "width: 12px; height: 12px;";

  const HANDLE_INSET = 20;
  const HANDLE_OFFSET = 73;
  const HANDLE_OFFSET_NO_TITLE = 32;
  const HANDLE_GAP = 25.5;
  const DEFAULT_MAX_NODE_HEIGHT = 500;
  const DEFAULT_MAX_NODE_WIDTH = 500;
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import { Spring } from "svelte/motion";

  import { Handle, NodeResizer, Position } from "@xyflow/svelte";
  import type { NodeProps, ResizeDragEvent, ResizeParams } from "@xyflow/svelte";
  import { type AgentDefinition, type AgentSpec } from "tauri-plugin-modular-agent-api";

  import { getEdgeColor } from "$lib/agent";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";

  import { useEditor } from "./context.svelte";

  type Props = NodeProps & {
    data: AgentSpec;
    agentDef: AgentDefinition | null;
    inputCount: number;
    title: Snippet;
    contents: Snippet;
  };

  let { data, agentDef, selected, width, height, inputCount, title, contents }: Props =
    $props();

  const editor = useEditor();

  const inputs = $derived(data.inputs ?? []);
  const outputs = $derived(data.outputs ?? []);
  const showErr = $derived(data.show_err ?? false);

  let hideTitle = $derived(agentDef?.hide_title ?? false);
  let bgColor = $derived(bgColors[agentDef ? (data.disabled ? 0 : 1) : 2]);

  let clientHeight = $state(0);
  let handleOffset = $derived(
    hideTitle
      ? clientHeight > 0 && clientHeight < HANDLE_OFFSET_NO_TITLE * 2
        ? clientHeight / 2 + 2
        : HANDLE_OFFSET_NO_TITLE
      : HANDLE_OFFSET,
  );

  let wd = $state<number | null>(null);
  let ht = $state<number | null>(null);
  let localResizing = false;

  // Sync from props when not resizing (for undo/redo, backend updates)
  $effect(() => {
    const w = width ?? null;
    const h = height ?? null;
    if (!localResizing) {
      wd = w;
      ht = h;
    }
  });

  function snapToGrid(value: number, gridSize: number): number {
    return Math.max(gridSize, Math.round(value / gridSize) * gridSize);
  }

  let resizeStartWidth: number | undefined;
  let resizeStartHeight: number | undefined;

  function onResizeStart() {
    localResizing = true;
    resizeStartWidth = width;
    resizeStartHeight = height;
    editor.resizing = true;
  }

  function onResize(_ev: ResizeDragEvent, params: ResizeParams) {
    if (editor.snapActive) {
      const g = editor.snapGridSize;
      wd = snapToGrid(params.width, g);
      ht = snapToGrid(params.height, g);
    } else {
      wd = params.width;
      ht = params.height;
    }
  }

  async function onResizeEnd(_ev: ResizeDragEvent, params: ResizeParams) {
    if (!data.id) return;
    let finalWidth = params.width;
    let finalHeight = params.height;
    if (editor.snapActive) {
      const g = editor.snapGridSize;
      finalWidth = snapToGrid(params.width, g);
      finalHeight = snapToGrid(params.height, g);
    }
    editor.props.svelteFlow.updateNode(data.id, { width: finalWidth, height: finalHeight });
    wd = finalWidth;
    ht = finalHeight;
    localResizing = false;
    editor.resizing = false;
    await editor.handleResizeEnd(
      data.id,
      resizeStartWidth,
      resizeStartHeight,
      finalWidth,
      finalHeight,
    );
  }

  let lastInputCount = $state(0);

  let highlight = new Spring(0, {
    stiffness: 0.03,
    damping: 1.0,
  });

  $effect(() => {
    if (inputCount > lastInputCount) {
      highlight.set(1, { instant: true });
      highlight.target = 0;
      lastInputCount = inputCount;
    }
  });
</script>

<NodeResizer isVisible={selected} {onResizeStart} {onResize} {onResizeEnd} />
<div
  bind:clientHeight
  class="flex flex-col p-1"
  style:width={wd ? `${wd}px` : "auto"}
  style:max-width={wd ? undefined : `${DEFAULT_MAX_NODE_WIDTH}px`}
  style:height={ht ? `${ht}px` : "auto"}
  style:box-shadow={highlight.current > 0.05
    ? `0 0 ${highlight.current * 40}px ${highlightColor}`
    : ""}
>
  <div class="{bgColor} flex flex-col grow min-h-0 p-0 border-none rounded-xl">
    {#if hideTitle}
      {#if agentDef?.title === "Router"}
        <div class="w-full min-w-6 flex-none rounded-t-lg"></div>
      {:else}
        <div class="w-full min-w-40 flex-none rounded-t-lg"></div>
      {/if}
    {:else}
      <div class="w-full min-w-40 flex-none pl-4 pr-4 pb-2 rounded-t-lg">
        {@render title()}
      </div>
    {/if}
    <div class="w-full flex-none grid grid-cols-2 gap-1 mt-4 mb-2">
      <div>
        {#each inputs as input}
          {@const color = getEdgeColor(input)}
          {#if input === "unit"}
            <div class="text-left text-[1.55rem] leading-none ml-6" style:color>▸</div>
          {:else}
            <div class="text-left ml-7" style:color>
              {input}
            </div>
          {/if}
        {/each}
      </div>
      <div>
        {#each outputs as output}
          {@const color = getEdgeColor(output)}
          {#if output === "unit"}
            <div class="text-right text-[1.55rem] leading-none mr-5" style:color>▸</div>
          {:else}
            <div class="text-right mr-7" style:color>
              {output}
            </div>
          {/if}
        {/each}
      </div>
    </div>
    <div
      class="w-full grow flex flex-col gap-2 overflow-hidden min-h-0"
      style:max-height={ht ? undefined : `${DEFAULT_MAX_NODE_HEIGHT}px`}
    >
      <ScrollArea class="h-full nodrag nowheel">
        {@render contents()}
      </ScrollArea>
    </div>
    {#if showErr}
      {@const errLabelColor = getEdgeColor("err")}
      <div class="text-right mr-5 mb-2" style:color={errLabelColor}>err</div>
    {/if}
  </div>
</div>

{#each inputs as input, idx}
  {@const color = getEdgeColor(input)}
  <Handle
    id={input}
    type="target"
    position={Position.Left}
    style="top: {idx * HANDLE_GAP +
      handleOffset}px; left: {HANDLE_INSET}px; {DEFAULT_HANDLE_STYLE}{color
      ? `background-color: ${color};`
      : ''}"
  />
{/each}
{#each outputs as output, idx}
  {@const color = getEdgeColor(output)}
  <Handle
    id={output}
    type="source"
    position={Position.Right}
    style="top: {idx * HANDLE_GAP +
      handleOffset}px; right: {HANDLE_INSET}px; {DEFAULT_HANDLE_STYLE}{color
      ? `background-color: ${color};`
      : ''}"
  />
{/each}
{#if showErr}
  {@const errColor = getEdgeColor("err")}
  <Handle
    id="err"
    type="source"
    position={Position.Right}
    style="top: {(ht ?? height ?? 100) -
      20}px; right: {HANDLE_INSET}px; {DEFAULT_HANDLE_STYLE}{errColor
      ? `background-color: ${errColor};`
      : ''}"
  />
{/if}
