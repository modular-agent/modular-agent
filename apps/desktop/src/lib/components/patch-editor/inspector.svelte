<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { Tween } from "svelte/motion";

  import BookOpenIcon from "@lucide/svelte/icons/book-open";
  import XIcon from "@lucide/svelte/icons/x";

  import { KIND_COLOR_DEFAULTS } from "$lib/agent";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import * as Tabs from "$lib/components/ui/tabs/index.js";

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

  let activeTab = $state("configs");

  const DEFAULT_WIDTH = 328;
  const DEFAULT_HEIGHT = 640;

  let cardEl: HTMLElement;
  let headerEl: HTMLElement;
  let isDragging = $state(false);
  const hasDesc = $derived(!!inspector.agentDef?.description?.trim());
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
  style="left: {x}px; top: {y}px; width: {width}px; height: {height}px; min-width: 240px; min-height: 200px; max-width: calc(100% - {x}px - 16px); max-height: calc(100% - {y}px - 16px); z-index: 40; opacity: {opacity.current}; pointer-events: {inspector.selectedCount ===
  0
    ? 'none'
    : 'auto'};"
  onpointerdown={(e) => e.stopPropagation()}
  role="dialog"
  aria-hidden={inspector.selectedCount === 0}
  tabindex="-1"
>
  <!-- Header (drag handle) -->
  <div
    bind:this={headerEl}
    class="px-4 pt-3 pb-2 flex-none select-none"
    style="cursor: {isDragging ? 'grabbing' : 'grab'};"
    onpointerdown={handleDragStart}
    onpointermove={handleDragMove}
    onpointerup={handleDragEnd}
    role="toolbar"
    tabindex="-1"
  >
    {#if inspector.hasSelection}
      <!-- Agent Info -->
      <div class="relative flex flex-col gap-1 text-sm">
        {#if inspector.agentDef?.category}
          <div class="text-xs text-muted-foreground">{inspector.agentDef.category}</div>
        {/if}
        <div class="text-lg font-medium">{inspector.displayTitle}</div>
        {#if hasDesc}
          <button
            class="absolute top-0 right-0 flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
            onclick={() =>
              editor.openRefCard(
                inspector.defName,
                inspector.displayTitle,
                inspector.agentDef?.description ?? "",
              )}
          >
            <BookOpenIcon size={12} />
            Ref
          </button>
        {/if}
      </div>
    {/if}
  </div>

  {#if inspector.hasSelection}
    <ScrollArea class="flex-1 min-h-0">
      <div class="px-4 pb-4 flex flex-col gap-3">
        <Tabs.Root bind:value={activeTab}>
          <Tabs.List variant="line" class="h-6 gap-4">
            <Tabs.Trigger value="configs" class="px-0 text-xs after:-bottom-1.5"
              >Configs</Tabs.Trigger
            >
            <Tabs.Trigger value="colors" class="px-0 text-xs after:-bottom-1.5">Colors</Tabs.Trigger
            >
          </Tabs.List>

          <Tabs.Content value="configs" class="flex flex-col gap-3 pt-3">
            {#if Object.keys(inspector.configs).length > 0}
              <form class="flex flex-col gap-4">
                {#each Object.entries(inspector.configs) as [key, value]}
                  <SidebarConfig
                    name={key}
                    {value}
                    configSpec={inspector.configSpecs[key]}
                    {updateConfig}
                  />
                {/each}
              </form>
            {:else}
              <div class="text-xs text-muted-foreground">No configs</div>
            {/if}
          </Tabs.Content>

          <Tabs.Content value="colors" class="flex flex-col gap-4 pt-3">
            <!-- Color -->
            <div class="flex flex-col gap-2">
              <div>Color</div>
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

            <!-- Background -->
            <div class="flex flex-col gap-2">
              <div>Background</div>
              <div class="flex items-center gap-1.5 flex-wrap">
                <button
                  aria-label="Reset to default background color"
                  class="{SWATCH_CLASS} flex items-center justify-center
                     text-muted-foreground hover:bg-accent"
                  class:ring-2={inspector.extensions.bg_color == null}
                  class:ring-ring={inspector.extensions.bg_color == null}
                  onclick={() => inspector.onUpdateExtension?.("bg_color", null)}
                  title="Default"
                >
                  <XIcon size={10} />
                </button>
                {#each [1, 2, 3, 4, 5, 6] as n}
                  <button
                    aria-label="Background color {n}"
                    class={SWATCH_CLASS}
                    class:ring-2={inspector.extensions.bg_color === n}
                    class:ring-ring={inspector.extensions.bg_color === n}
                    style="background-color: var(--color-agent-{n})"
                    onclick={() => inspector.onUpdateExtension?.("bg_color", n)}
                  ></button>
                {/each}
                <input
                  type="color"
                  aria-label="Custom background color"
                  class={COLOR_INPUT_CLASS}
                  value={typeof inspector.extensions.bg_color === "string"
                    ? inspector.extensions.bg_color
                    : "#888888"}
                  onchange={(e) => inspector.onUpdateExtension?.("bg_color", e.currentTarget.value)}
                />
              </div>
            </div>

            <!-- Text -->
            <div class="flex flex-col gap-2">
              <div>Text</div>
              <div class="flex items-center gap-1.5 flex-wrap">
                <button
                  aria-label="Reset to default text color"
                  class="{SWATCH_CLASS} flex items-center justify-center
                     text-muted-foreground hover:bg-accent"
                  class:ring-2={inspector.extensions.fg_color == null}
                  class:ring-ring={inspector.extensions.fg_color == null}
                  onclick={() => inspector.onUpdateExtension?.("fg_color", null)}
                  title="Default"
                >
                  <XIcon size={10} />
                </button>
                {#each [1, 2, 3, 4, 5, 6] as n}
                  <button
                    aria-label="Text color {n}"
                    class={SWATCH_CLASS}
                    class:ring-2={inspector.extensions.fg_color === n}
                    class:ring-ring={inspector.extensions.fg_color === n}
                    style="background-color: var(--color-agent-{n})"
                    onclick={() => inspector.onUpdateExtension?.("fg_color", n)}
                  ></button>
                {/each}
                <input
                  type="color"
                  aria-label="Custom text color"
                  class={COLOR_INPUT_CLASS}
                  value={typeof inspector.extensions.fg_color === "string"
                    ? inspector.extensions.fg_color
                    : "#888888"}
                  onchange={(e) => inspector.onUpdateExtension?.("fg_color", e.currentTarget.value)}
                />
              </div>
            </div>
          </Tabs.Content>
        </Tabs.Root>
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
