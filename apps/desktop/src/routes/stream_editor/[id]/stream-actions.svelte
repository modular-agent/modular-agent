<script lang="ts">
  import PlayIcon from "@lucide/svelte/icons/play";
  import SquareIcon from "@lucide/svelte/icons/square";

  import { Button } from "$lib/components/ui/button/index.js";

  let { onPause, onPlay } = $props();
</script>

<div class="flex mr-4">
  <Button onclick={onPause} variant="ghost" class="w-4">
    <SquareIcon />
  </Button>
  <Button onclick={onPlay} variant="ghost" class="w-4">
    <PlayIcon />
  </Button>
</div>
