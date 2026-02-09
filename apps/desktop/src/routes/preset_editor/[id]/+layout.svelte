<script lang="ts">
  import { SvelteFlowProvider } from "@xyflow/svelte";

  import { page } from "$app/state";

  const { children } = $props();
</script>

{#key page.params.id}
  <SvelteFlowProvider>
    {@render children?.()}
  </SvelteFlowProvider>
{/key}
