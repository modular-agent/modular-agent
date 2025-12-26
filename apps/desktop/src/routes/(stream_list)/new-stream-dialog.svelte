<script lang="ts">
  import { goto } from "$app/navigation";

  import { Button, buttonVariants } from "$lib/components/ui/button/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { newStream } from "$lib/shared.svelte";

  let name = $state("");

  async function handleNewStream(e: Event) {
    e.preventDefault();
    if (!name) return;
    const stream_id = await newStream(name);
    name = "";
    if (stream_id) {
      goto(`/stream_editor/${stream_id}`);
    }
  }
</script>

<Dialog.Root>
  <form>
    <Dialog.Trigger class={buttonVariants({ variant: "outline" })}>+ New</Dialog.Trigger>
    <Dialog.Content class="sm:max-w-[425px]">
      <Dialog.Header>
        <Dialog.Title>New Stream</Dialog.Title>
      </Dialog.Header>
      <div class="grid gap-4">
        <div class="grid gap-3">
          <Label for="name-1">Name</Label>
          <Input id="name-1" name="name" bind:value={name} />
        </div>
      </div>
      <Dialog.Footer>
        <Button type="submit" onclick={handleNewStream}>New</Button>
      </Dialog.Footer>
    </Dialog.Content>
  </form>
</Dialog.Root>
