<script lang="ts">
  import { addAgentStream, getAgentStreamSpec } from "tauri-plugin-askit-api";

  import { goto } from "$app/navigation";

  import { Button } from "$lib/components/ui/button/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Menubar from "$lib/components/ui/menubar/index.js";

  import NewStreamDialog from "@/lib/components/new-stream-dialog.svelte";
  import { reloadStreamInfos, startStream } from "@/lib/shared.svelte";
  import { exitApp } from "@/lib/utils";

  type Props = {
    stream_id: string;
    name: string;
    onNewStream: (name: string) => void;
    onSaveStream: () => void;
    onImportStream: () => void;
    onExportStream: () => void;
  };

  let { stream_id, name, onNewStream, onSaveStream, onImportStream, onExportStream }: Props =
    $props();

  let openNewStreamDialog = $state(false);

  function handleNewStream() {
    openNewStreamDialog = true;
  }

  let openSaveAsDialog = $state(false);
  let new_name = $state("");

  async function handleSaveAsStream(e: Event) {
    new_name = name;
    openSaveAsDialog = true;
  }

  async function handleSaveAsSubmit(e: Event) {
    openSaveAsDialog = false;
    if (!new_name) return;
    if (new_name === name) {
      // same name, just save
      onSaveStream();
      return;
    }
    const s = await getAgentStreamSpec(stream_id);
    if (!s) return;
    const new_id = await addAgentStream(new_name, s);
    if (!new_id) return;
    await reloadStreamInfos();
    goto(`/stream_editor/${new_id}`, { invalidateAll: true });
  }

  async function handleQuit() {
    await exitApp();
  }
</script>

<Menubar.Root class="border-none shadow-none bg-transparent">
  <Menubar.Menu>
    <Menubar.Trigger>File</Menubar.Trigger>
    <Menubar.Content>
      <Menubar.Item onclick={handleNewStream}>New</Menubar.Item>
      <Menubar.Item onclick={onSaveStream}>Save</Menubar.Item>
      <Menubar.Item onclick={handleSaveAsStream}>Save as...</Menubar.Item>
      <Menubar.Separator />
      <Menubar.Item onclick={onImportStream}>Import</Menubar.Item>
      <Menubar.Item onclick={onExportStream}>Export</Menubar.Item>
      <Menubar.Separator />
      <Menubar.Item onclick={handleQuit}>Quit</Menubar.Item>
    </Menubar.Content>
  </Menubar.Menu>
</Menubar.Root>

<NewStreamDialog {onNewStream} bind:open={openNewStreamDialog} />

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
