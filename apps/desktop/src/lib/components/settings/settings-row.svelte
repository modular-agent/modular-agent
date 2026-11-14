<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    label: string;
    description?: string | null;
    /** The control. Receives the label's id for `aria-labelledby`. */
    children?: Snippet<[string]>;
  }

  const { label, description, children }: Props = $props();

  const labelId = $props.id();
</script>

<!-- The label's basis, not its text length, decides when the control drops to
     its own line: beside the label while there is room, below it when the
     dialog is narrow. -->
<div class="flex min-h-14 flex-wrap items-center gap-x-6 gap-y-2 py-3">
  <div class="min-w-0 flex-1 basis-40">
    <div id={labelId} class="text-sm">{label}</div>
    {#if description}
      <div class="mt-0.5 text-xs text-muted-foreground">{description}</div>
    {/if}
  </div>
  {#if children}
    <div class="flex max-w-full flex-wrap items-center gap-2 *:max-w-full">
      {@render children(labelId)}
    </div>
  {/if}
</div>
