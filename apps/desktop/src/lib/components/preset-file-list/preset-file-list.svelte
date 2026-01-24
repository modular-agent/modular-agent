<script lang="ts">
  import { cn } from "$lib/utils.js";

  import type { PresetFileListRootProps } from "./types";

  let { children, class: className }: PresetFileListRootProps = $props();
</script>

<div class={cn("flex flex-col", className)}>
  {@render children?.()}
</div>
