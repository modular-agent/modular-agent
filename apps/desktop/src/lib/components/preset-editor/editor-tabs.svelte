<script lang="ts">
  import { goto } from "$app/navigation";

  import XIcon from "@lucide/svelte/icons/x";

  import ScrollArea from "$lib/components/ui/scroll-area/scroll-area.svelte";
  import { tabStore } from "$lib/tab-store.svelte";

  let viewportRef: HTMLElement | null = $state(null);

  $effect(() => {
    const el = viewportRef;
    if (!el) return;
    const handleWheel = (e: WheelEvent) => {
      if (Math.abs(e.deltaX) > Math.abs(e.deltaY)) return;
      if (e.deltaY !== 0) {
        el.scrollLeft += e.deltaY;
        e.preventDefault();
      }
    };
    el.addEventListener("wheel", handleWheel, { passive: false });
    return () => el.removeEventListener("wheel", handleWheel);
  });

  function getDisplayName(name: string): string {
    const lastSlash = name.lastIndexOf("/");
    return lastSlash >= 0 ? name.substring(lastSlash + 1) : name;
  }

  async function handleTabClick(id: string) {
    if (tabStore.activeTabId === id) return;
    tabStore.activeTabId = id;
    try {
      await goto(`/preset_editor/${id}`, { noScroll: true });
    } catch {
      tabStore.closeTab(id);
      if (tabStore.tabs.length === 0) {
        goto("/open_presets");
      } else {
        goto(`/preset_editor/${tabStore.activeTabId}`, { noScroll: true });
      }
    }
  }

  async function handleTabClose(event: Event, id: string) {
    event.stopPropagation();
    const wasActive = tabStore.activeTabId === id;
    tabStore.closeTab(id);
    if (!wasActive) return;
    if (tabStore.tabs.length === 0) {
      await goto("/open_presets");
    } else {
      await goto(`/preset_editor/${tabStore.activeTabId}`, { noScroll: true });
    }
  }

  function handleMouseDown(event: MouseEvent, id: string) {
    if (event.button === 1) {
      event.preventDefault();
      handleTabClose(event, id);
    }
  }
</script>

<ScrollArea
  orientation="horizontal"
  bind:viewportRef
  class="bg-sidebar border-b border-sidebar-border select-none shrink-0"
>
  <div class="flex items-center gap-0.5 px-1 py-0.5">
    {#each tabStore.tabs as tab (tab.id)}
      {@const isActive = tab.id === tabStore.activeTabId}
      <button
        class="flex items-center gap-1.5 pl-3 pr-1.5 py-1 rounded-md text-xs min-w-[100px] max-w-[200px] transition-colors
          {isActive
          ? 'bg-muted text-foreground font-medium'
          : 'text-muted-foreground hover:bg-muted/50'}"
        onclick={() => handleTabClick(tab.id)}
        onmousedown={(e) => handleMouseDown(e, tab.id)}
        title={tab.name}
      >
        <span class="flex-1 truncate text-left">{getDisplayName(tab.name)}</span>
        <span
          role="button"
          tabindex="-1"
          class="flex-none size-4 rounded-sm hover:bg-muted-foreground/20 inline-flex items-center justify-center"
          onclick={(e) => handleTabClose(e, tab.id)}
          onkeydown={(e) => {
            if (e.key === "Enter") handleTabClose(e, tab.id);
          }}
          aria-label="Close tab"
        >
          <XIcon class="size-3" />
        </span>
      </button>
    {/each}
  </div>
</ScrollArea>
