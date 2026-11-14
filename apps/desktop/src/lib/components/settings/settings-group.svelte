<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title?: string;
    children: Snippet;
  }

  const { title, children }: Props = $props();
</script>

<section>
  {#if title}
    <h3 class="mb-2 px-1 text-sm font-medium text-muted-foreground">{title}</h3>
  {/if}
  <div class="divide-y rounded-xl bg-muted/70 px-4">
    {@render children()}
  </div>
</section>
