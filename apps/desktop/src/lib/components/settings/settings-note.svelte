<script lang="ts">
  import type { Snippet } from "svelte";

  const { children }: { children: Snippet } = $props();
</script>

<p class="mt-2 px-1 text-sm text-amber-600 dark:text-amber-400" role="status">
  {@render children()}
</p>
