<script lang="ts" module>
  const bgColors = [
    "bg-muted dark:bg-muted",
    "bg-background dark:bg-background",
    "bg-destructive dark:bg-destructive",
  ];

  const highlightColor = "var(--color-agent-highlight)";

  const DEFAULT_HANDLE_STYLE = "width: 12px; height: 12px;";

  const HANDLE_OFFSET = 87;
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
    titleColor: string;
    contents: Snippet;
  };

  let { data, agentDef, selected, width, height, inputCount, title, titleColor, contents }: Props =
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

  let wd = $derived<number | null>(width ?? null);
  let ht = $derived<number | null>(height ?? null);

  let resizeStartWidth: number | undefined;
  let resizeStartHeight: number | undefined;

  function onResizeStart() {
    resizeStartWidth = width;
    resizeStartHeight = height;
  }

  function onResize(_ev: ResizeDragEvent, params: ResizeParams) {
    wd = params.width;
    ht = params.height;
  }

  async function onResizeEnd(_ev: ResizeDragEvent, params: ResizeParams) {
    if (!data.id) return;
    await editor.handleResizeEnd(
      data.id,
      resizeStartWidth,
      resizeStartHeight,
      params.width,
      params.height,
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
  class="{bgColor} flex flex-col p-0 border-primary border-3 rounded-xl"
  style:width={wd ? `${wd}px` : "auto"}
  style:max-width={wd ? undefined : `${DEFAULT_MAX_NODE_WIDTH}px`}
  style:height={ht ? `${ht}px` : "auto"}
  style:box-shadow={highlight.current > 0.05
    ? `0 0 ${highlight.current * 40}px ${highlightColor}`
    : ""}
>
  {#if hideTitle}
    {#if agentDef?.title === "Router"}
      <div class="w-full min-w-6 flex-none rounded-t-lg"></div>
    {:else}
      <div class="w-full min-w-40 flex-none rounded-t-lg"></div>
    {/if}
  {:else}
    <div
      class="w-full min-w-40 flex-none pl-4 pr-4 pb-2 rounded-t-lg {titleColor} text-primary-foreground"
    >
      {@render title()}
    </div>
  {/if}
  <div class="w-full flex-none grid grid-cols-2 gap-1 mt-4 mb-2">
    <div>
      {#each inputs as input}
        <div class="text-left ml-2">
          {input === "unit" ? "▸" : input}
        </div>
      {/each}
    </div>
    <div>
      {#each outputs as output}
        <div class="text-right mr-2">
          {output === "unit" ? "▸" : output}
        </div>
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
    <div class="text-right mr-2 mb-2">err</div>
  {/if}
</div>
{#each inputs as input, idx}
  {@const color = getEdgeColor(input)}
  <Handle
    id={input}
    type="target"
    position={Position.Left}
    style="top: {idx * HANDLE_GAP + handleOffset}px; {DEFAULT_HANDLE_STYLE}{color
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
    style="top: {idx * HANDLE_GAP + handleOffset}px; {DEFAULT_HANDLE_STYLE}{color
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
    style="top: {(ht ?? height ?? 100) - 20}px; {DEFAULT_HANDLE_STYLE}{errColor
      ? `background-color: ${errColor};`
      : ''}"
  />
{/if}
