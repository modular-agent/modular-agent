<script module lang="ts">
  // The global shortcut is registered only at startup, so the restart notice
  // compares against the value this process started with — captured the first
  // time the section mounts, not on each reopening of the dialog.
  let startupGlobalShortcut: string | undefined;
</script>

<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { DEFAULT_HOTKEYS, type HotkeyDefinition } from "$lib/hotkeys";
  import { getCoreSettings, getModuleDefinitions } from "$lib/module";

  import { blurOnEnter, saveCoreSettings } from "./helpers";
  import SettingsGroup from "./settings-group.svelte";
  import SettingsNote from "./settings-note.svelte";
  import SettingsRow from "./settings-row.svelte";

  // All defaults, overlaid with the user's overrides
  const userKeys = getCoreSettings().shortcut_keys ?? {};
  const initialKeys: Record<string, string> = {};
  for (const def of DEFAULT_HOTKEYS) {
    initialKeys[def.id] = userKeys[def.id] ?? def.defaultKey;
    if (def.defaultModule) {
      initialKeys[`${def.id}.module`] = userKeys[`${def.id}.module`] ?? def.defaultModule;
    }
  }
  const appliedGlobalShortcut = (startupGlobalShortcut ??= initialKeys["global_shortcut"] ?? "");

  let shortcut_keys = $state(initialKeys);

  const globalShortcutDef = DEFAULT_HOTKEYS.find((d) => d.id === "global_shortcut")!;

  // "Global" is rendered separately
  const hotkeyGroups = DEFAULT_HOTKEYS.reduce(
    (acc, def) => {
      if (def.group === "Global") return acc;
      (acc[def.group] ??= []).push(def);
      return acc;
    },
    {} as Record<string, HotkeyDefinition[]>,
  );

  // Module definitions for the Quick Add selector
  const moduleDefs = getModuleDefinitions();
  const moduleOptions = Object.entries(moduleDefs)
    .map(([defName, def]) => ({
      value: defName,
      label: def.title ?? def.name ?? defName,
    }))
    .sort((a, b) => a.label.localeCompare(b.label));

  function getModuleLabel(defName: string): string {
    const def = moduleDefs[defName];
    return def?.title ?? def?.name ?? defName;
  }

  // Saves every key except global_shortcut, which keeps its persisted value
  async function saveShortcutKeys() {
    const persistedGlobal = getCoreSettings().shortcut_keys?.["global_shortcut"] ?? "";
    await saveCoreSettings({
      shortcut_keys: { ...shortcut_keys, global_shortcut: persistedGlobal },
    });
  }

  async function resetKey(def: HotkeyDefinition) {
    shortcut_keys[def.id] = def.defaultKey;
    if (def.defaultModule) {
      shortcut_keys[`${def.id}.module`] = def.defaultModule;
    }
    await saveShortcutKeys();
  }

  async function saveGlobalShortcut() {
    await saveCoreSettings({ shortcut_keys });
  }

  async function resetGlobalShortcut() {
    shortcut_keys["global_shortcut"] = globalShortcutDef.defaultKey;
    await saveGlobalShortcut();
  }
</script>

{#snippet keyControl(def: HotkeyDefinition, labelId: string, save: () => void, reset: () => void)}
  <Input
    aria-labelledby={labelId}
    type="text"
    value={shortcut_keys[def.id] ?? ""}
    oninput={(e: Event) => {
      shortcut_keys[def.id] = (e.target as HTMLInputElement).value;
    }}
    onchange={save}
    onkeydown={blurOnEnter}
    class="h-8 w-36"
    placeholder="(disabled)"
  />
  <Button type="button" variant="ghost" size="sm" class="h-8 px-2 text-xs" onclick={reset}>
    Reset
  </Button>
{/snippet}

<div class="space-y-8">
  <div>
    <SettingsGroup title="Global">
      <SettingsRow label={globalShortcutDef.label}>
        {#snippet children(labelId)}
          {@render keyControl(globalShortcutDef, labelId, saveGlobalShortcut, resetGlobalShortcut)}
        {/snippet}
      </SettingsRow>
    </SettingsGroup>
    {#if shortcut_keys["global_shortcut"] !== appliedGlobalShortcut}
      <SettingsNote>Restart required to apply the new global shortcut.</SettingsNote>
    {/if}
  </div>

  {#each Object.entries(hotkeyGroups) as [group, defs] (group)}
    <SettingsGroup title={group}>
      {#each defs as def (def.id)}
        <SettingsRow label={def.label}>
          {#snippet children(labelId)}
            {#if def.defaultModule}
              {@const moduleKey = `${def.id}.module`}
              <Select.Root
                type="single"
                value={shortcut_keys[moduleKey] ?? def.defaultModule}
                onValueChange={(v) => {
                  shortcut_keys[moduleKey] = v;
                  saveShortcutKeys();
                }}
              >
                <Select.Trigger
                  size="sm"
                  aria-label={`${def.label} module`}
                  class="w-48 bg-background"
                >
                  <span class="truncate">
                    {getModuleLabel(shortcut_keys[moduleKey] ?? def.defaultModule)}
                  </span>
                </Select.Trigger>
                <Select.Content class="max-h-60">
                  {#each moduleOptions as opt (opt.value)}
                    <Select.Item value={opt.value}>{opt.label}</Select.Item>
                  {/each}
                </Select.Content>
              </Select.Root>
            {/if}
            {@render keyControl(def, labelId, saveShortcutKeys, () => resetKey(def))}
          {/snippet}
        </SettingsRow>
      {/each}
    </SettingsGroup>
  {/each}
</div>
