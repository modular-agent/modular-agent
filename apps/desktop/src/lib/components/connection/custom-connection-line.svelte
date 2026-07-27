<script lang="ts">
  import { useConnection } from "@xyflow/svelte";

  import { controlPoint } from "./bezier-utils";

  const connection = useConnection();

  let path = $derived.by(() => {
    const c = connection.current;
    if (!c.inProgress) return "";
    const [scx, scy] = controlPoint(c.fromPosition, c.from.x, c.from.y, c.to.x, c.to.y);
    const [tcx, tcy] = controlPoint(c.toPosition, c.to.x, c.to.y, c.from.x, c.from.y);
    return `M${c.from.x},${c.from.y} C${scx},${scy} ${tcx},${tcy} ${c.to.x},${c.to.y}`;
  });
</script>

<path d={path} fill="none" class="svelte-flow__connection-path" />
