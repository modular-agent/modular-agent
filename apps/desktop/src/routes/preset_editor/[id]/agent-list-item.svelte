<script lang="ts">
  import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
  import type { AgentDefinitions } from "tauri-plugin-modular-agent-api";

  import * as Collapsible from "$lib/components/ui/collapsible/index.js";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";

  import AgentListItems from "./agent-list-item.svelte";

  interface Props {
    categories: Record<string, any>;
    agentDefs: AgentDefinitions;
    expandAll?: boolean;
    onDragAgentStart?: (event: DragEvent, agentName: string) => void;
  }

  let { categories, agentDefs, expandAll = false, onDragAgentStart }: Props = $props();

  const categoryKeys = $derived(Object.keys(categories).sort());
</script>

{#each categoryKeys as key}
  {#if key === "00agents"}
    {@const agentNames = categories[key].sort()}
    {#each agentNames as agentName}
      <Sidebar.MenuButton
        draggable={true}
        ondragstart={(event) => onDragAgentStart?.(event, agentName)}
      >
        {agentDefs[agentName].title ?? agentName}
      </Sidebar.MenuButton>
    {/each}
  {:else}
    <Sidebar.MenuItem>
      <Collapsible.Root
        open={expandAll}
        class="group/collapsible [&[data-state=open]>button>svg:first-child]:rotate-90"
      >
        <Collapsible.Trigger>
          {#snippet child({ props })}
            <Sidebar.MenuButton {...props}>
              <ChevronRightIcon className="transition-transform" />
              {key}
            </Sidebar.MenuButton>
          {/snippet}
        </Collapsible.Trigger>
        <Collapsible.Content>
          <Sidebar.MenuSub>
            <AgentListItems
              categories={categories[key]}
              {agentDefs}
              {expandAll}
              {onDragAgentStart}
            />
          </Sidebar.MenuSub>
        </Collapsible.Content>
      </Collapsible.Root>
    </Sidebar.MenuItem>
  {/if}
{/each}
