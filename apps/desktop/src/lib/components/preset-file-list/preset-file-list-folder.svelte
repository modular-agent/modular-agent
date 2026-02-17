<script lang="ts">
  import FolderIcon from "@lucide/svelte/icons/folder";
  import FolderOpenIcon from "@lucide/svelte/icons/folder-open";

  import * as Collapsible from "$lib/components/ui/collapsible/index.js";
  import { cn } from "$lib/utils";

  import type { PresetFileListFolderProps } from "./types";

  let {
    name,
    depth = 0,
    title: titleText,
    open = $bindable(true),
    class: className,
    draggable: isDraggable = false,
    droptarget = false,
    icon,
    onclick,
    ondragstart,
    ondragenter,
    ondragleave,
    ondrop,
    children,
  }: PresetFileListFolderProps = $props();
</script>

<Collapsible.Root bind:open draggable={isDraggable} {ondragstart}>
  <Collapsible.Trigger
    class={cn(
      "flex w-full place-items-center gap-1 overflow-hidden hover:bg-sidebar-accent hover:text-sidebar-accent-foreground",
      droptarget && "bg-foreground/20",
      className,
    )}
    style="padding-left: {8 + depth * 16}px"
    title={titleText}
    {onclick}
    {ondragenter}
    {ondragleave}
    {ondrop}
  >
    {#if icon}
      {@render icon({ name, open })}
    {:else if open}
      <FolderOpenIcon class="size-4 shrink-0" />
    {:else}
      <FolderIcon class="size-4 shrink-0" />
    {/if}
    <span class="truncate">{name}</span>
  </Collapsible.Trigger>
  <Collapsible.Content>
    {@render children?.()}
  </Collapsible.Content>
</Collapsible.Root>
