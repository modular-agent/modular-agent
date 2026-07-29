<script lang="ts">
  import { untrack } from "svelte";
  import { Spring } from "svelte/motion";

  import { BaseEdge, getBezierEdgeCenter, type EdgeProps } from "@xyflow/svelte";

  import { sharedAgentEvents } from "$lib/shared.svelte";

  import { controlPoint } from "./bezier-utils";

  let {
    id,
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
    target,
    targetHandleId,
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

  // Same spring as the receiving node's glow (node-base.svelte) so the two read
  // as one event.
  let flash = new Spring(0, { stiffness: 0.03, damping: 1.0 });
  let lastSeq = 0;

  // Read without getAgent(): creating entries is the agent nodes' job.
  const targetInput = $derived(sharedAgentEvents.agents[target]?.input);

  $effect(() => {
    const input = targetInput;
    if (!input?.seq) return;
    untrack(() => {
      if (input.seq <= lastSeq) return;
      lastSeq = input.seq;
      if (input.port !== targetHandleId) return;
      flash.set(1, { instant: true });
      flash.target = 0;
    });
  });

  // The edge colour lives inside the style string built by connectionSpecToEdge.
  const strokeColor = $derived(
    /stroke:\s*([^;]+)/.exec(style ?? "")?.[1]?.trim() ?? "var(--color-connection-default)",
  );

  // Glow only — stroke-width is set in CSS, and overriding it here thins the line.
  const edgeStyle = $derived(
    flash.current > 0.02
      ? `${style ?? ""} filter: drop-shadow(0 0 ${flash.current * 10}px ${strokeColor});`
      : style,
  );
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
  interactionWidth={interactionWidth ?? 8}
  style={edgeStyle}
/>
