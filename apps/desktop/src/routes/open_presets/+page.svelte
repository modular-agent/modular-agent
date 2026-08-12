<script lang="ts">
  import { onMount } from "svelte";

  import { PresetList, PresetListEmpty } from "$lib/components/preset-list";
  import { presetTreeStore } from "$lib/preset-tree-store.svelte";
  import { titlebarState } from "$lib/titlebar-state.svelte";

  import type { PageProps } from "./$types";

  let { data }: PageProps = $props();

  onMount(() => {
    titlebarState.reset();
    titlebarState.title = "Open Presets";
  });
</script>

<div class="flex flex-col p-4 gap-4 w-full h-full">
  <header class="flex-none">
    <div class="text-lg font-semibold">Open Presets</div>
  </header>
  {#if presetTreeStore.isEmpty}
    <PresetListEmpty />
  {:else}
    <PresetList presets={data.presetInfos} />
  {/if}
</div>
