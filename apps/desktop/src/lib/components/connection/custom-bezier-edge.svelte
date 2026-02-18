<script lang="ts">
  import { BaseEdge, getBezierEdgeCenter, type EdgeProps } from "@xyflow/svelte";
  import { controlPoint } from "./bezier-utils";

  let {
    id,
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
    style,
    interactionWidth,
    label,
    labelStyle,
    markerStart,
    markerEnd,
  }: EdgeProps = $props();

  let pathData = $derived.by(() => {
    const [scx, scy] = controlPoint(sourcePosition, sourceX, sourceY, targetX, targetY);
    const [tcx, tcy] = controlPoint(targetPosition, targetX, targetY, sourceX, sourceY);
    const path = `M${sourceX},${sourceY} C${scx},${scy} ${tcx},${tcy} ${targetX},${targetY}`;
    const [labelX, labelY] = getBezierEdgeCenter({
      sourceX,
      sourceY,
      targetX,
      targetY,
      sourceControlX: scx,
      sourceControlY: scy,
      targetControlX: tcx,
      targetControlY: tcy,
    });
    return { path, labelX, labelY };
  });
</script>

<BaseEdge
  {id}
  path={pathData.path}
  labelX={pathData.labelX}
  labelY={pathData.labelY}
  {label}
  {labelStyle}
  {markerStart}
  {markerEnd}
  {interactionWidth}
  {style}
/>
