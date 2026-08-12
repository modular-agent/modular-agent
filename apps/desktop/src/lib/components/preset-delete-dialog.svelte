<script lang="ts">
  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";

  type Props = {
    name: string;
    isFolder?: boolean;
    /** When set, the reason deleting is not allowed. Shown instead of the
     * confirmation question, with the Delete button disabled. */
    blocked?: string;
    onDelete: (name: string) => void;
    open?: boolean;
  };

  let { name, isFolder = false, blocked = "", onDelete, open = $bindable(false) }: Props = $props();

  async function handleDelete() {
    await onDelete(name);
    open = false;
  }
</script>

<AlertDialog.Root bind:open>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Delete {isFolder ? "Folder" : "Preset"}</AlertDialog.Title>
      <AlertDialog.Description>
        {#if blocked}
          {blocked}
        {:else}
          Are you sure you want to delete "{name}"? This action cannot be undone.
        {/if}
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action onclick={handleDelete} disabled={!!blocked}>Delete</AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
