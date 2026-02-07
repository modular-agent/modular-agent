<script lang="ts">
  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";

  let {
    open = $bindable(false),
    x = 0,
    y = 0,
    selectedCount = 0,
    onenable,
    ondisable,
    oncut,
    oncopy,
    ontoggleerr,
    onalign,
    ondistribute,
  }: {
    open: boolean;
    x: number;
    y: number;
    selectedCount?: number;
    onenable?: () => void;
    ondisable?: () => void;
    oncut?: () => void;
    oncopy?: () => void;
    ontoggleerr?: () => void;
    onalign?: (direction: "left" | "center" | "right" | "top" | "middle" | "bottom") => void;
    ondistribute?: (direction: "horizontal" | "vertical") => void;
  } = $props();

  const anchor = $derived({
    getBoundingClientRect: () => DOMRect.fromRect({ x, y, width: 0, height: 0 }),
  });

  function handle(fn?: () => void) {
    fn?.();
    open = false;
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
      <ContextMenu.Shortcut>ctl-X</ContextMenu.Shortcut>
    </ContextMenu.Item>
    <ContextMenu.Item inset onclick={() => handle(oncopy)}>
      Copy
      <ContextMenu.Shortcut>ctl-C</ContextMenu.Shortcut>
    </ContextMenu.Item>
    <ContextMenu.Separator />
    <ContextMenu.Item inset onclick={() => handle(onenable)}>Enable</ContextMenu.Item>
    <ContextMenu.Item inset onclick={() => handle(ondisable)}>Disable</ContextMenu.Item>
    <ContextMenu.Separator />
    <ContextMenu.Item inset onclick={() => handle(ontoggleerr)}>Err</ContextMenu.Item>
    <ContextMenu.Separator />
    <ContextMenu.Sub>
      <ContextMenu.SubTrigger inset>Align</ContextMenu.SubTrigger>
      <ContextMenu.SubContent>
        <ContextMenu.Item disabled={selectedCount < 2} onclick={() => { onalign?.("left"); open = false; }}>Align Left</ContextMenu.Item>
        <ContextMenu.Item disabled={selectedCount < 2} onclick={() => { onalign?.("center"); open = false; }}>Align Center</ContextMenu.Item>
        <ContextMenu.Item disabled={selectedCount < 2} onclick={() => { onalign?.("right"); open = false; }}>Align Right</ContextMenu.Item>
        <ContextMenu.Separator />
        <ContextMenu.Item disabled={selectedCount < 2} onclick={() => { onalign?.("top"); open = false; }}>Align Top</ContextMenu.Item>
        <ContextMenu.Item disabled={selectedCount < 2} onclick={() => { onalign?.("middle"); open = false; }}>Align Middle</ContextMenu.Item>
        <ContextMenu.Item disabled={selectedCount < 2} onclick={() => { onalign?.("bottom"); open = false; }}>Align Bottom</ContextMenu.Item>
        <ContextMenu.Separator />
        <ContextMenu.Item disabled={selectedCount < 3} onclick={() => { ondistribute?.("horizontal"); open = false; }}>Distribute Horizontally</ContextMenu.Item>
        <ContextMenu.Item disabled={selectedCount < 3} onclick={() => { ondistribute?.("vertical"); open = false; }}>Distribute Vertically</ContextMenu.Item>
      </ContextMenu.SubContent>
    </ContextMenu.Sub>
  </ContextMenu.Content>
</ContextMenu.Root>
