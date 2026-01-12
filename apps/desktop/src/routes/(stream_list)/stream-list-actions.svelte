<script lang="ts">
  import EllipsisVerticalIcon from "@lucide/svelte/icons/ellipsis-vertical";
  import PlayIcon from "@lucide/svelte/icons/play";
  import SquareIcon from "@lucide/svelte/icons/square";
  import { addAgentStream, getAgentStreamSpec, uniqueStreamName } from "tauri-plugin-askit-api";

  import { Button } from "$lib/components/ui/button/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import {
    deleteStream,
    reloadStreamInfos,
    renameStream,
    startStream,
    stopStream,
    streamInfos,
  } from "$lib/shared.svelte";

  import { getCoreSettings, setCoreSettings } from "@/lib/agent";

  type Props = {
    id: string;
    name: string;
    run_on_start?: boolean | undefined;
  };

  let { id, name, run_on_start }: Props = $props();

  // start and stop

  let running = $derived(streamInfos.get(id)?.running);

  async function handleStart() {
    await startStream(id);
  }

  async function handleStop() {
    await stopStream(id);
  }

  // rename and delete

  let new_name = $state("");
  let openRenameDialog = $state(false);
  let openDeleteDialog = $state(false);

  async function handleRenameStream(e: Event) {
    e.preventDefault();
    new_name = name;
    openRenameDialog = true;
  }

  async function handleRenameStreamSubmit(e: Event) {
    e.preventDefault();
    if (!new_name) return;
    await renameStream(id, new_name);
    openRenameDialog = false;
    await reloadStreamInfos();
  }

  async function handleDuplicateStream(e: Event) {
    e.preventDefault();
    const s = await getAgentStreamSpec(id);
    if (!s) return;
    const new_name = await uniqueStreamName(name);
    const new_id = await addAgentStream(new_name, s);
    if (!new_id) return;
    await reloadStreamInfos();
  }

  async function handleDeleteStreamSubmit(e: Event) {
    e.preventDefault();
    await deleteStream(id);
    openDeleteDialog = false;
  }

  async function handleRunOnStart() {
    // update core setting
    const coreSettings = await getCoreSettings();
    const auto_start_streams = coreSettings.auto_start_streams || [];
    if (run_on_start) {
      // remove from streams_on_start
      const index = auto_start_streams.indexOf(name);
      if (index > -1) {
        auto_start_streams.splice(index, 1);
      }
    } else {
      // add to streams_on_start
      auto_start_streams.push(name);
    }
    coreSettings.auto_start_streams = auto_start_streams;
    await setCoreSettings(coreSettings);

    await reloadStreamInfos();
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
      <DropdownMenu.Item onclick={handleRenameStream}>Rename</DropdownMenu.Item>
      <DropdownMenu.Item onclick={handleDuplicateStream}>Duplicate</DropdownMenu.Item>
      <DropdownMenu.Item onclick={() => (openDeleteDialog = true)}>Delete</DropdownMenu.Item>
      <DropdownMenu.Separator />
      <DropdownMenu.Item onclick={handleRunOnStart}>Run on Start</DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
</div>

<Dialog.Root bind:open={openRenameDialog}>
  <Dialog.Content class="sm:max-w-[425px]">
    <form onsubmit={handleRenameStreamSubmit}>
      <Dialog.Header>
        <Dialog.Title>Rename Stream</Dialog.Title>
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
    <form onsubmit={handleDeleteStreamSubmit}>
      <Dialog.Header>
        <Dialog.Title>Delete Stream</Dialog.Title>
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
