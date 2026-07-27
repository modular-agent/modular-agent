<script lang="ts">
  import { onMount } from "svelte";

  import { getAgentDefinitions, getCoreSettings, getGlobalConfigsMap } from "$lib/agent";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import { titlebarState } from "$lib/titlebar-state.svelte";

  import Agent from "./Agent.svelte";
  import Core from "./Core.svelte";

  const coreSettings = getCoreSettings();
  const agentDefs = getAgentDefinitions();
  const globalConfigsMap = getGlobalConfigsMap();

  onMount(() => {
    titlebarState.reset();
    titlebarState.title = "Settings";
  });
</script>

<ScrollArea class="w-full h-full">
  <div class="w-full pt-4 pl-4 pr-4 pb-6">
    <header class="flex-none h-14 items-center">
      <div class="text-2xl font-semibold">Settings</div>
    </header>
    <div class="@container/main flex flex-1 flex-col">
      <Core settings={coreSettings} />

      <div class="flex flex-col mt-8 gap-6">
        <div class="flex-none text-xl font-semibold">Agents</div>
        {#each Object.entries(globalConfigsMap) as [agentName, agentConfigs]}
          <Agent {agentName} {agentConfigs} agentDef={agentDefs[agentName]} />
        {/each}
      </div>
    </div>
  </div>
</ScrollArea>
