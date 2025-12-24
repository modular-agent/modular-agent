<script lang="ts">
  import * as Menubar from "$lib/components/ui/menubar/index.js";

  let { onImportStream, onExportStream } = $props();
</script>

<Menubar.Root>
  <Menubar.Menu>
    <Menubar.Trigger>File</Menubar.Trigger>
    <Menubar.Content>
      <Menubar.Item onclick={onImportStream}>Import</Menubar.Item>
      <Menubar.Item onclick={onExportStream}>Export</Menubar.Item>
    </Menubar.Content>
  </Menubar.Menu>
</Menubar.Root>
