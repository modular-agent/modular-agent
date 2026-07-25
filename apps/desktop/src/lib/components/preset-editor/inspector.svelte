<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { Tween } from "svelte/motion";

  import XIcon from "@lucide/svelte/icons/x";

  import { KIND_COLOR_DEFAULTS } from "$lib/agent";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";
  import { renderMarkdown } from "$lib/sanitize";

  import { useEditor } from "./context.svelte";
  import SidebarConfig from "./sidebar-config.svelte";

  const editor = useEditor();
  const inspector = editor.inspector;

  const SWATCH_CLASS = "w-3 h-3 rounded-full border border-border";
  const COLOR_INPUT_CLASS = "w-4 h-5 rounded cursor-pointer border-none p-0";

  const FADE_OUT_DELAY = 1500;
  const FADE_OUT_DURATION = 300;
  const FADE_IN_DURATION = 150;

  const opacity = new Tween(inspector.selectedCount > 0 ? 1 : 0);

  const DEFAULT_WIDTH = 320;
  const DEFAULT_HEIGHT = 640;

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
  class="absolute flex flex-col rounded-lg border border-border bg-sidebar shadow-lg overflow-hidden resize"
  class:select-none={isDragging}
  style="left: {x}px; top: {y}px; width: {width}px; height: {height}px; min-width: 240px; min-height: 200px; max-width: calc(100% - {x}px - 16px); max-height: calc(100% - {y}px - 16px); z-index: 40; opacity: {opacity.current}; pointer-events: {opacity.current ===
  0
    ? 'none'
    : 'auto'};"
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
          <div class="text-lg font-medium">{inspector.displayTitle}</div>
          {#if inspector.agentDef?.description}
            {@const descriptionHtml = renderMarkdown(inspector.agentDef.description)}
            <div class="text-xs text-muted-foreground mt-1 inspector-description">
              {@html descriptionHtml}
            </div>
          {/if}
        </div>

        <Separator />

        <!-- Color -->
        <div class="flex flex-col gap-2">
          <div class="text-xs text-muted-foreground">Color</div>
          <div class="flex items-center gap-1.5 flex-wrap">
            <button
              aria-label="Reset to default color"
              class="{SWATCH_CLASS} flex items-center justify-center
                     text-muted-foreground hover:bg-accent"
              class:ring-2={inspector.extensions.color == null}
              class:ring-ring={inspector.extensions.color == null}
              onclick={() => inspector.onUpdateExtension?.("color", null)}
              title="Default"
            >
              <XIcon size={10} />
            </button>
            {#each [1, 2, 3, 4, 5, 6] as n}
              <button
                aria-label="Color {n}"
                class={SWATCH_CLASS}
                class:ring-2={inspector.extensions.color === n}
                class:ring-ring={inspector.extensions.color === n}
                style="background-color: var(--color-agent-{n})"
                onclick={() => inspector.onUpdateExtension?.("color", n)}
              ></button>
            {/each}
            <input
              type="color"
              aria-label="Custom color"
              class={COLOR_INPUT_CLASS}
              value={typeof inspector.extensions.color === "string"
                ? inspector.extensions.color
                : "#888888"}
              onchange={(e) => inspector.onUpdateExtension?.("color", e.currentTarget.value)}
            />
          </div>
          <div class="flex items-center gap-1.5">
            <button
              class="text-xs text-muted-foreground hover:text-foreground"
              onclick={() => {
                const rawColor =
                  inspector.extensions.color ??
                  inspector.agentDef?.hints?.color ??
                  KIND_COLOR_DEFAULTS[inspector.agentDef?.kind ?? "default"] ??
                  4;
                const ports = [
                  ...inspector.inputs.filter((p: string) => p !== "err"),
                  ...inspector.outputs.filter((p: string) => p !== "err"),
                ];
                if (ports.length === 0) return;
                const pc: Record<string, number | string> = {};
                for (const p of ports) pc[p] = rawColor;
                inspector.onUpdateExtension?.("port_colors", pc);
              }}>Apply to ports</button
            >
            {#if inspector.extensions.port_colors}
              <button
                class="text-xs text-muted-foreground hover:text-foreground"
                onclick={() => inspector.onUpdateExtension?.("port_colors", null)}>Clear</button
              >
            {/if}
          </div>
        </div>

        <Separator />

        <!-- Configs -->
        {#if Object.keys(inspector.configs).length > 0}
          <form class="flex flex-col gap-2">
            {#each Object.entries(inspector.configs) as [key, value]}
              <SidebarConfig
                name={key}
                {value}
                configSpec={inspector.configSpecs[key]}
                connected={inspector.connectedConfigs.includes(key)}
                {updateConfig}
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

  .inspector-description {
    overflow-wrap: break-word;
  }
  .inspector-description :global(p) {
    margin-bottom: 0.25rem;
  }
  .inspector-description :global(p:last-child) {
    margin-bottom: 0;
  }
  .inspector-description :global(code) {
    background-color: var(--muted);
    padding: 0.05rem 0.2rem;
    border-radius: 0.15rem;
    font-size: 0.85em;
  }
  .inspector-description :global(pre) {
    background-color: var(--muted);
    padding: 0.5rem;
    border-radius: 0.3rem;
    overflow-x: auto;
    margin-bottom: 0.25rem;
  }
  .inspector-description :global(pre code) {
    background-color: transparent;
    padding: 0;
  }
  .inspector-description :global(a) {
    color: var(--link-color);
    text-decoration: underline;
  }
  .inspector-description :global(a:hover) {
    opacity: 0.8;
  }
  .inspector-description :global(ul),
  .inspector-description :global(ol) {
    padding-left: 1.25rem;
    margin-bottom: 0.25rem;
  }
  .inspector-description :global(li) {
    margin-bottom: 0.1rem;
  }
  .inspector-description :global(blockquote) {
    border-left: 3px solid var(--border);
    padding-left: 0.75rem;
    margin-left: 0;
    margin-bottom: 0.25rem;
    color: var(--muted-foreground);
  }
</style>
