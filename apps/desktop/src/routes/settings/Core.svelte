<script lang="ts">
  import { message } from "@tauri-apps/plugin-dialog";

  import { onMount } from "svelte";

  import { resetMode, setMode } from "mode-watcher";

  import { getAgentDefinitions } from "$lib/agent";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { FieldGroup, Field, FieldLabel } from "$lib/components/ui/field/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { DEFAULT_HOTKEYS, type HotkeyDefinition } from "$lib/hotkeys";
  import { exitApp, setCoreSettings } from "$lib/modular_agent";

  interface Props {
    settings: Record<string, any>;
  }

  const { settings }: Props = $props();

  let autostart = $state(false);
  let color_mode = $state<string>("");
  let run_in_background = $state(false);
  let shortcut_keys = $state<Record<string, string>>({});
  let snap_enabled = $state(true);
  let snap_grid_size = $state(12);
  let show_grid = $state(true);
  let grid_gap = $state(24);
  let max_history_length = $state(2000);

  // Group hotkey definitions by group
  const hotkeyGroups = $derived(
    DEFAULT_HOTKEYS.reduce(
      (acc, def) => {
        if (!acc[def.group]) acc[def.group] = [];
        acc[def.group].push(def);
        return acc;
      },
      {} as Record<string, HotkeyDefinition[]>,
    ),
  );

  // Agent definitions for Quick Add selector
  const agentDefs = getAgentDefinitions();
  const agentOptions = $derived(
    Object.entries(agentDefs)
      .map(([defName, def]) => ({
        value: defName,
        label: def.title ?? def.name ?? defName,
      }))
      .sort((a, b) => a.label.localeCompare(b.label)),
  );

  function getAgentLabel(defName: string): string {
    const def = agentDefs[defName];
    return def?.title ?? def?.name ?? defName;
  }

  onMount(() => {
    autostart = settings["autostart"] ?? false;
    color_mode = settings["color_mode"] ?? "";
    run_in_background = settings["run_in_background"] ?? false;
    snap_enabled = settings["snap_enabled"] ?? true;
    snap_grid_size = settings["snap_grid_size"] ?? 12;
    show_grid = settings["show_grid"] ?? true;
    grid_gap = settings["grid_gap"] ?? 24;
    max_history_length = settings["max_history_length"] ?? 2000;

    // Initialize shortcut_keys with all defaults, then overlay user overrides
    const userKeys = settings["shortcut_keys"] ?? {};
    const keys: Record<string, string> = {};
    for (const def of DEFAULT_HOTKEYS) {
      keys[def.id] = userKeys[def.id] ?? def.defaultKey;
      if (def.defaultAgent) {
        keys[`${def.id}.agent`] = userKeys[`${def.id}.agent`] ?? def.defaultAgent;
      }
    }
    shortcut_keys = keys;
  });

  function resetKey(def: HotkeyDefinition) {
    shortcut_keys[def.id] = def.defaultKey;
    if (def.defaultAgent) {
      shortcut_keys[`${def.id}.agent`] = def.defaultAgent;
    }
  }

  async function setColorMode(mode: string) {
    if (mode === "light") {
      color_mode = "light";
      setMode("light");
    } else if (mode === "dark") {
      color_mode = "dark";
      setMode("dark");
    } else {
      color_mode = "";
      resetMode();
    }
  }

  async function saveSettings() {
    await setCoreSettings({
      autostart,
      color_mode,
      run_in_background,
      shortcut_keys,
      snap_enabled,
      snap_grid_size,
      show_grid,
      grid_gap,
      max_history_length,
    });
    // confirm restart
    await message("Modular Agent will quit to apply changes.\n\nPlease restart.");
    await exitApp();
  }
</script>

