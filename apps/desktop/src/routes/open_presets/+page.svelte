<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { onMount } from "svelte";

  import { presetInfos } from "$lib/shared.svelte";

  import PresetList from "./preset-list.svelte";

  let presets = $derived.by(() =>
    Array.from(presetInfos?.values()).sort((a, b) => a.name.localeCompare(b.name)),
  );

  onMount(async () => {
    getCurrentWindow().setTitle("Preset List - MAK Desktop");
  });
</script>

<div class="flex flex-col p-4 gap-4 w-full h-full">
  <header class="flex-none">
    <div class="text-lg font-semibold">Open Presets</div>
  </header>
  <PresetList {presets} />
</div>
