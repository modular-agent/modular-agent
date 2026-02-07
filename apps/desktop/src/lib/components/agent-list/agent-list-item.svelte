<script lang="ts">
  import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";

  import * as Collapsible from "$lib/components/ui/collapsible/index.js";

  import AgentListItem from "./agent-list-item.svelte";
  import type { AgentListItemProps } from "./types.js";

  let { categories, agentDefs, expandAll = false, onAddAgent }: AgentListItemProps = $props();

  const categoryKeys = $derived(Object.keys(categories).sort());
</script>

{#each categoryKeys as key}
  {#if key === "00agents"}
    {@const agentNames = categories[key].sort()}
    {#each agentNames as agentName}
      <li>
        <button
          class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-start text-sm
                 hover:bg-accent hover:text-accent-foreground"
          onclick={() => onAddAgent?.(agentName)}
        >
          {agentDefs[agentName].title ?? agentName}
        </button>
      </li>
    {/each}
  {:else}
    <li class="group/menu-item relative">
      <Collapsible.Root
        open={expandAll}
        class="[&[data-state=open]>button>svg:first-child]:rotate-90"
      >
        <Collapsible.Trigger>
          {#snippet child({ props })}
            <button
              {...props}
              class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-start text-sm
                     hover:bg-accent hover:text-accent-foreground"
            >
              <ChevronRightIcon class="size-4 transition-transform" />
              {key}
            </button>
          {/snippet}
        </Collapsible.Trigger>
        <Collapsible.Content>
          <ul class="mx-3.5 flex min-w-0 translate-x-px flex-col gap-1 border-s px-2.5 py-0.5">
            <AgentListItem categories={categories[key]} {agentDefs} {expandAll} {onAddAgent} />
          </ul>
        </Collapsible.Content>
      </Collapsible.Root>
    </li>
  {/if}
{/each}
