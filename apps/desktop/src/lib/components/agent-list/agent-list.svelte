<script lang="ts">
  import { type ComponentProps } from "svelte";

  import { getAgentDefinitions } from "$lib/agent";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";

  import AgentListItem from "./agent-list-item.svelte";

  type Props = ComponentProps<typeof Sidebar.Root> & {
    onAddAgent: (agentName: string, position?: { x: number; y: number }) => Promise<void>;
    onDragAgentStart?: (event: DragEvent, agentName: string) => void;
  };

  let { onAddAgent, onDragAgentStart, ...restProps }: Props = $props();

  const agentDefs = getAgentDefinitions();

  let searchTerm = $state("");

  const filteredAgentDefs = $derived(
    Object.fromEntries(
      Object.entries(agentDefs).filter(([_name, def]) => {
        const term = searchTerm.trim().toLowerCase();
        if (!term) {
          return true;
        }
        const title = (def.title ?? "").toLowerCase();
        return title.includes(term);
      }),
    ),
  );

  const EXPAND_THRESHOLD = 8;
  const expandAll = $derived(Object.keys(filteredAgentDefs).length <= EXPAND_THRESHOLD);

  const categories = $derived(
    Object.keys(filteredAgentDefs).reduce(
      (acc, key) => {
        const categoryPath = (filteredAgentDefs[key].category ?? "_unknown_").split("/");
        let currentLevel = acc;

        for (const part of categoryPath) {
          if (!currentLevel[part]) {
            currentLevel[part] = {};
          }
          currentLevel = currentLevel[part];
        }

        if (!currentLevel["00agents"]) {
          currentLevel["00agents"] = [];
        }
        currentLevel["00agents"].push(key);

        return acc;
      },
      {} as Record<string, any>,
    ),
  );
</script>

<Sidebar.Root collapsible="none" variant="floating" class="rounded-md" {...restProps}>
  <Sidebar.Header>
    <div class="flex items-center gap-4">
      <Label>Agents</Label>
      <Input
        type="search"
        class="mr-8 text-sm h-6 bg-sidebar dark:bg-sidebar"
        bind:value={searchTerm}
      />
    </div>
  </Sidebar.Header>
  <Sidebar.Content>
    <Sidebar.Menu>
      <AgentListItem {categories} {agentDefs} {expandAll} {onDragAgentStart} />
    </Sidebar.Menu>
  </Sidebar.Content>
</Sidebar.Root>
