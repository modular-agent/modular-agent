<script lang="ts">
  import { listen } from "@tauri-apps/api/event";

  import { onMount } from "svelte";
  import { untrack } from "svelte";

  import { SvelteFlowProvider } from "@xyflow/svelte";
  import { toast } from "svelte-sonner";
  import { getPatchInfo, getPatchSpec } from "tauri-plugin-modular-agent-api";

  import { patchToFlow } from "$lib/agent";
  import { closePatch } from "$lib/modular_agent";
  import { sharedPatchEvents } from "$lib/shared.svelte";
  import { tabStore } from "$lib/tab-store.svelte";
  import type { PatchFlow, PatchRenamedMessage } from "$lib/types";

  import EditorInstance from "./editor-instance.svelte";

  // Loaded flow data per tab (reads inside untrack to avoid infinite loop)
  let flows = $state.raw<Record<string, PatchFlow>>({});
  let loading = $state<Set<string>>(new Set());

  // Listen for patch rename events (from move operations).
  // Deliberately not origin-filtered: sidebar renames arrive as our own echo
  // and this is the only path that updates tab names. The flow update below
  // changes only `name` and keeps the nodes/edges array identities, which the
  // editor's flow-sync effect relies on to leave the canvas untouched.
  onMount(() => {
    const unlisten = listen<PatchRenamedMessage>("ma:patch_renamed", (event) => {
      const { id, newName } = event.payload;
      if (id in flows) {
        flows = { ...flows, [id]: { ...flows[id], name: newName } };
      }
      tabStore.updateName(id, newName);
    });
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
          // Unload stopped patch from backend (fire-and-forget)
          closePatch(id).catch((e) => console.error("Failed to close patch:", e));
        }
      }
    });
  });

  async function loadFlow(id: string) {
    loading = new Set([...loading, id]);
    // Capture the merge baseline before the fetch: a structure change that
    // lands while the IPC is in flight bumps the seq past this value, so the
    // editor still schedules a merge for it after mount.
    const baseStructureSeq = sharedPatchEvents.structureChanged[id] ?? 0;
    try {
      const info = await getPatchInfo(id);
      // Re-check after await — tab might have been closed during IPC
      if (!tabStore.tabs.find((t) => t.id === id)) return;

      const spec = await getPatchSpec(id);
      if (!info || !spec) {
        console.error("Patch not found:", id);
        return;
      }
      const flow = patchToFlow(info, spec);
      flow.baseStructureSeq = baseStructureSeq;
      // Check tab still exists before setting
      if (tabStore.tabs.find((t) => t.id === id)) {
        flows = { ...flows, [id]: flow };
      }
    } catch (e) {
      console.error("Failed to load patch:", id, e);
      toast.error("Failed to load patch");
    } finally {
      const next = new Set(loading);
      next.delete(id);
      loading = next;
      // If tab was closed while loading, ensure backend cleanup
      if (!tabStore.tabs.find((t) => t.id === id)) {
        closePatch(id).catch((e) => console.error("Failed to close patch:", e));
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
        inert={!isActive}
      >
        <SvelteFlowProvider>
          <EditorInstance tabId={tab.id} {flow} active={isActive} />
        </SvelteFlowProvider>
      </div>
    {/if}
  {/each}
</div>
