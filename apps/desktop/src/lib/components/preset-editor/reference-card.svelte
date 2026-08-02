<script lang="ts">
  import { onDestroy, onMount } from "svelte";

  import BookOpenIcon from "@lucide/svelte/icons/book-open";
  import XIcon from "@lucide/svelte/icons/x";

  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import { renderMarkdown } from "$lib/sanitize";

  import { useEditor, type RefCard } from "./context.svelte";

  let { card }: { card: RefCard } = $props();

  const editor = useEditor();

  let cardEl: HTMLElement;
  let headerEl: HTMLElement;
  let isDragging = $state(false);
  let dragOffsetX = 0;
  let dragOffsetY = 0;

  let resizeObserver: ResizeObserver;

  onMount(() => {
    resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry && !isDragging) {
        const w = Math.round(entry.borderBoxSize[0].inlineSize);
        const h = Math.round(entry.borderBoxSize[0].blockSize);
        if (w !== card.width) card.width = w;
        if (h !== card.height) card.height = h;
      }
    });
    resizeObserver.observe(cardEl);
  });

  onDestroy(() => {
    resizeObserver?.disconnect();
  });

  // Raise in capture phase: reordering the keyed each after the header takes
  // pointer capture would move this DOM node and drop the capture mid-drag
  function raiseToFront() {
    editor.bringRefCardToFront(card.defName);
  }

  function handleDragStart(e: PointerEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    isDragging = true;
    dragOffsetX = e.clientX - card.x;
    dragOffsetY = e.clientY - card.y;
    headerEl.setPointerCapture(e.pointerId);
  }

  function handleDragMove(e: PointerEvent) {
    if (!isDragging) return;
    const rect = cardEl.parentElement!.getBoundingClientRect();
    card.x = Math.max(0, Math.min(e.clientX - dragOffsetX, rect.width - cardEl.offsetWidth));
    card.y = Math.max(0, Math.min(e.clientY - dragOffsetY, rect.height - cardEl.offsetHeight));
  }

  function handleDragEnd() {
    isDragging = false;
  }

  function handleWindowResize() {
    if (!cardEl?.parentElement) return;
    const rect = cardEl.parentElement.getBoundingClientRect();
    const maxX = rect.width - cardEl.offsetWidth;
    const maxY = rect.height - cardEl.offsetHeight;
    if (card.x > maxX) card.x = Math.max(0, maxX);
    if (card.y > maxY) card.y = Math.max(0, maxY);
  }
</script>

<svelte:window onresize={handleWindowResize} />

<div
  bind:this={cardEl}
  class="ref-card absolute flex flex-col rounded-lg border border-border bg-sidebar shadow-lg overflow-hidden resize"
  class:select-none={isDragging}
  style="left: {card.x}px; top: {card.y}px; width: {card.width}px; height: {card.height}px; min-width: 240px; min-height: 160px; max-width: calc(100% - {card.x}px - 16px); max-height: calc(100% - {card.y}px - 16px); z-index: 41;"
  onpointerdown={(e) => e.stopPropagation()}
  onpointerdowncapture={raiseToFront}
  role="dialog"
  tabindex="-1"
>
  <!-- Header (drag handle) -->
  <div
    bind:this={headerEl}
    class="flex items-center justify-between gap-2 px-4 py-2 flex-none select-none border-b border-border"
    style="cursor: {isDragging ? 'grabbing' : 'grab'};"
    onpointerdown={handleDragStart}
    onpointermove={handleDragMove}
    onpointerup={handleDragEnd}
    role="toolbar"
    tabindex="-1"
  >
    <div class="flex min-w-0 items-center gap-1.5">
      <BookOpenIcon size={14} class="flex-none text-muted-foreground" />
      <div class="text-sm font-medium truncate">{card.title}</div>
    </div>
    <button
      aria-label="Close reference card"
      class="flex-none text-muted-foreground hover:text-foreground"
      onclick={() => editor.closeRefCard(card.defName)}
    >
      <XIcon size={14} />
    </button>
  </div>

  <ScrollArea class="flex-1 min-h-0">
    <div class="px-4 py-2 text-sm agent-desc-md">
      {@html renderMarkdown(card.description)}
    </div>
  </ScrollArea>
</div>

<style>
  .ref-card::-webkit-resizer {
    display: none;
  }

  .agent-desc-md {
    overflow-wrap: break-word;
  }
  .agent-desc-md :global(h1),
  .agent-desc-md :global(h2),
  .agent-desc-md :global(h3) {
    font-weight: 600;
    margin-top: 1rem;
    margin-bottom: 0.4rem;
  }
  .agent-desc-md :global(h1) {
    font-size: 1.1em;
  }
  .agent-desc-md :global(h2) {
    font-size: 1.05em;
  }
  .agent-desc-md :global(h1:first-child),
  .agent-desc-md :global(h2:first-child),
  .agent-desc-md :global(h3:first-child) {
    margin-top: 0;
  }
  .agent-desc-md :global(p) {
    margin-bottom: 0.4rem;
  }
  .agent-desc-md :global(p:last-child) {
    margin-bottom: 0;
  }
  .agent-desc-md :global(code) {
    background-color: var(--muted);
    padding: 0.1rem 0.3rem;
    border-radius: 0.2rem;
    font-size: 0.85em;
  }
  .agent-desc-md :global(pre) {
    background-color: var(--muted);
    padding: 0.5rem;
    border-radius: 0.3rem;
    overflow-x: auto;
    margin-bottom: 0.4rem;
  }
  .agent-desc-md :global(pre code) {
    background-color: transparent;
    padding: 0;
  }
  .agent-desc-md :global(a) {
    color: var(--link-color);
    text-decoration: underline;
  }
  .agent-desc-md :global(a:hover) {
    opacity: 0.8;
  }
  .agent-desc-md :global(blockquote) {
    border-left: 3px solid var(--border);
    padding-left: 0.75rem;
    margin-left: 0;
    margin-bottom: 0.4rem;
    color: var(--muted-foreground);
  }
  .agent-desc-md :global(ul),
  .agent-desc-md :global(ol) {
    padding-left: 1.5rem;
    margin-bottom: 0.4rem;
  }
  .agent-desc-md :global(li) {
    margin-bottom: 0.1rem;
  }
  .agent-desc-md :global(table) {
    border-collapse: separate;
    border-spacing: 0;
  }
  .agent-desc-md :global(th) {
    text-align: left;
    font-weight: 700;
    border-bottom: 1.5px solid var(--border);
    padding-bottom: 0.35rem;
  }
  .agent-desc-md :global(td) {
    text-align: left;
  }
  .agent-desc-md :global(th),
  .agent-desc-md :global(td) {
    padding: 0;
  }
  .agent-desc-md :global(th:not(:first-child)),
  .agent-desc-md :global(td:not(:first-child)) {
    padding-left: 1rem;
  }
  .agent-desc-md :global(tbody td) {
    padding-top: 0.35rem;
  }
</style>
