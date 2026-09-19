<script lang="ts">
  import { onMount } from "svelte";

  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import { getModuleDefinitions, getCoreSettings, getGlobalConfigsMap } from "$lib/module";
  import { titlebarState } from "$lib/titlebar-state.svelte";

  import Core from "./Core.svelte";
  import Module from "./Module.svelte";

  const coreSettings = getCoreSettings();
  const moduleDefs = getModuleDefinitions();
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
        <div class="flex-none text-xl font-semibold">Modules</div>
        {#each Object.entries(globalConfigsMap) as [moduleName, moduleConfigs]}
          <Module {moduleName} {moduleConfigs} moduleDef={moduleDefs[moduleName]} />
        {/each}
      </div>
    </div>
  </div>
</ScrollArea>
