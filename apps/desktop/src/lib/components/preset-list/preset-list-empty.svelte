<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";

  import ImportIcon from "@lucide/svelte/icons/import";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import WorkflowIcon from "@lucide/svelte/icons/workflow";

  import { goto } from "$app/navigation";

  import { importPreset, newPresetWithName } from "$lib/agent";
  import PresetActionDialog from "$lib/components/preset-action-dialog.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Empty from "$lib/components/ui/empty/index.js";
  import { tabStore } from "$lib/tab-store.svelte";

  let openNewPresetDialog = $state(false);

  async function onNewPreset(name: string) {
    const new_id = await newPresetWithName(name);
    if (new_id) {
      tabStore.openTab(new_id, name);
      goto(`/preset_editor/${new_id}`, { noScroll: true });
    }
  }

  async function handleImport() {
    const file = await open({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!file) return;

    const id = await importPreset(file as string, "");
    tabStore.openTab(id, id);
    goto(`/preset_editor/${id}`, { noScroll: true });
  }
</script>

<Empty.Root>
  <Empty.Header>
    <Empty.Media variant="icon">
      <WorkflowIcon />
    </Empty.Media>
    <Empty.Title>No presets yet</Empty.Title>
    <Empty.Description>
      Create a preset to start wiring agents together, or import one you already have.
    </Empty.Description>
  </Empty.Header>
  <Empty.Content class="flex-row justify-center">
    <Button onclick={() => (openNewPresetDialog = true)}>
      <PlusIcon />
      New Preset
    </Button>
    <Button variant="outline" onclick={handleImport}>
      <ImportIcon />
      Import
    </Button>
  </Empty.Content>
</Empty.Root>

<PresetActionDialog action="New" name="" onAction={onNewPreset} bind:open={openNewPresetDialog} />
