<script lang="ts">
  import { useSvelteFlow } from "@xyflow/svelte";
  import "@xyflow/svelte/dist/style.css";

  import type { PresetFlow } from "$lib/types";

  import { setEditor } from "./context.svelte";
  import EditorCanvas from "./editor-canvas.svelte";
  import RightSidebar from "./right-sidebar.svelte";

  let { tabId, flow, active }: { tabId: string; flow: PresetFlow; active: boolean } = $props();

  const svelteFlow = useSvelteFlow();
  const editor = setEditor({
    preset_id: () => tabId,
    flow: () => flow,
    svelteFlow,
  });

  // Sync active prop to EditorState
  $effect(() => {
    editor.active = active;
  });

  // Trigger SvelteFlow resize recalculation when becoming visible
  let wasActive = false;
  $effect(() => {
    if (active && !wasActive) {
      requestAnimationFrame(() => {
        window.dispatchEvent(new Event("resize"));
      });
    }
    wasActive = active;
  });
</script>

<div class="flex flex-row w-full h-full {editor.bgColor}">
  <div class="flex-1 min-w-0 flex flex-col">
    <EditorCanvas />
  </div>
  {#if editor.sidebarOpen}
    <RightSidebar />
  {/if}
</div>
