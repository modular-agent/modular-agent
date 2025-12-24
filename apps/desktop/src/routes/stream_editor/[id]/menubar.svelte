<script lang="ts">
  import * as Menubar from "$lib/components/ui/menubar/index.js";

  let { onImportStream, onExportStream } = $props();
</script>

<Menubar.Root class="border-none shadow-none">
  <Menubar.Menu>
    <Menubar.Trigger class="focus:bg-background data-[state=open]:bg-background"
      >File</Menubar.Trigger
    >
    <Menubar.Content>
      <Menubar.Item onclick={onImportStream}>Import</Menubar.Item>
      <Menubar.Item onclick={onExportStream}>Export</Menubar.Item>
    </Menubar.Content>
  </Menubar.Menu>
</Menubar.Root>
