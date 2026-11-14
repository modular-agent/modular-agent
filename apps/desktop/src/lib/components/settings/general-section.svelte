<script lang="ts">
  import { resetMode, setMode } from "mode-watcher";

  import * as Select from "$lib/components/ui/select/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { getCoreSettings } from "$lib/module";

  import { saveCoreSettings } from "./helpers";
  import SettingsGroup from "./settings-group.svelte";
  import SettingsRow from "./settings-row.svelte";

  const settings = getCoreSettings();

  let autostart = $state(settings.autostart ?? false);
  let color_mode = $state(settings.color_mode ?? "");
  let run_in_background = $state(settings.run_in_background ?? false);

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
    await saveCoreSettings({ color_mode });
  }
</script>

<div class="space-y-8">
  <SettingsGroup title="Startup">
    <SettingsRow label="Run in Background">
      {#snippet children(labelId)}
        <Switch
          aria-labelledby={labelId}
          bind:checked={run_in_background}
          onCheckedChange={(v) => saveCoreSettings({ run_in_background: v })}
        />
      {/snippet}
    </SettingsRow>
    <SettingsRow label="Auto Start on System Boot">
      {#snippet children(labelId)}
        <Switch
          aria-labelledby={labelId}
          bind:checked={autostart}
          onCheckedChange={(v) => saveCoreSettings({ autostart: v })}
        />
      {/snippet}
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup title="Appearance">
    <SettingsRow label="Color Mode">
      {#snippet children(labelId)}
        <Select.Root type="single" value={color_mode} onValueChange={setColorMode}>
          <Select.Trigger aria-labelledby={labelId} class="min-w-40 bg-background">
            {color_mode === "light" ? "Light" : color_mode === "dark" ? "Dark" : "System Default"}
          </Select.Trigger>
          <Select.Content>
            <Select.Item value="">System Default</Select.Item>
            <Select.Item value="light">Light</Select.Item>
            <Select.Item value="dark">Dark</Select.Item>
          </Select.Content>
        </Select.Root>
      {/snippet}
    </SettingsRow>
  </SettingsGroup>
</div>
