<script lang="ts">
  import EllipsisVerticalIcon from "@lucide/svelte/icons/ellipsis-vertical";
  import PlayIcon from "@lucide/svelte/icons/play";
  import SquareIcon from "@lucide/svelte/icons/square";

  import { Button } from "$lib/components/ui/button/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";

  import { runningStreams, startStream, stopStream } from "@/lib/shared.svelte";

  let {
    id,
    renameStream,
    deleteStream,
    toggleRunOnStart,
  }: {
    id: string;
    renameStream: (id: string, rename: string) => Promise<string | null>;
    deleteStream: (id: string) => Promise<void>;
    toggleRunOnStart: (id: string) => Promise<void>;
  } = $props();

  // start and stop

  let running = $derived(runningStreams.has(id));

  async function handleStart() {
    await startStream(id);
  }

  async function handleStop() {
    await stopStream(id);
  }

  // rename and delete

  let name = $state("");
  let openRenameDialog = $state(false);
  let openDeleteDialog = $state(false);

  async function handleRenameStream(e: Event) {
    e.preventDefault();
    if (!name) return;
    const new_name = await renameStream(id, name);
    name = "";
    openRenameDialog = false;
  }

  async function handleDeleteStream(e: Event) {
    e.preventDefault();
    await deleteStream(id);
    openDeleteDialog = false;
  }

  async function handleRunOnStart(e: Event) {
    await toggleRunOnStart(id);
  }
</script>

{#if running}
  <Button onclick={handleStop} variant="ghost" class="w-4 mr-4">
    <SquareIcon color="red" />
  </Button>
{:else}
  <Button onclick={handleStart} variant="ghost" class="w-4 mr-4">
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
    <DropdownMenu.Item onclick={() => (openRenameDialog = true)}>Rename</DropdownMenu.Item>
    <DropdownMenu.Item onclick={() => (openDeleteDialog = true)}>Delete</DropdownMenu.Item>
    <DropdownMenu.Separator />
    <DropdownMenu.Item onclick={handleRunOnStart}>Run on Start</DropdownMenu.Item>
  </DropdownMenu.Content>
</DropdownMenu.Root>

<Dialog.Root bind:open={openRenameDialog}>
  <form>
    <Dialog.Content class="sm:max-w-[425px]">
      <Dialog.Header>
        <Dialog.Title>Rename Stream</Dialog.Title>
      </Dialog.Header>
      <div class="grid gap-4">
        <div class="grid gap-3">
          <Label for="name-1">Name</Label>
          <Input id="name-1" name="name" bind:value={name} />
        </div>
      </div>
      <Dialog.Footer>
        <Button type="submit" onclick={handleRenameStream}>Rename</Button>
      </Dialog.Footer>
    </Dialog.Content>
  </form>
</Dialog.Root>

<Dialog.Root bind:open={openDeleteDialog}>
  <form>
    <Dialog.Content class="sm:max-w-[425px]">
      <Dialog.Header>
        <Dialog.Title>Delete Stream</Dialog.Title>
        <Dialog.Description>Are you sure?</Dialog.Description>
      </Dialog.Header>
      <Dialog.Footer>
        <Button type="submit" onclick={handleDeleteStream}>Delete</Button>
      </Dialog.Footer>
    </Dialog.Content>
  </form>
</Dialog.Root>
