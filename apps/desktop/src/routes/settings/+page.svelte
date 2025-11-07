<script lang="ts">
  import Agent from "./Agent.svelte";
  import Core from "./Core.svelte";

  const { data } = $props();

  const settings = $derived(data.coreSettings);
  const agentGlobalConfigsMap = $derived(data.agentGlobalConfigsMap);
  const agentDefs = $derived(data.agentDefs);
</script>

<main class="container mx-auto p-8 space-y-8 mt-20">
  <h1 class="text-xl font-semibold sm:text-2xl">Settings</h1>
  <Core {settings} />

  <h2 class="text-xl font-semibold sm:text-2xl">Agents</h2>
  {#each Object.entries(agentGlobalConfigsMap) as [agentName, agentConfigs]}
    <Agent {agentName} {agentConfigs} agentDef={agentDefs[agentName]} />
  {/each}
</main>
