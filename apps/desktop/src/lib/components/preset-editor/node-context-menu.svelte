<script lang="ts">
  import XIcon from "@lucide/svelte/icons/x";
  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
  import { formatHotkey, getHotkeyKey, type ResolvedHotkeys } from "$lib/hotkeys";

  let {
    open = $bindable(false),
    x = 0,
    y = 0,
    selectedCount = 0,
    hotkeys = [],
    onenable,
    ondisable,
    oncut,
    oncopy,
    onexport,
    ontoggleerr,
    onalign,
    ondistribute,
    oncolor,
    onapplycolortoports,
    onclearportcolors,
  }: {
    open: boolean;
    x: number;
    y: number;
    selectedCount?: number;
    hotkeys: ResolvedHotkeys;
    onenable?: () => void;
    ondisable?: () => void;
    oncut?: () => void;
    oncopy?: () => void;
    onexport?: () => void;
    ontoggleerr?: () => void;
    onalign?: (direction: "left" | "center" | "right" | "top" | "middle" | "bottom") => void;
    ondistribute?: (direction: "horizontal" | "vertical") => void;
    oncolor?: (color: number | null) => void;
    onapplycolortoports?: () => void;
    onclearportcolors?: () => void;
  } = $props();

  const anchor = $derived({
    getBoundingClientRect: () => DOMRect.fromRect({ x, y, width: 0, height: 0 }),
  });

  function handle(fn?: () => void) {
    fn?.();
    open = false;
  }

  function hk(id: string): string {
    return formatHotkey(getHotkeyKey(hotkeys, id));
  }
</script>

<ContextMenu.Root bind:open>
  <ContextMenu.Trigger class="hidden" />
  <ContextMenu.Content
    class="w-52"
    strategy="fixed"
    updatePositionStrategy="always"
    customAnchor={anchor}
  >
    <ContextMenu.Item inset onclick={() => handle(oncut)}>
      Cut
      <ContextMenu.Shortcut>{hk("editor.cut")}</ContextMenu.Shortcut>
    </ContextMenu.Item>
    <ContextMenu.Item inset onclick={() => handle(oncopy)}>
      Copy
      <ContextMenu.Shortcut>{hk("editor.copy")}</ContextMenu.Shortcut>
    </ContextMenu.Item>
    <ContextMenu.Item inset onclick={() => handle(onexport)}>Export</ContextMenu.Item>
    <ContextMenu.Separator />
    <ContextMenu.Item inset onclick={() => handle(onenable)}>Enable</ContextMenu.Item>
    <ContextMenu.Item inset onclick={() => handle(ondisable)}>Disable</ContextMenu.Item>
    <ContextMenu.Separator />
    <ContextMenu.Item inset onclick={() => handle(ontoggleerr)}>Show Err</ContextMenu.Item>
    <ContextMenu.Separator />
    <ContextMenu.Sub>
      <ContextMenu.SubTrigger inset>Color</ContextMenu.SubTrigger>
      <ContextMenu.SubContent>
        <div class="flex items-center gap-1.5 px-2 py-1.5">
          <button
            aria-label="Default color"
            class="w-4 h-4 rounded-full border border-border flex items-center justify-center
                   text-muted-foreground hover:bg-accent"
            onclick={() => {
              oncolor?.(null);
              open = false;
            }}
            title="Default"
          >
            <XIcon size={10} />
          </button>
          {#each [1, 2, 3, 4, 5, 6] as n}
            <button
              aria-label="Color {n}"
              class="w-4 h-4 rounded-full border border-border hover:scale-110 transition-transform"
              style="background-color: var(--color-agent-{n})"
              onclick={() => {
                oncolor?.(n);
                open = false;
              }}
            ></button>
          {/each}
        </div>
        <ContextMenu.Separator />
        <ContextMenu.Item inset onclick={() => handle(onapplycolortoports)}
          >Apply to ports</ContextMenu.Item
        >
        <ContextMenu.Item inset onclick={() => handle(onclearportcolors)}
          >Clear port colors</ContextMenu.Item
        >
      </ContextMenu.SubContent>
    </ContextMenu.Sub>
    <ContextMenu.Separator />
    <ContextMenu.Sub>
      <ContextMenu.SubTrigger inset>Align</ContextMenu.SubTrigger>
      <ContextMenu.SubContent>
        <ContextMenu.Item
          disabled={selectedCount < 2}
          onclick={() => {
            onalign?.("left");
            open = false;
          }}>Align Left</ContextMenu.Item
        >
        <ContextMenu.Item
          disabled={selectedCount < 2}
          onclick={() => {
            onalign?.("center");
            open = false;
          }}>Align Center</ContextMenu.Item
        >
        <ContextMenu.Item
          disabled={selectedCount < 2}
          onclick={() => {
            onalign?.("right");
            open = false;
          }}>Align Right</ContextMenu.Item
        >
        <ContextMenu.Separator />
        <ContextMenu.Item
          disabled={selectedCount < 2}
          onclick={() => {
            onalign?.("top");
            open = false;
          }}>Align Top</ContextMenu.Item
        >
        <ContextMenu.Item
          disabled={selectedCount < 2}
          onclick={() => {
            onalign?.("middle");
            open = false;
          }}>Align Middle</ContextMenu.Item
        >
        <ContextMenu.Item
          disabled={selectedCount < 2}
          onclick={() => {
            onalign?.("bottom");
            open = false;
          }}>Align Bottom</ContextMenu.Item
        >
        <ContextMenu.Separator />
        <ContextMenu.Item
          disabled={selectedCount < 3}
          onclick={() => {
            ondistribute?.("horizontal");
            open = false;
          }}>Distribute Horizontally</ContextMenu.Item
        >
        <ContextMenu.Item
          disabled={selectedCount < 3}
          onclick={() => {
            ondistribute?.("vertical");
            open = false;
          }}>Distribute Vertically</ContextMenu.Item
        >
      </ContextMenu.SubContent>
    </ContextMenu.Sub>
  </ContextMenu.Content>
</ContextMenu.Root>
