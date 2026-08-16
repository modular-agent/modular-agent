<script lang="ts">
  import { onMount } from "svelte";

  import { PatchList, PatchListEmpty } from "$lib/components/patch-list";
  import { patchTreeStore } from "$lib/patch-tree-store.svelte";
  import { titlebarState } from "$lib/titlebar-state.svelte";

  import type { PageProps } from "./$types";

  let { data }: PageProps = $props();

  onMount(() => {
    titlebarState.reset();
    titlebarState.title = "Open Patches";
  });
</script>

<div class="flex flex-col p-4 gap-4 w-full h-full">
  <header class="flex-none">
    <div class="text-lg font-semibold">Open Patches</div>
  </header>
  {#if patchTreeStore.isEmpty}
    <PatchListEmpty />
  {:else}
    <PatchList patches={data.patchInfos} />
  {/if}
</div>
