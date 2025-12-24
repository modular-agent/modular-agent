<script lang="ts">
  import CirclePlayIcon from "@lucide/svelte/icons/circle-play";

  import Badge from "@/lib/components/ui/badge/badge.svelte";

  let { active }: { active: boolean } = $props();
</script>

{#if active}
  <Badge variant="secondary" class="bg-green-500 text-white dark:bg-green-500">
    <CirclePlayIcon />
    Running
  </Badge>
{/if}