<Card.Root class="@container/card">
  <Card.Header>
    <Card.Title>Core</Card.Title>
  </Card.Header>
  <Card.Content class="px-2 pt-4">
    <form>
      <FieldGroup>
        <Field orientation="horizontal">
          <Switch bind:checked={run_in_background} />
          <FieldLabel>Run in Background</FieldLabel>
        </Field>

        <Field orientation="horizontal">
          <Switch bind:checked={autostart} />
          <FieldLabel>Auto Start on System Boot</FieldLabel>
        </Field>

        <Field orientation="vertical">
          <FieldLabel>Color Mode</FieldLabel>
          <Select.Root type="single" value={color_mode} onValueChange={setColorMode}>
            <Select.Trigger class="max-w-xs">
              {color_mode === "light" ? "Light" : color_mode === "dark" ? "Dark" : "System Default"}
            </Select.Trigger>
            <Select.Content>
              <Select.Item value="">System Default</Select.Item>
              <Select.Item value="light">Light</Select.Item>
              <Select.Item value="dark">Dark</Select.Item>
            </Select.Content>
          </Select.Root>
        </Field>

        <div class="font-semibold mt-4">Grid / Snap</div>

        <Field orientation="horizontal">
          <Switch bind:checked={snap_enabled} />
          <FieldLabel>Snap to Grid</FieldLabel>
        </Field>

        <Field orientation="horizontal">
          <Switch bind:checked={show_grid} />
          <FieldLabel>Show Grid</FieldLabel>
        </Field>

        <Field orientation="vertical">
          <FieldLabel>Snap Grid Size: {snap_grid_size}px</FieldLabel>
          <Input type="number" min={2} max={48} bind:value={snap_grid_size} class="max-w-xs" />
        </Field>

        <Field orientation="vertical">
          <FieldLabel>Grid Gap: {grid_gap}px</FieldLabel>
          <Input type="number" min={4} max={96} bind:value={grid_gap} class="max-w-xs" />
        </Field>

        <div class="font-semibold mt-4">Undo / Redo</div>

        <Field orientation="vertical">
          <FieldLabel>Max Undo History: {max_history_length}</FieldLabel>
          <Input type="number" min={10} max={10000} bind:value={max_history_length} class="max-w-xs" />
        </Field>

        {#each Object.entries(hotkeyGroups) as [group, defs]}
          <div class="font-semibold mt-4">{group}</div>

          {#each defs as def}
            <Field orientation="vertical" class="gap-1">
              <div class="flex items-center gap-2">
                <FieldLabel class="min-w-[160px]">{def.label}</FieldLabel>
                <Input
                  type="text"
                  value={shortcut_keys[def.id] ?? ""}
                  oninput={(e: Event) => {
                    shortcut_keys[def.id] = (e.target as HTMLInputElement).value;
                  }}
                  class="max-w-[200px] h-8 text-sm"
                  placeholder="(disabled)"
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  class="h-8 px-2 text-xs"
                  onclick={() => resetKey(def)}
                >
                  Reset
                </Button>
              </div>
              {#if def.defaultAgent}
                <div class="flex items-center gap-2 ml-[168px]">
                  <Select.Root
                    type="single"
                    value={shortcut_keys[`${def.id}.agent`] ?? def.defaultAgent}
                    onValueChange={(v) => {
                      shortcut_keys[`${def.id}.agent`] = v;
                    }}
                  >
                    <Select.Trigger class="max-w-xs h-8 text-sm">
                      {getAgentLabel(shortcut_keys[`${def.id}.agent`] ?? def.defaultAgent ?? "")}
                    </Select.Trigger>
                    <Select.Content class="max-h-60">
                      {#each agentOptions as opt}
                        <Select.Item value={opt.value}>{opt.label}</Select.Item>
                      {/each}
                    </Select.Content>
                  </Select.Root>
                </div>
              {/if}
            </Field>
          {/each}
        {/each}

        <Field orientation="responsive" class="mt-4">
          <Button type="submit" onclick={saveSettings} variant="outline">Save</Button>
        </Field>
      </FieldGroup>
    </form>
  </Card.Content>
</Card.Root>
