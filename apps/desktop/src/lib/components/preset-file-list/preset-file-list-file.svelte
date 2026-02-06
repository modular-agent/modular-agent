<script lang="ts">
  import FileIcon from "@lucide/svelte/icons/file";

  import { cn } from "$lib/utils";

  import type { PresetFileListFileProps } from "./types";

  let {
    name,
    icon,
    type = "button",
    class: className,
    ...rest
  }: PresetFileListFileProps = $props();
</script>

<button {type} class={cn("flex place-items-center gap-1 pl-[3px] overflow-hidden", className)} {...rest}>
  {#if icon}
    {@render icon({ name })}
  {:else}
    <FileIcon class="size-4 shrink-0" />
  {/if}
  <span class="truncate">{name}</span>
</button>
