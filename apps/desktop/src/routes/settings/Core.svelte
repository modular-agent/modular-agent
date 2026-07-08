<script lang="ts">
  import { onMount } from "svelte";

  import { resetMode, setMode } from "mode-watcher";
  import { toast } from "svelte-sonner";

  import { getAgentDefinitions, getCoreSettings, setCoreSettings } from "$lib/agent";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { FieldGroup, Field, FieldLabel } from "$lib/components/ui/field/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { CORE_DEFAULTS } from "$lib/core-settings-store.svelte";
  import { DEFAULT_HOTKEYS, type HotkeyDefinition } from "$lib/hotkeys";
  import type { CoreSettings } from "$lib/types";

  interface Props {
    settings: Record<string, any>;
  }

  const { settings }: Props = $props();

  let autostart = $state(false);
  let color_mode = $state<string>("");
  let connection_opacity = $state(CORE_DEFAULTS.connectionOpacity * 100);
  let grid_gap = $state(CORE_DEFAULTS.gridGap);
  let max_history_length = $state(CORE_DEFAULTS.maxHistoryLength);
  let run_in_background = $state(false);
  let shortcut_keys = $state<Record<string, string>>({});
  let show_grid = $state(true);
  let snap_enabled = $state(true);
  let snap_grid_size = $state(CORE_DEFAULTS.snapGridSize);

  let initialGlobalShortcut = $state("");

  // Group hotkey definitions by group (exclude "Global" — rendered separately)
  const hotkeyGroups = $derived(
    DEFAULT_HOTKEYS.reduce(
      (acc, def) => {
        if (def.group === "Global") return acc;
        if (!acc[def.group]) acc[def.group] = [];
        acc[def.group].push(def);
        return acc;
      },
      {} as Record<string, HotkeyDefinition[]>,
    ),
  );

  // Global shortcut definition
  const globalShortcutDef = DEFAULT_HOTKEYS.find((d) => d.id === "global_shortcut")!;

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
    connection_opacity = Math.round(
      (settings["connection_opacity"] ?? CORE_DEFAULTS.connectionOpacity) * 100,
    );
    grid_gap = settings["grid_gap"] ?? CORE_DEFAULTS.gridGap;
    max_history_length = settings["max_history_length"] ?? CORE_DEFAULTS.maxHistoryLength;
    run_in_background = settings["run_in_background"] ?? false;
    show_grid = settings["show_grid"] ?? true;
    snap_enabled = settings["snap_enabled"] ?? true;
    snap_grid_size = settings["snap_grid_size"] ?? CORE_DEFAULTS.snapGridSize;

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
    initialGlobalShortcut = keys["global_shortcut"] ?? "";
  });

  // Auto-save helper (silent on success, toast on error)
  async function autoSave(partial: Partial<CoreSettings>) {
    try {
      await setCoreSettings(partial);
    } catch (e) {
      toast.error("Failed to save settings");
      console.error("Failed to save settings:", e);
    }
  }

  // Auto-save shortcut_keys, preserving persisted global_shortcut
  async function autoSaveShortcutKeys() {
    const persistedGlobal = getCoreSettings().shortcut_keys?.["global_shortcut"] ?? "";
    await autoSave({
      shortcut_keys: { ...shortcut_keys, global_shortcut: persistedGlobal },
    });
  }

  async function resetKey(def: HotkeyDefinition) {
    shortcut_keys[def.id] = def.defaultKey;
    if (def.defaultAgent) {
      shortcut_keys[`${def.id}.agent`] = def.defaultAgent;
    }
    await autoSaveShortcutKeys();
  }

  async function autoSaveGlobalShortcut() {
    await autoSave({ shortcut_keys });
  }

  async function resetGlobalShortcutKey() {
    shortcut_keys["global_shortcut"] = globalShortcutDef.defaultKey;
    await autoSaveGlobalShortcut();
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
    await autoSave({ color_mode });
  }

  function blurOnEnter(e: KeyboardEvent) {
    if (e.key === "Enter") (e.target as HTMLInputElement).blur();
  }
