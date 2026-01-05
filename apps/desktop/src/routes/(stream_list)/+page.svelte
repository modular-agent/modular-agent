<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { onMount } from "svelte";

  import { streamInfos } from "$lib/shared.svelte";

  import StreamList from "./stream-list.svelte";

  let streams = $derived.by(() =>
    Array.from(streamInfos?.values()).sort((a, b) => a.name.localeCompare(b.name)),
  );

  onMount(async () => {
    getCurrentWindow().setTitle("Streams - Agent Stream App");
  });
</script>

<header class="flex-none h-14 items-center"></header>
<StreamList {streams} />
