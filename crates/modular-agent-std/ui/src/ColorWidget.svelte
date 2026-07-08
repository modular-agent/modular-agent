<!--
  ConfigWidget for type_="color" (Rust: custom_config(type_ = "color")).

  A color is a genuine value type (a "#rrggbb" string, like the built-in
  "image" type is a data-URL string), so a type_-keyed ConfigWidget is
  appropriate here — unlike presentation-only variants of built-in types
  (e.g. a slider for an integer), which belong in a NodeView instead.

  Renders a color picker at the input position, or a swatch + hex label at
  the readonly display position. Writes back via updateConfig on "change".
-->
<script lang="ts">
  import type { ConfigWidgetProps } from "@modular-agent/widget-kit";

  let { configKey, value, readonly, updateConfig }: ConfigWidgetProps = $props();

  const HEX_RE = /^#[0-9a-fA-F]{6}$/;
  const hexValue = $derived(typeof value === "string" && HEX_RE.test(value) ? value : "#000000");
</script>

<div class="color-root" class:readonly>
  {#if readonly}
    <span class="color-swatch" style="background-color: {hexValue};"></span>
  {:else}
    <input
      type="color"
      class="nodrag color-input"
      value={hexValue}
      onchange={(evt) => {
        const v = evt.currentTarget.value;
        if (v !== value) {
          updateConfig(configKey, v);
        }
      }}
    />
  {/if}
  <span class="color-hex">{hexValue}</span>
</div>

<style>
  /* Plain scoped CSS only: the desktop's Tailwind does not scan external
     package sources, so utility classes would produce no CSS here. */
  .color-root {
    flex: none;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .color-root.readonly {
    padding-left: 0.75rem;
  }
  .color-swatch {
    width: 16px;
    height: 16px;
    border: 1px solid var(--border);
    border-radius: 3px;
  }
  .color-input {
    width: 40px;
    height: 24px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: none;
  }
  .color-hex {
    font-size: 0.875rem;
    line-height: 1.25rem;
    color: var(--muted-foreground);
    font-variant-numeric: tabular-nums;
  }
</style>
