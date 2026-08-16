<script lang="ts">
  import * as Menubar from "$lib/components/ui/menubar/index.js";
  import { exitApp } from "$lib/modular_agent";

  type Props = {
    onShowNewDialog: () => void;
    onSavePatch: () => void;
    onShowSaveAsDialog: () => void;
    onImportPatch: () => void;
    onExportPatch: () => void;
  };

  let { onShowNewDialog, onSavePatch, onShowSaveAsDialog, onImportPatch, onExportPatch }: Props =
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
      <Menubar.Item onclick={onSavePatch}>Save</Menubar.Item>
      <Menubar.Item onclick={onShowSaveAsDialog}>Save as...</Menubar.Item>
      <Menubar.Separator />
      <Menubar.Item onclick={onImportPatch}>Import</Menubar.Item>
      <Menubar.Item onclick={onExportPatch}>Export</Menubar.Item>
      <Menubar.Separator />
      <Menubar.Item onclick={handleQuit}>Quit</Menubar.Item>
    </Menubar.Content>
  </Menubar.Menu>
</Menubar.Root>
