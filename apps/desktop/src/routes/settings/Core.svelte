<script lang="ts">
  import { message } from "@tauri-apps/plugin-dialog";

  import { Button, Input, Label, Toggle } from "flowbite-svelte";

  import Card from "@/components/Card.svelte";
  import { exitApp, setCoreSettings } from "@/lib/utils";

  interface Props {
    settings: Record<string, any>;
  }

  const { settings }: Props = $props();

  let autostart = $state(settings["autostart"]);
  let shortcut_keys = $state(settings["shortcut_keys"]);

  async function saveSettings() {
    await setCoreSettings({
      autostart,
      shortcut_keys,
    });
    // confirm restart
    await message("Agent Stream App will quit to apply changes.\n\nPlease restart.");
    await exitApp();
  }
</script>

<Card title="Core">
  <form class="grid grid-cols-6 gap-6">
    <Toggle bind:checked={autostart}>Auto Start</Toggle>

    <div class="col-span-6">
      <h3 class="text-lg font-semibold">Shortcut Keys</h3>
    </div>

    <Label class="col-span-2 space-y-2">
      <span>Global Shortcut</span>
    </Label>
    <Input class="col-span-4" type="text" bind:value={shortcut_keys["global_shortcut"]} />

    <Label class="col-span-2 space-y-2">
      <span>Fullscreen</span>
    </Label>
    <Input class="col-span-4" type="text" bind:value={shortcut_keys["fullscreen"]} />

    <Button onclick={saveSettings} class="w-fit" outline>Save</Button>
  </form>
</Card>
