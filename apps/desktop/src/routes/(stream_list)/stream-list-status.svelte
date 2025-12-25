<script lang="ts">
  import CirclePlayIcon from "@lucide/svelte/icons/circle-play";

  import Badge from "@/lib/components/ui/badge/badge.svelte";

  let { active, run_on_start }: { active: boolean; run_on_start?: boolean } = $props();
</script>

<div class="w-24">
  {#if active}
    <Badge variant="outline" class="text-muted-foreground px-1.5">
      <CirclePlayIcon color="green" />
      Running
    </Badge>
  {/if}
  {#if run_on_start}
    <Badge variant="outline" class="text-muted-foreground px-1.5 ml-2">
      <CirclePlayIcon color="blue" />
      on Start
    </Badge>
  {/if}
</div>
