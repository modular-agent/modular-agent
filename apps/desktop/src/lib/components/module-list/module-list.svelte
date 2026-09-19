<script lang="ts">
  import { Input } from "$lib/components/ui/input/index.js";
  import { getModuleDefinitions } from "$lib/module";

  import ModuleListItem from "./module-list-item.svelte";

  let {
    onAddModule,
    visible = false,
  }: {
    onAddModule: (moduleName: string) => void;
    visible?: boolean;
  } = $props();

  const moduleDefs = getModuleDefinitions();

  let searchRef: HTMLInputElement | null = $state(null);

  $effect(() => {
    if (visible) {
      searchRef?.focus();
    }
  });

  let searchTerm = $state("");

  const filteredModuleDefs = $derived(
    Object.fromEntries(
      Object.entries(moduleDefs).filter(([_name, def]) => {
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
  const expandAll = $derived(Object.keys(filteredModuleDefs).length <= EXPAND_THRESHOLD);

  const categories = $derived(
    Object.keys(filteredModuleDefs).reduce(
      (acc, key) => {
        const categoryPath = (filteredModuleDefs[key].category ?? "_unknown_").split("/");
        let currentLevel = acc;

        for (const part of categoryPath) {
          if (!currentLevel[part]) {
            currentLevel[part] = {};
          }
          currentLevel = currentLevel[part];
        }

        if (!currentLevel["00modules"]) {
          currentLevel["00modules"] = [];
        }
        currentLevel["00modules"].push(key);

        return acc;
      },
      {} as Record<string, any>,
    ),
  );
</script>

<div class="flex flex-col gap-2 p-2">
  <div class="flex items-center gap-4">
    <span class="text-sm font-medium">Modules</span>
    <Input bind:ref={searchRef} type="search" class="text-sm h-6" bind:value={searchTerm} />
  </div>
</div>
<div class="max-h-80 overflow-y-auto px-1">
  <ul class="flex w-full min-w-0 flex-col gap-1">
    <ModuleListItem {categories} {moduleDefs} {expandAll} {onAddModule} />
  </ul>
</div>
