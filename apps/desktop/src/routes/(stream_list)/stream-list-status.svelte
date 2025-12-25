<script lang="ts">
  import CirclePlayIcon from "@lucide/svelte/icons/circle-play";

  import Badge from "@/lib/components/ui/badge/badge.svelte";

  let { active }: { active: boolean } = $props();
</script>

{#if active}
  <Badge variant="outline" class="text-muted-foreground px-1.5">
    <CirclePlayIcon color="green" />
    Running
  </Badge>
{/if}
