<script lang="ts">
  import EllipsisVerticalIcon from "@lucide/svelte/icons/ellipsis-vertical";
  import PlayIcon from "@lucide/svelte/icons/play";
  import SquareIcon from "@lucide/svelte/icons/square";
  import { addPreset, getPresetSpec } from "tauri-plugin-mak-api";

  import { getCoreSettings, setCoreSettings } from "$lib/agent";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import {
    deletePresetAndReload,
    reloadPreseetInfos,
    renamePresetAndReload,
    startPresetAndReload,
    stopPresetAndReload,
    presetInfos,
  } from "$lib/shared.svelte";

  type Props = {
    id: string;
    name: string;
    run_on_start?: boolean | undefined;
  };

  let { id, name, run_on_start }: Props = $props();

  // start and stop

  let running = $derived(presetInfos.get(id)?.running);

  async function handleStart() {
    await startPresetAndReload(id);
  }

  async function handleStop() {
    await stopPresetAndReload(id);
  }

  // rename and delete

  let new_name = $state("");
  let openRenameDialog = $state(false);
  let openDeleteDialog = $state(false);

  async function handleRenamePreset(e: Event) {
    e.preventDefault();
    new_name = name;
    openRenameDialog = true;
  }

  async function handleRenamePresetSubmit(e: Event) {
    e.preventDefault();
    if (!new_name) return;
    await renamePresetAndReload(id, new_name);
    openRenameDialog = false;
    await reloadPreseetInfos();
  }

  async function handleDuplicatePreset(e: Event) {
    e.preventDefault();
    const s = await getPresetSpec(id);
    if (!s) return;
    const new_name = await uniquePresetName(name);
    const new_id = await addPreset(new_name, s);
    if (!new_id) return;
    await reloadPreseetInfos();
  }

  async function handleDeletePresetSubmit(e: Event) {
    e.preventDefault();
    await deletePresetAndReload(id);
    openDeleteDialog = false;
  }

  async function handleRunOnStart() {
    // update core setting
    const coreSettings = await getCoreSettings();
    const auto_start_presets = coreSettings.auto_start_presets || [];
    if (run_on_start) {
      // remove from auto_start_presets
      const index = auto_start_presets.indexOf(name);
      if (index > -1) {
        auto_start_presets.splice(index, 1);
      }
    } else {
      // add to auto_start_presets
      auto_start_presets.push(name);
    }
    coreSettings.auto_start_presets = auto_start_presets;
    await setCoreSettings(coreSettings);

    await reloadPreseetInfos();
  }
</script>

<div class="flex items-center justify-end gap-2">
  {#if running}
    <Button onclick={handleStop} variant="ghost" class="w-4">
      <SquareIcon color="red" />
    </Button>
  {:else}
    <Button onclick={handleStart} variant="ghost" class="w-4">
      <PlayIcon color="blue" />
    </Button>
  {/if}

  <DropdownMenu.Root>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}
        <Button {...props} variant="ghost" size="icon" class="relative size-8 p-0">
          <span class="sr-only">Open menu</span>
          <EllipsisVerticalIcon />
        </Button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content>
      <DropdownMenu.Item onclick={handleRenamePreset}>Rename</DropdownMenu.Item>
      <DropdownMenu.Item onclick={handleDuplicatePreset}>Duplicate</DropdownMenu.Item>
      <DropdownMenu.Item onclick={() => (openDeleteDialog = true)}>Delete</DropdownMenu.Item>
      <DropdownMenu.Separator />
      <DropdownMenu.Item onclick={handleRunOnStart}>Run on Start</DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
</div>

<Dialog.Root bind:open={openRenameDialog}>
  <Dialog.Content class="sm:max-w-[425px]">
    <form onsubmit={handleRenamePresetSubmit}>
      <Dialog.Header>
        <Dialog.Title>Rename Preset</Dialog.Title>
      </Dialog.Header>
      <div class="mt-4 mb-4">
        <Input id="name-1" name="name" bind:value={new_name} />
      </div>
      <Dialog.Footer>
        <Button type="submit">Rename</Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={openDeleteDialog}>
  <Dialog.Content class="sm:max-w-[425px]">
    <form onsubmit={handleDeletePresetSubmit}>
      <Dialog.Header>
        <Dialog.Title>Delete Preset</Dialog.Title>
        <Dialog.Description
          ><div class="mt-8 text-center text-lg">
            <strong>{name}</strong>
          </div></Dialog.Description
        >
      </Dialog.Header>
      <Dialog.Footer>
        <Button type="submit">Delete</Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
