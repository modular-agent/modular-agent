<script lang="ts">
  import { Input } from "$lib/components/ui/input/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { CORE_DEFAULTS } from "$lib/core-settings-store.svelte";
  import { getCoreSettings } from "$lib/module";

  import { blurOnEnter, saveCoreSettings } from "./helpers";
  import SettingsGroup from "./settings-group.svelte";
  import SettingsRow from "./settings-row.svelte";

  const settings = getCoreSettings();

  let connection_opacity = $state(
    Math.round((settings.connection_opacity ?? CORE_DEFAULTS.connectionOpacity) * 100),
  );
  let grid_gap = $state(settings.grid_gap ?? CORE_DEFAULTS.gridGap);
  let max_history_length = $state(settings.max_history_length ?? CORE_DEFAULTS.maxHistoryLength);
  let snap_enabled = $state(settings.snap_enabled ?? true);
  let snap_grid_size = $state(settings.snap_grid_size ?? CORE_DEFAULTS.snapGridSize);
</script>

<div class="space-y-8">
  <SettingsGroup title="Grid / Snap">
    <SettingsRow label="Snap to Grid">
      {#snippet children(labelId)}
        <Switch
          aria-labelledby={labelId}
          bind:checked={snap_enabled}
          onCheckedChange={(v) => saveCoreSettings({ snap_enabled: v })}
        />
      {/snippet}
    </SettingsRow>
    <SettingsRow label="Snap Grid Size" description="2–400 px">
      {#snippet children(labelId)}
        <Input
          aria-labelledby={labelId}
          type="number"
          min={2}
          max={400}
          bind:value={snap_grid_size}
          onchange={() => saveCoreSettings({ snap_grid_size })}
          onkeydown={blurOnEnter}
          class="w-24"
        />
      {/snippet}
    </SettingsRow>
    <SettingsRow label="Grid Gap" description="4–400 px">
      {#snippet children(labelId)}
        <Input
          aria-labelledby={labelId}
          type="number"
          min={4}
          max={400}
          bind:value={grid_gap}
          onchange={() => saveCoreSettings({ grid_gap })}
          onkeydown={blurOnEnter}
          class="w-24"
        />
      {/snippet}
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup title="Connections">
    <SettingsRow label="Connection Opacity">
      {#snippet children(labelId)}
        <input
          aria-labelledby={labelId}
          type="range"
          min={10}
          max={100}
          step={5}
          bind:value={connection_opacity}
          onchange={() => saveCoreSettings({ connection_opacity: connection_opacity / 100 })}
          class="w-48 accent-primary"
        />
        <span class="w-12 text-right text-sm tabular-nums">{connection_opacity}%</span>
      {/snippet}
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup title="Undo / Redo">
    <SettingsRow label="Max Undo History" description="10–10000 steps">
      {#snippet children(labelId)}
        <Input
          aria-labelledby={labelId}
          type="number"
          min={10}
          max={10000}
          bind:value={max_history_length}
          onchange={() => saveCoreSettings({ max_history_length })}
          onkeydown={blurOnEnter}
          class="w-24"
        />
      {/snippet}
    </SettingsRow>
  </SettingsGroup>
</div>
