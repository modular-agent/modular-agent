<script lang="ts">
  import PlayIcon from "@lucide/svelte/icons/play";
  import SquareIcon from "@lucide/svelte/icons/square";

  import { Button } from "$lib/components/ui/button/index.js";

  type Props = {
    running: boolean;
    onStartPreset?: () => Promise<void>;
    onStopPreset?: () => Promise<void>;
  };

  let { running, onStartPreset, onStopPreset }: Props = $props();

  async function handleStart() {
    await onStartPreset?.();
  }

  async function handleStop() {
    await onStopPreset?.();
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
