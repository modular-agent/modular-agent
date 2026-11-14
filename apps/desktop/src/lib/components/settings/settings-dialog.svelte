<script lang="ts">
  import type { Component } from "svelte";

  import KeyboardIcon from "@lucide/svelte/icons/keyboard";
  import LayoutGridIcon from "@lucide/svelte/icons/layout-grid";
  import PackageIcon from "@lucide/svelte/icons/package";
  import PlugIcon from "@lucide/svelte/icons/plug";
  import Settings2Icon from "@lucide/svelte/icons/settings-2";
  import XIcon from "@lucide/svelte/icons/x";
  import { Tabs } from "bits-ui";
  import type { ModuleConfigs } from "tauri-plugin-modular-agent-api";

  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { getGlobalConfigs } from "$lib/modular_agent";
  import { getModuleDefinitions } from "$lib/module";

  import CanvasSection from "./canvas-section.svelte";
  import GeneralSection from "./general-section.svelte";
  import McpSection from "./mcp-section.svelte";
  import ModuleSection from "./module-section.svelte";
  import ShortcutsSection from "./shortcuts-section.svelte";
  import { settingsDialog } from "./state.svelte";

  type NavItem = { value: string; label: string; icon: Component };
  type NavGroup = { group: string | null; items: NavItem[] };

  const CORE_NAV: NavGroup[] = [
    { group: null, items: [{ value: "general", label: "General", icon: Settings2Icon }] },
    {
      group: "Editor",
      items: [
        { value: "canvas", label: "Canvas", icon: LayoutGridIcon },
        { value: "shortcuts", label: "Shortcuts", icon: KeyboardIcon },
      ],
    },
    { group: "Integrations", items: [{ value: "mcp", label: "MCP Server", icon: PlugIcon }] },
  ];

  const tabClass =
    "flex w-full cursor-pointer items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm text-muted-foreground outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 data-[state=active]:bg-background data-[state=active]:font-medium data-[state=active]:text-foreground data-[state=active]:shadow-sm dark:data-[state=active]:bg-accent";

  const moduleDefs = getModuleDefinitions();

  let section = $state("general");
  let globalConfigs = $state<Record<string, ModuleConfigs>>({});

  const moduleValue = (moduleName: string) => `module:${moduleName}`;

  // A definition whose global configs are all hidden has nothing to show.
  const moduleSections = $derived(
    Object.entries(globalConfigs).filter(([moduleName]) =>
      Object.values(moduleDefs[moduleName]?.global_configs ?? {}).some(
        (spec) => spec.hidden !== true,
      ),
    ),
  );

  const nav = $derived.by(() => {
    const items = moduleSections.map(([moduleName]) => ({
      value: moduleValue(moduleName),
      label: moduleDefs[moduleName]?.title ?? moduleName,
      icon: PackageIcon,
    }));
    return items.length > 0 ? [...CORE_NAV, { group: "Modules", items }] : CORE_NAV;
  });

  const sectionLabel = $derived(
    nav.flatMap((g) => g.items).find((item) => item.value === section)?.label,
  );

  $effect(() => {
    if (!settingsDialog.open) return;
    section = "general";
    getGlobalConfigs()
      .then((configs) => (globalConfigs = configs))
      .catch((e) => console.error("Failed to load module settings:", e));
  });
</script>

<Dialog.Root bind:open={settingsDialog.open}>
  <Dialog.Content
    showCloseButton={false}
    class="flex h-[min(720px,calc(100vh-4rem))] w-[min(960px,calc(100vw-4rem))] max-w-none gap-0 overflow-hidden rounded-xl p-0 sm:max-w-none"
  >
    <Tabs.Root orientation="vertical" bind:value={section} class="flex min-h-0 min-w-0 flex-1">
      <div class="flex w-56 shrink-0 flex-col overflow-y-auto border-r bg-muted/50 p-3">
        <Dialog.Title class="px-3 pt-1 pb-3 text-sm font-medium text-muted-foreground">
          Settings
        </Dialog.Title>
        <Tabs.List class="flex flex-col gap-0.5">
          {#each nav as g (g.group)}
            {#if g.group}
              <div
                aria-hidden="true"
                class="px-3 pt-4 pb-1 text-xs font-medium text-muted-foreground"
              >
                {g.group}
              </div>
            {/if}
            {#each g.items as item (item.value)}
              <Tabs.Trigger value={item.value} class={tabClass}>
                <item.icon class="size-4 shrink-0" />
                <span class="truncate">{item.label}</span>
              </Tabs.Trigger>
            {/each}
          {/each}
        </Tabs.List>
      </div>

      <div class="flex min-w-0 flex-1 flex-col">
        <div class="flex h-14 shrink-0 items-center justify-between border-b px-8">
          <div class="text-base font-semibold">{sectionLabel}</div>
          <Dialog.Close
            class="flex size-8 items-center justify-center rounded-full bg-muted text-muted-foreground hover:bg-accent hover:text-foreground"
          >
            <XIcon class="size-4" />
            <span class="sr-only">Close</span>
          </Dialog.Close>
        </div>
        <!-- Tabs.Content hides inactive panels with the `hidden` attribute, so
             the panels themselves must not carry a display class. -->
        <div class="min-h-0 flex-1 overflow-y-auto px-8 py-6">
          <Tabs.Content value="general"><GeneralSection /></Tabs.Content>
          <Tabs.Content value="canvas"><CanvasSection /></Tabs.Content>
          <Tabs.Content value="shortcuts"><ShortcutsSection /></Tabs.Content>
          <Tabs.Content value="mcp"><McpSection /></Tabs.Content>
          {#each moduleSections as [moduleName, moduleConfigs] (moduleName)}
            <Tabs.Content value={moduleValue(moduleName)}>
              <ModuleSection {moduleName} {moduleConfigs} moduleDef={moduleDefs[moduleName]} />
            </Tabs.Content>
          {/each}
        </div>
      </div>
    </Tabs.Root>
  </Dialog.Content>
</Dialog.Root>
