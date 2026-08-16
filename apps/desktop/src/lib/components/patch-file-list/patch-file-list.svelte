<script lang="ts">
  import { cn } from "$lib/utils";

  import type { PatchFileListRootProps } from "./types";

  let { children, class: className }: PatchFileListRootProps = $props();
</script>

<div class={cn("flex flex-col", className)}>
  {@render children?.()}
</div>
