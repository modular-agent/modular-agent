<script lang="ts">
  import PlayIcon from "@lucide/svelte/icons/play";
  import SquareIcon from "@lucide/svelte/icons/square";

  import { Button } from "$lib/components/ui/button/index.js";

  import { runningStreams, startStream, stopStream } from "@/lib/shared.svelte";

  let { stream } = $props();

  let isRunning = $derived(runningStreams.has(stream.id));

  async function handleStart() {
    await startStream(stream.id);
  }

  async function handleStop() {
    await stopStream(stream.id);
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
