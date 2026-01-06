<script lang="ts" module>
  const bgColors = [
    "bg-muted dark:bg-muted",
    "bg-background dark:bg-background",
    "bg-destructive dark:bg-destructive",
  ];

  const highlightColor = "oklch(0.95 0.12 102)";

  const DEFAULT_HANDLE_STYLE = "width: 12px; height: 12px;";

  const HANDLE_OFFSET = 87;
  const HANDLE_GAP = 25.5;
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import { Spring } from "svelte/motion";

  import { Handle, NodeResizer, Position } from "@xyflow/svelte";
  import type { NodeProps, ResizeDragEvent, ResizeParams } from "@xyflow/svelte";
  import type { AgentDefinition, AgentSpec } from "tauri-plugin-askit-api";

  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";

  type Props = NodeProps & {
    data: AgentSpec;
    agentDef: AgentDefinition | null;
    titleColor: string;
    inputCount: number;
    title: Snippet;
    contents: Snippet;
  };

  let { data, agentDef, selected, height, titleColor, inputCount, title, contents }: Props =
    $props();

  const inputs = $derived(data.inputs ?? []);
  const outputs = $derived(data.outputs ?? []);
  const showErr = $derived(data.show_err ?? false);

  let bgColor = $derived(bgColors[agentDef ? (data.disabled ? 0 : 1) : 2]);

  let ht = $state<number | null>(null);

  function onResize(_ev: ResizeDragEvent, params: ResizeParams) {
    ht = params.height;
  }

  let lastInputCount = $state(0);

  let highlight = new Spring(0, {
    stiffness: 0.03,
    damping: 1.0,
  });

  $effect(() => {
    ht = height ?? ht;
    if (inputCount > lastInputCount) {
      highlight.set(1, { instant: true });
      highlight.target = 0;
      lastInputCount = inputCount;
    }
  });
</script>

<NodeResizer isVisible={selected} {onResize} />
<div
  class="{bgColor} flex flex-col p-0 border-primary border-3 rounded-xl"
  style:height={ht ? `${ht}px` : "auto"}
  style:box-shadow={highlight.current > 0.05
    ? `0 0 ${highlight.current * 40}px ${highlightColor}`
    : ""}
>
  <div class="w-full min-w-40 flex-none pl-4 pr-4 pb-2 {titleColor} rounded-t-lg">
    {@render title()}
  </div>
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
  <div class="w-full grow flex flex-col gap-2 overflow-auto min-h-0">
    <ScrollArea>
      {@render contents()}
    </ScrollArea>
  </div>
  {#if showErr}
    <div class="text-right mr-2 mb-2">err</div>
  {/if}
</div>
{#each inputs as input, idx}
  <Handle
    id={input}
    type="target"
    position={Position.Left}
    style="top: {idx * HANDLE_GAP + HANDLE_OFFSET}px; {DEFAULT_HANDLE_STYLE}"
  />
{/each}
{#each outputs as output, idx}
  <Handle
    id={output}
    type="source"
    position={Position.Right}
    style="top: {idx * HANDLE_GAP + HANDLE_OFFSET}px; {DEFAULT_HANDLE_STYLE}"
  />
{/each}
{#if showErr}
  <Handle
    id="err"
    type="source"
    position={Position.Right}
    style="top: {(ht ?? height ?? 100) - 20}px; {DEFAULT_HANDLE_STYLE}"
  />
{/if}
