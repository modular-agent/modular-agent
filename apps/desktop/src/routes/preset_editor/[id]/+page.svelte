<script lang="ts">
  import { getPresetInfo } from "tauri-plugin-modular-agent-api";

  import { tabStore } from "$lib/tab-store.svelte";

  import type { PageProps } from "./$types";

  let { data }: PageProps = $props();

  // Trigger tab activation on URL navigation (deep links, initial load)
  $effect(() => {
    const id = data.preset_id;
    if (!tabStore.tabs.find((t) => t.id === id)) {
      // Deep link: fetch name from backend and open tab
      getPresetInfo(id).then((info) => {
        if (info) tabStore.openTab(id, info.name);
      });
    }
    tabStore.activeTabId = id;
  });
</script>
