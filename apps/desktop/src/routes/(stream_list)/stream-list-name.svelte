<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";

  let { id, name }: { id: string; name: string } = $props();
</script>

<a href={`/stream_editor/${id}`}>
  <Button variant="link" class="p-0">
    {name}
  </Button>
</a>
