<script lang="ts">
  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";

  let {
    open = $bindable(false),
    x = 0,
    y = 0,
    onenable,
    ondisable,
    oncut,
    oncopy,
    ontoggleerr,
  } = $props();

  const anchor = $derived({
    getBoundingClientRect: () => DOMRect.fromRect({ x, y, width: 0, height: 0 }),
  });

  function handleEnable() {
    onenable?.();
    open = false;
  }

  function handleDisable() {
    ondisable?.();
    open = false;
  }

  function handleCut() {
    oncut?.();
    open = false;
  }

  function handleCopy() {
    oncopy?.();
    open = false;
  }

  function handleToggleErr() {
    ontoggleerr?.();
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
    <ContextMenu.Item inset onclick={handleCut}>
      Cut
      <ContextMenu.Shortcut>ctl-X</ContextMenu.Shortcut>
    </ContextMenu.Item>
    <ContextMenu.Item inset onclick={handleCopy}>
      Copy
      <ContextMenu.Shortcut>ctl-C</ContextMenu.Shortcut>
    </ContextMenu.Item>
    <ContextMenu.Separator />
    <ContextMenu.Item inset onclick={handleEnable}>Enable</ContextMenu.Item>
    <ContextMenu.Item inset onclick={handleDisable}>Disable</ContextMenu.Item>
    <ContextMenu.Separator />
    <ContextMenu.Item inset onclick={handleToggleErr}>Err</ContextMenu.Item>
  </ContextMenu.Content>
</ContextMenu.Root>
