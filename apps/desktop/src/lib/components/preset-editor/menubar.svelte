<script lang="ts">
  import * as Menubar from "$lib/components/ui/menubar/index.js";
  import { exitApp } from "$lib/modular_agent";

  type Props = {
    onShowNewDialog: () => void;
    onSavePreset: () => void;
    onShowSaveAsDialog: () => void;
    onImportPreset: () => void;
    onExportPreset: () => void;
  };

  let { onShowNewDialog, onSavePreset, onShowSaveAsDialog, onImportPreset, onExportPreset }: Props =
    $props();

  async function handleQuit() {
    await exitApp();
  }
</script>

<Menubar.Root class="border-none shadow-none bg-transparent">
  <Menubar.Menu>
    <Menubar.Trigger>File</Menubar.Trigger>
    <Menubar.Content>
      <Menubar.Item onclick={onShowNewDialog}>New</Menubar.Item>
      <Menubar.Item onclick={onSavePreset}>Save</Menubar.Item>
      <Menubar.Item onclick={onShowSaveAsDialog}>Save as...</Menubar.Item>
      <Menubar.Separator />
      <Menubar.Item onclick={onImportPreset}>Import</Menubar.Item>
      <Menubar.Item onclick={onExportPreset}>Export</Menubar.Item>
      <Menubar.Separator />
      <Menubar.Item onclick={handleQuit}>Quit</Menubar.Item>
    </Menubar.Content>
  </Menubar.Menu>
</Menubar.Root>
