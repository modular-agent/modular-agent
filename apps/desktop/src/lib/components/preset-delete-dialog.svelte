<script lang="ts">
  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";

  type Props = {
    name: string;
    onDelete: (name: string) => void;
    open?: boolean;
  };

  let { name, onDelete, open = $bindable(false) }: Props = $props();

  async function handleDelete() {
    await onDelete(name);
    open = false;
  }
</script>

<AlertDialog.Root bind:open>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Delete Preset</AlertDialog.Title>
      <AlertDialog.Description>
        Are you sure you want to delete "{name}"? This action cannot be undone.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action onclick={handleDelete}>Delete</AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
