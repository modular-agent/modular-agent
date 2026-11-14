<script lang="ts">
  import type { ComponentProps } from "svelte";

  import { mergeProps } from "bits-ui";

  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import type { WithoutChildren } from "$lib/utils";

  type Item = { title: string; icon: any } & ({ url: string } | { onclick: () => void });

  let {
    items,
    ...restProps
  }: { items: Item[] } & WithoutChildren<ComponentProps<typeof Sidebar.Group>> = $props();
</script>

<Sidebar.Group {...restProps}>
  <Sidebar.GroupContent>
    <Sidebar.Menu>
      {#each items as item (item.title)}
        <Sidebar.MenuItem>
          <Sidebar.MenuButton>
            {#snippet child({ props })}
              {#if "url" in item}
                <a href={item.url} {...props}>
                  <item.icon />
                  <span>{item.title}</span>
                </a>
              {:else}
                <button type="button" {...mergeProps(props, { onclick: item.onclick })}>
                  <item.icon />
                  <span>{item.title}</span>
                </button>
              {/if}
            {/snippet}
            {#snippet tooltipContent()}
              <span>{item.title}</span>
            {/snippet}
          </Sidebar.MenuButton>
        </Sidebar.MenuItem>
      {/each}
    </Sidebar.Menu>
  </Sidebar.GroupContent>
</Sidebar.Group>
