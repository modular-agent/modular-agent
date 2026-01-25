<script lang="ts">
  import type { ComponentProps } from "svelte";

  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import type { WithoutChildren } from "$lib/utils";

  let {
    items,
  }: { items: { title: string; url: string; icon?: any }[] } & WithoutChildren<
    ComponentProps<typeof Sidebar.Group>
  > = $props();
</script>

<Sidebar.Group>
  <Sidebar.GroupContent class="flex flex-col gap-2">
    <Sidebar.Menu>
      {#each items as item (item.title)}
        <Sidebar.MenuItem>
          <Sidebar.MenuButton>
            {#snippet child({ props })}
              <a href={item.url} {...props}>
                <item.icon />
                <span>{item.title}</span>
              </a>
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
