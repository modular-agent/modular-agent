<script lang="ts">
  import PlayIcon from "@lucide/svelte/icons/play";
  import SquareIcon from "@lucide/svelte/icons/square";

  import { Button } from "$lib/components/ui/button/index.js";
  import { startStream, stopStream } from "$lib/shared.svelte";

  let { flow } = $props();

  let isRunning = $derived(flow.running ?? false);

  async function handleStart() {
    await startStream(flow.id);
    flow.running = true;
  }

  async function handleStop() {
    await stopStream(flow.id);
    flow.running = false;
  }
</script>

<div class="flex mr-4">
  {#if isRunning}
    <Button onclick={handleStop} variant="ghost" class="w-4">
      <SquareIcon color="red" />
    </Button>
  {:else}
    <Button onclick={handleStart} variant="ghost" class="w-4">
      <PlayIcon color="blue" />
    </Button>
  {/if}
</div>
