<script lang="ts">
  import * as Menubar from "$lib/components/ui/menubar/index.js";

  import NewStreamDialog from "@/lib/components/new-stream-dialog.svelte";

  type Props = {
    onNewStream: (name: string) => void;
    onImportStream: () => void;
    onExportStream: () => void;
  };

  let { onNewStream, onImportStream, onExportStream }: Props = $props();

  let newStreamDialogOpen = $state(false);

  function handleNewStream() {
    newStreamDialogOpen = true;
  }
</script>

<Menubar.Root class="border-none shadow-none">
  <Menubar.Menu>
    <Menubar.Trigger class="focus:bg-background data-[state=open]:bg-background"
      >File</Menubar.Trigger
    >
    <Menubar.Content>
      <Menubar.Item onclick={handleNewStream}>New</Menubar.Item>
      <Menubar.Item onclick={onImportStream}>Import</Menubar.Item>
      <Menubar.Item onclick={onExportStream}>Export</Menubar.Item>
    </Menubar.Content>
  </Menubar.Menu>
</Menubar.Root>

<NewStreamDialog {onNewStream} bind:open={newStreamDialogOpen} />
