<script lang="ts">
  import { message } from "@tauri-apps/plugin-dialog";

  import { onMount } from "svelte";

  import { resetMode, setMode } from "mode-watcher";

  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { FieldGroup, Field, FieldLabel } from "$lib/components/ui/field/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";

  import { exitApp, setCoreSettings } from "@/lib/utils";

  interface Props {
    settings: Record<string, any>;
  }

  const { settings }: Props = $props();

  let autostart = $state(false);
  let color_mode = $state<string>("");
  let run_in_background = $state(false);
  let shortcut_keys = $state<Record<string, string>>({});

  onMount(() => {
    autostart = settings["autostart"] ?? false;
    color_mode = settings["color_mode"] ?? "";
    run_in_background = settings["run_in_background"] ?? false;
    shortcut_keys = settings["shortcut_keys"] ?? {};
  });

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
    });
    // confirm restart
    await message("Agent Stream App will quit to apply changes.\n\nPlease restart.");
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

        <div class="font-semibold mt-4">Shortcut Keys</div>

        {#each Object.entries(shortcut_keys) as [key, _]}
          <Field orientation="horizontal" class="grid gap-4 sm:grid-cols-[220px_1fr] items-center">
            <FieldLabel>
              {key}
            </FieldLabel>
            <Input type="text" bind:value={shortcut_keys[key]} />
          </Field>
        {/each}

        <Field orientation="responsive" class="mt-4">
          <Button type="submit" onclick={saveSettings} variant="outline">Save</Button>
        </Field>
      </FieldGroup>
    </form>
  </Card.Content>
</Card.Root>
