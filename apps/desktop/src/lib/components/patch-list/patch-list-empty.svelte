<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";

  import ImportIcon from "@lucide/svelte/icons/import";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import WorkflowIcon from "@lucide/svelte/icons/workflow";

  import { goto } from "$app/navigation";

  import { importPatch, newPatchWithName } from "$lib/agent";
  import PatchActionDialog from "$lib/components/patch-action-dialog.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Empty from "$lib/components/ui/empty/index.js";
  import { tabStore } from "$lib/tab-store.svelte";

  let openNewPatchDialog = $state(false);

  async function onNewPatch(name: string) {
    const new_id = await newPatchWithName(name);
    if (new_id) {
      tabStore.openTab(new_id, name);
      goto(`/patch_editor/${new_id}`, { noScroll: true });
    }
  }

  async function handleImport() {
    const file = await open({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!file) return;

    const id = await importPatch(file as string, "");
    tabStore.openTab(id, id);
    goto(`/patch_editor/${id}`, { noScroll: true });
  }
</script>

<Empty.Root>
  <Empty.Header>
    <Empty.Media variant="icon">
      <WorkflowIcon />
    </Empty.Media>
    <Empty.Title>No patches yet</Empty.Title>
    <Empty.Description>
      Create a patch to start wiring agents together, or import one you already have.
    </Empty.Description>
  </Empty.Header>
  <Empty.Content class="flex-row justify-center">
    <Button onclick={() => (openNewPatchDialog = true)}>
      <PlusIcon />
      New Patch
    </Button>
    <Button variant="outline" onclick={handleImport}>
      <ImportIcon />
      Import
    </Button>
  </Empty.Content>
</Empty.Root>

<PatchActionDialog action="New" name="" onAction={onNewPatch} bind:open={openNewPatchDialog} />
