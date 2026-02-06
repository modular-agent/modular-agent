<script lang="ts">
  import { addPresetWithName, getPresetSpec } from "tauri-plugin-modular-agent-api";

  import { goto } from "$app/navigation";

  import { Button } from "$lib/components/ui/button/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Menubar from "$lib/components/ui/menubar/index.js";
  import { exitApp } from "$lib/modular_agent";

  import PresetActionDialog from "$lib/components/preset-action-dialog.svelte";

  type Props = {
    preset_id: string;
    name: string;
    onNewPreset: (name: string) => void;
    onSavePreset: () => void;
    onImportPreset: () => void;
    onExportPreset: () => void;
  };

  let { preset_id, name, onNewPreset, onSavePreset, onImportPreset, onExportPreset }: Props =
    $props();

  let openNewPresetDialog = $state(false);

  function handleNewPreset() {
    openNewPresetDialog = true;
  }

  async function handleSavePreset(e: Event) {
    await onSavePreset();
  }

  let openSaveAsDialog = $state(false);
  let new_name = $state("");

  async function handleSaveAsPreset(e: Event) {
    new_name = name;
    openSaveAsDialog = true;
  }

  async function handleSaveAsSubmit(e: Event) {
    openSaveAsDialog = false;
    if (!new_name) return;
    if (new_name === name) {
      // same name, just save
      await onSavePreset();
      return;
    }
    const s = await getPresetSpec(preset_id);
    if (!s) return;
    const new_id = await addPresetWithName(s, new_name);
    if (!new_id) return;
    goto(`/preset_editor/${new_id}`, { invalidateAll: true });
  }

  async function handleQuit() {
    await exitApp();
  }
</script>

<Menubar.Root class="border-none shadow-none bg-transparent">
  <Menubar.Menu>
    <Menubar.Trigger>File</Menubar.Trigger>
    <Menubar.Content>
      <Menubar.Item onclick={handleNewPreset}>New</Menubar.Item>
      <Menubar.Item onclick={handleSavePreset}>Save</Menubar.Item>
      <Menubar.Item onclick={handleSaveAsPreset}>Save as...</Menubar.Item>
      <Menubar.Separator />
      <Menubar.Item onclick={onImportPreset}>Import</Menubar.Item>
      <Menubar.Item onclick={onExportPreset}>Export</Menubar.Item>
      <Menubar.Separator />
      <Menubar.Item onclick={handleQuit}>Quit</Menubar.Item>
    </Menubar.Content>
  </Menubar.Menu>
</Menubar.Root>

<PresetActionDialog action="New" {name} onAction={onNewPreset} bind:open={openNewPresetDialog} />

<Dialog.Root bind:open={openSaveAsDialog}>
  <Dialog.Content class="sm:max-w-[425px]">
    <form onsubmit={handleSaveAsSubmit}>
      <Dialog.Header>
        <Dialog.Title>Save As...</Dialog.Title>
      </Dialog.Header>
      <div class="mt-4 mb-4">
        <Input id="name-1" name="name" bind:value={new_name} />
      </div>
      <Dialog.Footer>
        <Button type="submit">Save</Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
