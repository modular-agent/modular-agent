<script lang="ts">
  import { SvelteFlowProvider } from "@xyflow/svelte";
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";
  import { untrack } from "svelte";
  import { getPresetInfo, getPresetSpec } from "tauri-plugin-modular-agent-api";

  import { listen } from "@tauri-apps/api/event";

  import { presetToFlow } from "$lib/agent";
  import { closePreset } from "$lib/modular_agent";
  import { tabStore } from "$lib/tab-store.svelte";
  import type { PresetFlow } from "$lib/types";

  import EditorInstance from "./editor-instance.svelte";

  // Loaded flow data per tab (reads inside untrack to avoid infinite loop)
  let flows = $state.raw<Record<string, PresetFlow>>({});
  let loading = $state<Set<string>>(new Set());

  // Listen for preset rename events (from move operations)
  onMount(() => {
    const unlisten = listen<{ id: string; newName: string }>(
      "ma:preset_renamed",
      (event) => {
        const { id, newName } = event.payload;
        if (id in flows) {
          flows = { ...flows, [id]: { ...flows[id], name: newName } };
        }
        tabStore.updateName(id, newName);
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  });

  // Watch tabStore.tabs changes only — untrack flows reads/writes
  $effect(() => {
    const currentTabs = tabStore.tabs;
    const tabIds = new Set(currentTabs.map((t) => t.id));

    untrack(() => {
      // Load data for new tabs
      for (const tab of currentTabs) {
        if (!(tab.id in flows) && !loading.has(tab.id)) {
          loadFlow(tab.id);
        }
      }
      // Cleanup closed tabs
      for (const id of Object.keys(flows)) {
        if (!tabIds.has(id)) {
          const { [id]: _, ...rest } = flows;
          flows = rest;
          // Unload stopped preset from backend (fire-and-forget)
          closePreset(id).catch((e) => console.error("Failed to close preset:", e));
        }
      }
    });
  });

  async function loadFlow(id: string) {
    loading = new Set([...loading, id]);
    try {
      const info = await getPresetInfo(id);
      // Re-check after await — tab might have been closed during IPC
      if (!tabStore.tabs.find((t) => t.id === id)) return;

      const spec = await getPresetSpec(id);
      if (!info || !spec) {
        console.error("Preset not found:", id);
        return;
      }
      const flow = presetToFlow(info, spec);
      // Check tab still exists before setting
      if (tabStore.tabs.find((t) => t.id === id)) {
        flows = { ...flows, [id]: flow };
      }
    } catch (e) {
      console.error("Failed to load preset:", id, e);
      toast.error("Failed to load preset");
    } finally {
      const next = new Set(loading);
      next.delete(id);
      loading = next;
      // If tab was closed while loading, ensure backend cleanup
      if (!tabStore.tabs.find((t) => t.id === id)) {
        closePreset(id).catch((e) =>
          console.error("Failed to close preset:", e),
        );
      }
    }
  }
</script>

<div class="relative flex-1 min-h-0">
  {#each tabStore.tabs as tab (tab.id)}
    {@const isActive = tab.id === tabStore.activeTabId}
    {@const flow = flows[tab.id]}
    {#if flow}
      <div
        class="absolute inset-0"
        style:visibility={isActive ? "visible" : "hidden"}
        style:z-index={isActive ? 1 : 0}
        style:pointer-events={isActive ? "auto" : "none"}
      >
        <SvelteFlowProvider>
          <EditorInstance tabId={tab.id} {flow} active={isActive} />
        </SvelteFlowProvider>
      </div>
    {/if}
  {/each}
</div>