</script>

<Card.Root class="@container/card">
  <Card.Header>
    <Card.Title>Core</Card.Title>
    <Card.Description>Settings are saved automatically.</Card.Description>
  </Card.Header>
  <Card.Content class="px-2 pt-4">
    <FieldGroup>
      <Field orientation="horizontal">
        <Switch
          bind:checked={run_in_background}
          onCheckedChange={(v) => autoSave({ run_in_background: v })}
        />
        <FieldLabel>Run in Background</FieldLabel>
      </Field>

      <Field orientation="horizontal">
        <Switch bind:checked={autostart} onCheckedChange={(v) => autoSave({ autostart: v })} />
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
        <Switch
          bind:checked={snap_enabled}
          onCheckedChange={(v) => autoSave({ snap_enabled: v })}
        />
        <FieldLabel>Snap to Grid</FieldLabel>
      </Field>

      <Field orientation="horizontal">
        <Switch bind:checked={show_grid} onCheckedChange={(v) => autoSave({ show_grid: v })} />
        <FieldLabel>Show Grid</FieldLabel>
      </Field>

      <Field orientation="vertical">
        <FieldLabel>Snap Grid Size: {snap_grid_size}px</FieldLabel>
        <Input
          type="number"
          min={2}
          max={400}
          bind:value={snap_grid_size}
          onchange={() => autoSave({ snap_grid_size })}
          onkeydown={blurOnEnter}
          class="max-w-xs"
        />
      </Field>

      <Field orientation="vertical">
        <FieldLabel>Grid Gap: {grid_gap}px</FieldLabel>
        <Input
          type="number"
          min={4}
          max={400}
          bind:value={grid_gap}
          onchange={() => autoSave({ grid_gap })}
          onkeydown={blurOnEnter}
          class="max-w-xs"
        />
      </Field>

      <div class="font-semibold mt-4">Connections</div>

      <Field orientation="vertical">
        <FieldLabel>Connection Opacity: {connection_opacity}%</FieldLabel>
        <input
          type="range"
          min={10}
          max={100}
          step={5}
          bind:value={connection_opacity}
          onchange={() => autoSave({ connection_opacity: connection_opacity / 100 })}
          class="max-w-xs accent-primary"
        />
      </Field>

      <div class="font-semibold mt-4">Undo / Redo</div>

      <Field orientation="vertical">
        <FieldLabel>Max Undo History: {max_history_length}</FieldLabel>
        <Input
          type="number"
          min={10}
          max={10000}
          bind:value={max_history_length}
          onchange={() => autoSave({ max_history_length })}
          onkeydown={blurOnEnter}
          class="max-w-xs"
        />
      </Field>

      <div class="font-semibold mt-4">Global Shortcut</div>

      <Field orientation="vertical" class="gap-1">
        <div class="flex items-center gap-2">
          <FieldLabel class="min-w-[160px]">{globalShortcutDef.label}</FieldLabel>
          <Input
            type="text"
            value={shortcut_keys["global_shortcut"] ?? ""}
            oninput={(e: Event) => {
              shortcut_keys["global_shortcut"] = (e.target as HTMLInputElement).value;
            }}
            onchange={() => autoSaveGlobalShortcut()}
            onkeydown={blurOnEnter}
            class="max-w-[200px] h-8 text-sm"
            placeholder="(disabled)"
          />
          <Button
            type="button"
            variant="ghost"
            size="sm"
            class="h-8 px-2 text-xs"
            onclick={resetGlobalShortcutKey}
          >
            Reset
          </Button>
        </div>
        {#if shortcut_keys["global_shortcut"] !== initialGlobalShortcut}
          <p class="text-sm text-amber-600 dark:text-amber-400">
            Restart required to apply the new global shortcut.
          </p>
        {/if}
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
                onchange={() => autoSaveShortcutKeys()}
                onkeydown={blurOnEnter}
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
                    autoSaveShortcutKeys();
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
    </FieldGroup>
  </Card.Content>
</Card.Root>
