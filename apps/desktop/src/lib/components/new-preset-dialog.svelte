<script lang="ts">
  import type { Snippet } from "svelte";

  import { Button } from "$lib/components/ui/button/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";

  type Props = {
    onNewPreset: (name: string) => void;
    open?: boolean;
    trigger?: Snippet;
  };

  let { onNewPreset, open = $bindable(false), trigger }: Props = $props();

  let name = $state("");

  async function handleNewPreset(e: Event) {
    e.preventDefault();
    if (!name) return;
    await onNewPreset(name);
    open = false;
    name = "";
  }
</script>

<Dialog.Root bind:open>
  <form>
    {#if trigger}
      {@render trigger()}
    {/if}
    <Dialog.Content class="sm:max-w-[425px]">
      <Dialog.Header>
        <Dialog.Title>New Preset</Dialog.Title>
      </Dialog.Header>
      <div class="grid gap-4">
        <div class="grid gap-3">
          <Label for="name-1">Name</Label>
          <Input id="name-1" name="name" bind:value={name} />
        </div>
      </div>
      <Dialog.Footer>
        <Button type="submit" onclick={handleNewPreset}>New</Button>
      </Dialog.Footer>
    </Dialog.Content>
  </form>
</Dialog.Root>
