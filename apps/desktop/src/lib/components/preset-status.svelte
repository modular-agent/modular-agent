<script lang="ts">
  import CirclePlayIcon from "@lucide/svelte/icons/circle-play";

  import Badge from "$lib/components/ui/badge/badge.svelte";
  import { cn } from "$lib/utils";

  type Props = {
    running?: boolean | undefined;
    run_on_start?: boolean | undefined;
    class?: string;
  };

  let { running, run_on_start, class: className }: Props = $props();
</script>

<!-- The `running` badge is for the preset list, where a row has no other way to
     show its state. The editor says it with the board instead. -->
<div class={cn("flex flex-none gap-2", className)}>
  {#if running}
    <Badge variant="outline" class="text-muted-foreground px-1.5">
      <CirclePlayIcon color="var(--color-agent-2)" />
      Running
    </Badge>
  {/if}
  {#if run_on_start}
    <Badge variant="outline" class="text-muted-foreground px-1.5">
      <CirclePlayIcon color="var(--color-agent-4)" />
      on Start
    </Badge>
  {/if}
</div>
