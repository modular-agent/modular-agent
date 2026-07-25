<script lang="ts">
  import FileIcon from "@lucide/svelte/icons/file";

  import { cn } from "$lib/utils";

  import type { PresetFileListFileProps } from "./types";

  let {
    name,
    depth = 0,
    icon,
    type = "button",
    class: className,
    ...rest
  }: PresetFileListFileProps = $props();
</script>

<button
  {type}
  class={cn(
    "flex w-full place-items-center gap-1 overflow-hidden hover:bg-sidebar-accent hover:text-sidebar-accent-foreground",
    className,
  )}
  style:padding-left="{8 + depth * 16}px"
  draggable="true"
  {...rest}
>
  {#if icon}
    {@render icon({ name })}
  {:else}
    <FileIcon class="size-4 shrink-0" />
  {/if}
  <span class="truncate">{name}</span>
</button>
