<!--
  NodeView for ChartDemoAgent: renders numeric array data from the agent's
  configs as a viewBox-based SVG line/scatter chart.

  Demonstrates the NodeView mechanism:
  - Size is measured via bind:clientWidth / bind:clientHeight on the root
    element (width: 100%), NOT via props — follows NodeResizer live resize.
  - Dark-mode-aware via CSS vars (--border, --muted-foreground, --primary).
  - nodrag / nowheel on the interactive area so the node isn't dragged while
    interacting with the chart.
  - A package-local dependency (d3-scale) that stays isolated to this UI
    package's node_modules.

  Accepts either a numeric array config value ([1, 5, 3, ...]) or an object
  whose values are numeric arrays ({ a: [...], b: [...] } → multi-series).
  Falls back to a JSON dump when no numeric series is found.
-->
<script lang="ts">
  import type { NodeViewProps } from "@modular-agent/widget-kit";

  import { scaleLinear } from "d3-scale";

  let { configs }: NodeViewProps = $props();

  type Series = { name: string; points: number[] };

  const PADDING = 8;
  const SCATTER_MAX_POINTS = 100;
  const SERIES_COLORS = [
    "var(--primary)",
    "var(--chart-2, #22c55e)",
    "var(--chart-3, #f59e0b)",
    "var(--chart-4, #8b5cf6)",
  ];

  let clientWidth = $state(0);
  let clientHeight = $state(0);

  function isNumericArray(v: unknown): v is number[] {
    return Array.isArray(v) && v.length > 0 && v.every((n) => typeof n === "number");
  }

  function toSeries(name: string, v: unknown): Series[] {
    if (isNumericArray(v)) {
      return [{ name, points: v }];
    }
    if (v && typeof v === "object" && !Array.isArray(v)) {
      const out: Series[] = [];
      for (const [k, sub] of Object.entries(v)) {
        if (isNumericArray(sub)) out.push({ name: k, points: sub });
      }
      return out;
    }
    return [];
  }

  // First config that yields at least one numeric series wins.
  const series = $derived.by(() => {
    for (const [key, v] of Object.entries(configs)) {
      const s = toSeries(key, v);
      if (s.length > 0) return s;
    }
    return [];
  });

  const valueRange = $derived.by(() => {
    let min = Infinity;
    let max = -Infinity;
    for (const s of series) {
      for (const p of s.points) {
        if (p < min) min = p;
        if (p > max) max = p;
      }
    }
    if (min === Infinity) return { min: 0, max: 1 };
    if (min === max) return { min: min - 1, max: max + 1 };
    return { min, max };
  });

  const width = $derived(clientWidth || 200);
  const height = $derived(clientHeight || 100);

  // d3-scale maps data space → viewBox space (y inverted: max on top).
  const yScale = $derived(
    scaleLinear()
      .domain([valueRange.min, valueRange.max])
      .range([height - PADDING, PADDING]),
  );

  function xScaleFor(points: number[]) {
    return scaleLinear()
      .domain([0, Math.max(points.length - 1, 1)])
      .range([PADDING, width - PADDING]);
  }

  function toPolyline(points: number[]): string {
    const xScale = xScaleFor(points);
    return points.map((v, i) => `${xScale(i).toFixed(1)},${yScale(v).toFixed(1)}`).join(" ");
  }

  function formatValue(v: number): string {
    return Number.isInteger(v) ? String(v) : v.toFixed(2);
  }
</script>

<div class="nodrag nowheel chart-root" bind:clientWidth bind:clientHeight>
  {#if series.length > 0}
    <svg width="100%" height="100%" viewBox="0 0 {width} {height}" preserveAspectRatio="none">
      <!-- frame -->
      <rect
        x={PADDING}
        y={PADDING}
        width={width - PADDING * 2}
        height={height - PADDING * 2}
        fill="none"
        stroke="var(--border)"
        stroke-width="1"
      />
      {#each series as s, si}
        {@const color = SERIES_COLORS[si % SERIES_COLORS.length]}
        {@const xScale = xScaleFor(s.points)}
        <polyline
          points={toPolyline(s.points)}
          fill="none"
          stroke={color}
          stroke-width="1.5"
          stroke-linejoin="round"
        />
        {#if s.points.length <= SCATTER_MAX_POINTS}
          {#each s.points as v, i}
            <circle cx={xScale(i)} cy={yScale(v)} r="2.5" fill={color} />
          {/each}
        {/if}
      {/each}
      <!-- min/max labels -->
      <text x={PADDING + 3} y={PADDING + 11} class="chart-label">
        {formatValue(valueRange.max)}
      </text>
      <text x={PADDING + 3} y={height - PADDING - 4} class="chart-label">
        {formatValue(valueRange.min)}
      </text>
    </svg>
  {:else}
    <div class="chart-empty">
      <p>No numeric array data</p>
      {#each Object.entries(configs) as [key, v]}
        <pre class="chart-fallback">{key}: {JSON.stringify(v)}</pre>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* Plain scoped CSS only: the desktop's Tailwind does not scan external
     package sources, so utility classes would produce no CSS here. */
  .chart-root {
    width: 100%;
    aspect-ratio: 2 / 1;
    min-height: 80px;
  }
  .chart-label {
    font-size: 10px;
    fill: var(--muted-foreground);
  }
  .chart-empty {
    padding: 0.5rem;
    font-size: 0.75rem;
    line-height: 1rem;
    color: var(--muted-foreground);
  }
  .chart-fallback {
    overflow-x: auto;
    color: var(--muted-foreground);
  }
</style>
