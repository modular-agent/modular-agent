<script lang="ts">
  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
  import { formatHotkey, getHotkeyKey, type ResolvedHotkeys } from "$lib/hotkeys";

  let {
    open = $bindable(false),
    x = 0,
    y = 0,
    running = false,
    snapEnabled = false,
    showGrid = false,
    hotkeys = [],
    onstart,
    onstop,
    onnew,
    onsave,
    onsaveas,
    onimport,
    onexport,
    onpaste,
    onaddagent,
    ontogglesnap,
    ontogglegrid,
  }: {
    open: boolean;
    x: number;
    y: number;
    running: boolean;
    snapEnabled: boolean;
    showGrid: boolean;
    hotkeys: ResolvedHotkeys;
    onstart?: () => void;
    onstop?: () => void;
    onnew?: () => void;
    onsave?: () => void;
    onsaveas?: () => void;
    onimport?: () => void;
    onexport?: () => void;
    onpaste?: () => void;
    onaddagent?: () => void;
    ontogglesnap?: () => void;
    ontogglegrid?: () => void;
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
    {#if running}
      <ContextMenu.Item inset onclick={() => handle(onstop)}>
        Stop
        <ContextMenu.Shortcut>{hk("editor.toggle_run")}</ContextMenu.Shortcut>
      </ContextMenu.Item>
    {:else}
      <ContextMenu.Item inset onclick={() => handle(onstart)}>
        Play
        <ContextMenu.Shortcut>{hk("editor.toggle_run")}</ContextMenu.Shortcut>
      </ContextMenu.Item>
    {/if}
    <ContextMenu.Separator />
    <ContextMenu.Item inset onclick={() => handle(onaddagent)}>
      Add agent
      <ContextMenu.Shortcut>{hk("editor.add_agent")}</ContextMenu.Shortcut>
    </ContextMenu.Item>
    <ContextMenu.Separator />
    <ContextMenu.Item inset onclick={() => handle(onpaste)}>
      Paste
      <ContextMenu.Shortcut>{hk("editor.paste")}</ContextMenu.Shortcut>
    </ContextMenu.Item>
    <ContextMenu.Separator />
    <ContextMenu.Sub>
      <ContextMenu.SubTrigger inset>File</ContextMenu.SubTrigger>
      <ContextMenu.SubContent>
        <ContextMenu.Item onclick={() => handle(onnew)}>New</ContextMenu.Item>
        <ContextMenu.Item onclick={() => handle(onsave)}>Save</ContextMenu.Item>
        <ContextMenu.Item onclick={() => handle(onsaveas)}>Save as...</ContextMenu.Item>
        <ContextMenu.Separator />
        <ContextMenu.Item onclick={() => handle(onimport)}>Import</ContextMenu.Item>
        <ContextMenu.Item onclick={() => handle(onexport)}>Export</ContextMenu.Item>
      </ContextMenu.SubContent>
    </ContextMenu.Sub>
    <ContextMenu.Sub>
      <ContextMenu.SubTrigger inset>View</ContextMenu.SubTrigger>
      <ContextMenu.SubContent>
        <ContextMenu.CheckboxItem checked={snapEnabled} onCheckedChange={() => handle(ontogglesnap)}>
          Snap to Grid
        </ContextMenu.CheckboxItem>
        <ContextMenu.CheckboxItem checked={showGrid} onCheckedChange={() => handle(ontogglegrid)}>
          Show Grid
          <ContextMenu.Shortcut>{hk("editor.toggle_grid")}</ContextMenu.Shortcut>
        </ContextMenu.CheckboxItem>
      </ContextMenu.SubContent>
    </ContextMenu.Sub>
  </ContextMenu.Content>
</ContextMenu.Root>
