<script lang="ts">
  import { getAgentDefinitions } from "$lib/agent";
  import { Input } from "$lib/components/ui/input/index.js";

  import AgentListItem from "./agent-list-item.svelte";

  let {
    onAddAgent,
    visible = false,
  }: {
    onAddAgent: (agentName: string) => void;
    visible?: boolean;
  } = $props();

  const agentDefs = getAgentDefinitions();

  let searchRef: HTMLInputElement | null = $state(null);

  $effect(() => {
    if (visible) {
      searchRef?.focus();
    }
  });

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

<div class="flex flex-col gap-2 p-2">
  <div class="flex items-center gap-4">
    <span class="text-sm font-medium">Agents</span>
    <Input bind:ref={searchRef} type="search" class="text-sm h-6" bind:value={searchTerm} />
  </div>
</div>
<div class="max-h-80 overflow-y-auto px-1">
  <ul class="flex w-full min-w-0 flex-col gap-1">
    <AgentListItem {categories} {agentDefs} {expandAll} {onAddAgent} />
  </ul>
</div>
