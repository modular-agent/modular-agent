<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { Tween } from "svelte/motion";
  import { cubicOut } from "svelte/easing";

  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";

  import AgentConfig from "./agent-config.svelte";
  import { useEditor } from "./context.svelte";

  const editor = useEditor();
  const inspector = editor.inspector;

  const FADE_OUT_DELAY = 1500;
  const FADE_OUT_DURATION = 300;
  const FADE_IN_DURATION = 150;

  const opacity = new Tween(inspector.selectedCount > 0 ? 1 : 0);

  const DEFAULT_WIDTH = 288;
  const DEFAULT_HEIGHT = 320;

  let cardEl: HTMLElement;
  let headerEl: HTMLElement;
  let isDragging = $state(false);
  let dragOffsetX = 0;
  let dragOffsetY = 0;

  let defaultX = $state(0);
  let defaultY = $state(16);
  let x = $derived(editor.inspectorX ?? defaultX);
  let y = $derived(editor.inspectorY ?? defaultY);
  let width = $derived(editor.inspectorWidth ?? DEFAULT_WIDTH);
  let height = $derived(editor.inspectorHeight ?? DEFAULT_HEIGHT);

  let resizeObserver: ResizeObserver;

  onMount(() => {
    if (editor.inspectorX === null && cardEl?.parentElement) {
      const rect = cardEl.parentElement.getBoundingClientRect();
      defaultX = rect.width - cardEl.offsetWidth - 16;
      defaultY = 16;
    }

    resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry && !isDragging) {
        const w = Math.round(entry.borderBoxSize[0].inlineSize);
        const h = Math.round(entry.borderBoxSize[0].blockSize);
        if (w !== width) editor.inspectorWidth = w;
        if (h !== height) editor.inspectorHeight = h;
      }
    });
    resizeObserver.observe(cardEl);
  });

  onDestroy(() => {
    resizeObserver?.disconnect();
  });

  function handleDragStart(e: PointerEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    isDragging = true;
    dragOffsetX = e.clientX - x;
    dragOffsetY = e.clientY - y;
    headerEl.setPointerCapture(e.pointerId);
  }

  function handleDragMove(e: PointerEvent) {
    if (!isDragging) return;
    const parent = cardEl.parentElement!;
    const rect = parent.getBoundingClientRect();
    const cardW = cardEl.offsetWidth;
    editor.inspectorX = Math.max(0, Math.min(e.clientX - dragOffsetX, rect.width - cardW));
    editor.inspectorY = Math.max(
      0,
      Math.min(e.clientY - dragOffsetY, rect.height - cardEl.offsetHeight),
    );
  }

  function handleDragEnd() {
    isDragging = false;
  }

  function handleWindowResize() {
    if (editor.inspectorX === null || !cardEl?.parentElement) return;
    const rect = cardEl.parentElement.getBoundingClientRect();
    const maxX = rect.width - cardEl.offsetWidth;
    const maxY = rect.height - cardEl.offsetHeight;
    if (editor.inspectorX > maxX) editor.inspectorX = Math.max(0, maxX);
    if (editor.inspectorY! > maxY) editor.inspectorY = Math.max(0, maxY);
  }

  $effect(() => {
    const nothingSelected = inspector.selectedCount === 0;
    if (nothingSelected) {
      opacity.set(0, { delay: FADE_OUT_DELAY, duration: FADE_OUT_DURATION, easing: cubicOut });
    } else {
      opacity.set(1, { delay: 0, duration: FADE_IN_DURATION, easing: cubicOut });
    }
  });

  function updateConfig(key: string, value: any) {
    inspector.onUpdateConfig?.(key, value);
  }
</script>

<svelte:window onresize={handleWindowResize} />

<div
  bind:this={cardEl}
  class="absolute flex flex-col rounded-lg border border-border bg-background shadow-lg overflow-hidden resize"
  class:select-none={isDragging}
  style="left: {x}px; top: {y}px; width: {width}px; height: {height}px; min-width: 240px; min-height: 200px; max-width: calc(100% - {x}px - 16px); max-height: calc(100% - {y}px - 16px); z-index: 40; opacity: {opacity.current}; pointer-events: {opacity.current === 0 ? 'none' : 'auto'};"
  onpointerdown={(e) => e.stopPropagation()}
  role="dialog"
  aria-hidden={opacity.current === 0}
  tabindex="-1"
>
  <!-- Header (drag handle) -->
  <div
    bind:this={headerEl}
    class="flex items-center justify-between px-3 py-4 flex-none select-none"
    style="cursor: {isDragging ? 'grabbing' : 'grab'};"
    onpointerdown={handleDragStart}
    onpointermove={handleDragMove}
    onpointerup={handleDragEnd}
    role="toolbar"
    tabindex="-1"
  ></div>

  {#if inspector.hasSelection}
    <ScrollArea class="flex-1 min-h-0">
      <div class="px-3 flex flex-col gap-3">
        <!-- Agent Info -->
        <div class="flex flex-col gap-1 text-sm">
          {#if inspector.agentDef?.category}
            <div class="text-xs text-muted-foreground">{inspector.agentDef.category}</div>
          {/if}
          <div class="font-medium">{inspector.displayTitle}</div>
          {#if inspector.agentDef?.description}
            <p class="text-xs text-muted-foreground mt-1">{inspector.agentDef.description}</p>
          {/if}
        </div>

        <Separator />

        <!-- Configs -->
        {#if Object.keys(inspector.configs).length > 0}
          <form class="flex flex-col gap-2">
            {#each Object.entries(inspector.configs) as [key, value]}
              <AgentConfig
                name={key}
                {value}
                configSpec={inspector.configSpecs[key]}
                connected={inspector.connectedConfigs.includes(key)}
                {updateConfig}
                showHandle={false}
              />
            {/each}
          </form>
        {/if}
      </div>
    </ScrollArea>
  {:else}
    <div class="flex-1 flex items-center justify-center text-sm text-muted-foreground p-4">
      {#if inspector.selectedCount === 0}{:else}
        {inspector.selectedCount} nodes selected
      {/if}
    </div>
  {/if}
</div>

<style>
  :global([role="dialog"]::-webkit-resizer) {
    display: none;
  }
</style>
