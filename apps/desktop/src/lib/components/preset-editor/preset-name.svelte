<script lang="ts">
  import type { HTMLAttributes } from "svelte/elements";

  import SlashIcon from "@lucide/svelte/icons/slash";

  import * as Breadcrumb from "$lib/components/ui/breadcrumb/index.js";
  import type { WithElementRef } from "$lib/utils";

  let {
    name,
    dirty = false,
    ref = $bindable(null),
    class: className,
    ...restProps
  }: WithElementRef<HTMLAttributes<HTMLElement>> & { name?: string; dirty?: boolean } = $props();

  let path_components = $derived(name ? name.split("/") : []);
</script>

<Breadcrumb.Root bind:ref class={className} {...restProps}>
  <Breadcrumb.List>
    {#each path_components as component, index (index)}
      <Breadcrumb.Item>
        <Breadcrumb.Page><span class="font-bold">{component}</span></Breadcrumb.Page>
      </Breadcrumb.Item>
      {#if index < path_components.length - 1}
        <Breadcrumb.Separator>
          <SlashIcon />
        </Breadcrumb.Separator>
      {/if}
    {/each}
    {#if dirty}
      <span class="text-muted-foreground ml-0.5">*</span>
    {/if}
  </Breadcrumb.List>
</Breadcrumb.Root>
