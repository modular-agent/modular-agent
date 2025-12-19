<script lang="ts">
  import { Accordion } from "flowbite-svelte";
  import type { AgentDefinitions } from "tauri-plugin-askit-api";

  import AgentListItems from "./AgentListItems.svelte";

  interface Props {
    agentDefs: AgentDefinitions;
    onAddAgent: (agentName: string) => Promise<void>;
  }

  let { agentDefs, onAddAgent }: Props = $props();

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

<div class="backdrop-blur-xs">
  <div class="mb-2 flex items-center justify-between gap-2">
    <h4 class="mb-0">Agents</h4>
    <input
      type="search"
      class="w-40 mr-1 rounded-md border border-gray-800 bg-gray-900 px-2 py-0.5 text-xs text-gray-100 focus:border-gray-400 focus:outline-none focus:ring-1 focus:ring-gray-400"
      bind:value={searchTerm}
    />
  </div>
  <hr />
  <Accordion flush class="" multiple={expandAll}>
    <AgentListItems {categories} {agentDefs} {expandAll} {onAddAgent} />
  </Accordion>
</div>
