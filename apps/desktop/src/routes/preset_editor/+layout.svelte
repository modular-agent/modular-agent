<script lang="ts">
  import EditorTabs from "$lib/components/preset-editor/editor-tabs.svelte";
  import { tabStore } from "$lib/tab-store.svelte";

  const { children } = $props();
</script>

<div class="flex flex-col w-full h-full overflow-hidden">
  {#if tabStore.tabs.length > 1}
    <EditorTabs />
  {/if}
  {@render children?.()}
</div>
