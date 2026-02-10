<script lang="ts">
  import EditorHost from "$lib/components/preset-editor/editor-host.svelte";
  import EditorTabs from "$lib/components/preset-editor/editor-tabs.svelte";
  import { tabStore } from "$lib/tab-store.svelte";

  const { children } = $props();
</script>

<div class="flex flex-col w-full h-full overflow-hidden">
  {#if tabStore.tabs.length > 1}
    <EditorTabs />
  {/if}
  <EditorHost />
  <!-- SvelteKit route slot (hidden): triggers tab activation via +page.svelte -->
  <div class="hidden">
    {@render children?.()}
  </div>
</div>
