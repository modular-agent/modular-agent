<script lang="ts">
  import CirclePlayIcon from "@lucide/svelte/icons/circle-play";

  import Badge from "$lib/components/ui/badge/badge.svelte";

  type Props = {
    running: boolean;
    run_on_start?: boolean;
  };

  let { running, run_on_start }: Props = $props();
</script>

<div class="flex flex-none gap-2">
  {#if running}
    <Badge variant="outline" class="text-muted-foreground px-1.5">
      <CirclePlayIcon color="green" />
      Running
    </Badge>
  {/if}
  {#if run_on_start}
    <Badge variant="outline" class="text-muted-foreground px-1.5">
      <CirclePlayIcon color="blue" />
      on Start
    </Badge>
  {/if}
</div>
