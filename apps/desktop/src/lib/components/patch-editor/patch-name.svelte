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

  // Deeper paths collapse their leading folders into an ellipsis so the titlebar stays one line.
  const MAX_VISIBLE = 3;

  let path_components = $derived(name ? name.split("/") : []);
  let collapsed = $derived(path_components.length > MAX_VISIBLE);
  let visible_components = $derived(collapsed ? path_components.slice(-2) : path_components);
</script>

<Breadcrumb.Root bind:ref title={name} class={className} {...restProps}>
  <Breadcrumb.List class="min-w-0 flex-nowrap">
    {#if collapsed}
      <Breadcrumb.Item class="shrink-0">
        <Breadcrumb.Ellipsis class="size-auto" />
      </Breadcrumb.Item>
      <Breadcrumb.Separator class="shrink-0">
        <SlashIcon />
      </Breadcrumb.Separator>
    {/if}
    {#each visible_components as component, index (index)}
      <Breadcrumb.Item class="min-w-0">
        <Breadcrumb.Page class="min-w-0 truncate font-bold">{component}</Breadcrumb.Page>
      </Breadcrumb.Item>
      {#if index < visible_components.length - 1}
        <Breadcrumb.Separator class="shrink-0">
          <SlashIcon />
        </Breadcrumb.Separator>
      {/if}
    {/each}
    {#if dirty}
      <span class="text-muted-foreground ml-0.5 shrink-0">*</span>
    {/if}
  </Breadcrumb.List>
</Breadcrumb.Root>
