<!--
  NodeView for SliderDemoAgent: renders the agent's integer configs as range
  sliders.

  A slider is an alternative input method for an integer — NOT a value type —
  so it is implemented as a NodeView (per agent type) instead of a
  ConfigWidget keyed on a fake type_. Also demonstrates placing ConfigHandle
  so configs keep accepting edge connections when the default rendering is
  replaced.

  Writes back via updateConfig on "change" (not "input") so continuous drags
  don't flood the undo history / IPC; a local drag value keeps the thumb and
  numeric label under the user's control while dragging, so external config
  updates (e.g. a wire driving this config) can't yank the thumb mid-drag.
-->
<script lang="ts">
  import { ConfigHandle, type NodeViewProps } from "@modular-agent/widget-kit";

  let { configs, configSpecs, updateConfig }: NodeViewProps = $props();

  // AgentConfigSpec carries no min/max/step metadata today; use sensible defaults.
  const MIN = 0;
  const MAX = 100;
  const STEP = 1;

  const sliderKeys = $derived(
    Object.keys(configs).filter((key) => {
      const spec = configSpecs[key];
      return spec?.type === "integer" && spec?.hidden !== true && spec?.readonly !== true;
    }),
  );

  // Live values while dragging (display only — commit happens on "change").
  let dragValues = $state<Record<string, number>>({});

  function numValue(key: string): number {
    const v = configs[key];
    return typeof v === "number" ? v : Number(v) || 0;
  }
</script>

<div class="slider-root">
  {#each sliderKeys as key (key)}
    {@const spec = configSpecs[key]}
    <div class="config-row">
      {#if spec?.hide_title !== true}
        <h3 class="config-title">{spec?.title || key}</h3>
        <ConfigHandle name={key} />
      {/if}
    </div>
    <div class="slider-row">
      <input
        type="range"
        class="nodrag slider-input"
        min={MIN}
        max={MAX}
        step={STEP}
        value={dragValues[key] ?? numValue(key)}
        oninput={(evt) => {
          dragValues[key] = Number(evt.currentTarget.value);
        }}
        onchange={(evt) => {
          delete dragValues[key];
          const v = Number(evt.currentTarget.value);
          if (!Number.isNaN(v) && v !== numValue(key)) {
            updateConfig(key, v);
          }
        }}
      />
      <span class="slider-value">
        {dragValues[key] ?? numValue(key)}
      </span>
    </div>
  {/each}
  {#if sliderKeys.length === 0}
    <p class="slider-empty">No integer configs</p>
  {/if}
</div>

<style>
  /* Plain scoped CSS only: the desktop's Tailwind does not scan external
     package sources, so utility classes would produce no CSS here. */
  .slider-root {
    flex-grow: 1;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0 1.75rem 1rem 1.75rem;
  }
  .config-row {
    flex: none;
    position: relative;
    display: flex;
    align-items: center;
  }
  .config-title {
    margin-left: 0.75rem;
  }
  .slider-row {
    flex: none;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .slider-input {
    flex-grow: 1;
    accent-color: var(--primary);
  }
  .slider-value {
    flex: none;
    width: 2rem;
    text-align: right;
    font-size: 0.875rem;
    line-height: 1.25rem;
    color: var(--muted-foreground);
    font-variant-numeric: tabular-nums;
  }
  .slider-empty {
    font-size: 0.75rem;
    line-height: 1rem;
    color: var(--muted-foreground);
  }
</style>
