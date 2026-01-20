<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { onMount } from "svelte";

  import { presetInfos } from "$lib/shared.svelte";

  import PresetList from "./preset-list.svelte";

  let presets = $derived.by(() =>
    Array.from(presetInfos?.values()).sort((a, b) => a.name.localeCompare(b.name)),
  );

  onMount(async () => {
    getCurrentWindow().setTitle("Presets - MAK Desktop");
  });
</script>

<header class="flex-none h-14 items-center"></header>
<PresetList {presets} />
