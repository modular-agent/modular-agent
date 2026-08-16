<script lang="ts">
  import PlayIcon from "@lucide/svelte/icons/play";
  import SquareIcon from "@lucide/svelte/icons/square";

  import { Button } from "$lib/components/ui/button/index.js";

  type Props = {
    running: boolean;
    onStartPatch?: () => Promise<void>;
    onStopPatch?: () => Promise<void>;
  };

  let { running, onStartPatch, onStopPatch }: Props = $props();

  async function handleStart() {
    await onStartPatch?.();
  }

  async function handleStop() {
    await onStopPatch?.();
  }
</script>

<div class="flex flex-row">
  {#if running}
    <Button onclick={handleStop} variant="ghost" class="w-4">
      <SquareIcon color="var(--color-agent-1)" />
    </Button>
  {:else}
    <Button onclick={handleStart} variant="ghost" class="w-4">
      <PlayIcon color="var(--color-agent-6)" />
    </Button>
  {/if}
</div>
