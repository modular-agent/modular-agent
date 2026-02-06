<script lang="ts">
  import { onMount } from "svelte";

  import type { PageProps } from "./$types";
  import { PresetList } from "$lib/components/preset-list";
  import { titlebarState } from "$lib/titlebar-state.svelte";

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
  <PresetList presets={data.presetInfos} />
</div>
