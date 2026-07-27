<script lang="ts">
  import { untrack } from "svelte";

  import { getPresetInfo } from "tauri-plugin-modular-agent-api";

  import { tabStore } from "$lib/tab-store.svelte";

  import type { PageProps } from "./$types";

  let { data }: PageProps = $props();

  // Trigger tab activation on URL navigation (deep links, initial load).
  // Only react to route changes (data.preset_id), NOT tabStore.tabs changes.
  // Without untrack, closeTab modifying tabs would re-trigger this effect
  // before navigation completes, causing the deep-link handler to re-add
  // the just-closed tab.
  $effect(() => {
    const id = data.preset_id;
    untrack(() => {
      if (!tabStore.tabs.find((t) => t.id === id)) {
        // Deep link: fetch name from backend and open tab
        getPresetInfo(id).then((info) => {
          if (info) tabStore.openTab(id, info.name);
        });
      }
      tabStore.activeTabId = id;
    });
  });
</script>
