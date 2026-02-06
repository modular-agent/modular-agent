<script lang="ts">
  import { useSvelteFlow } from "@xyflow/svelte";
  import "@xyflow/svelte/dist/style.css";

  import { setEditor } from "$lib/components/preset-editor/context.svelte";
  import EditorCanvas from "$lib/components/preset-editor/editor-canvas.svelte";

  import type { PageProps } from "./$types";

  let { data }: PageProps = $props();
  const svelteFlow = useSvelteFlow();

  const editor = setEditor({
    preset_id: () => data.preset_id,
    flow: () => data.flow,
    svelteFlow,
  });
</script>

<div class="flex flex-col w-full h-full {editor.bgColor}">
  <EditorCanvas />
</div>
