<script lang="ts">
  import FolderIcon from "@lucide/svelte/icons/folder";
  import FolderOpenIcon from "@lucide/svelte/icons/folder-open";

  import * as Collapsible from "$lib/components/ui/collapsible/index.js";
  import { cn } from "$lib/utils";

  import type { PresetFileListFolderProps } from "./types";

  let {
    name,
    open = $bindable(true),
    class: className,
    icon,
    onclick,
    children,
  }: PresetFileListFolderProps = $props();
</script>

<Collapsible.Root bind:open>
  <Collapsible.Trigger class={cn("flex place-items-center gap-1 overflow-hidden", className)} {onclick}>
    {#if icon}
      {@render icon({ name, open })}
    {:else if open}
      <FolderOpenIcon class="size-4 shrink-0" />
    {:else}
      <FolderIcon class="size-4 shrink-0" />
    {/if}
    <span class="truncate">{name}</span>
  </Collapsible.Trigger>
  <Collapsible.Content class="mx-2 border-l">
    <div class="relative flex place-items-start">
      <div class="bg-border mx-2 h-full w-px"></div>
      <div class="flex flex-col">
        {@render children?.()}
      </div>
    </div>
  </Collapsible.Content>
</Collapsible.Root>
