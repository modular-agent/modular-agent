<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";

  type Props = {
    id: string;
    name: string;
  };

  let { id, name }: Props = $props();
</script>

<a href={`/preset_editor/${id}`}>
  <Button variant="link" class="p-0">
    {name}
  </Button>
</a>
