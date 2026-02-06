<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { onMount } from "svelte";

  import type { PageProps } from "./$types";
  import PresetList from "./preset-list.svelte";

  let { data }: PageProps = $props();

  onMount(async () => {
    getCurrentWindow().setTitle("Preset List - Modular Agent");
  });
</script>

<div class="flex flex-col p-4 gap-4 w-full h-full">
  <header class="flex-none">
    <div class="text-lg font-semibold">Open Presets</div>
  </header>
  <PresetList presets={data.presetInfos} />
</div>
