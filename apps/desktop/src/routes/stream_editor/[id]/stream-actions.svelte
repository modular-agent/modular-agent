<script lang="ts">
  import PlayIcon from "@lucide/svelte/icons/play";
  import SquareIcon from "@lucide/svelte/icons/square";

  import { Button } from "$lib/components/ui/button/index.js";
  import { startStream, stopStream } from "$lib/shared.svelte";

  type Props = {
    stream_id: string;
    running: boolean;
  };

  let { stream_id, running = $bindable() }: Props = $props();

  async function handleStart() {
    await startStream(stream_id);
    running = true;
  }

  async function handleStop() {
    await stopStream(stream_id);
    running = false;
  }
</script>

<div class="flex flex-row">
  {#if running}
    <Button onclick={handleStop} variant="ghost" class="w-4">
      <SquareIcon color="red" />
    </Button>
  {:else}
    <Button onclick={handleStart} variant="ghost" class="w-4">
      <PlayIcon color="blue" />
    </Button>
  {/if}
</div>
