<script lang="ts">
  import XIcon from "@lucide/svelte/icons/x";

  import { Button } from "$lib/components/ui/button/index.js";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";

  import AgentConfig from "./agent-config.svelte";
  import { useEditor } from "./context.svelte";

  const editor = useEditor();
  const inspector = editor.inspector;

  function updateConfig(key: string, value: any) {
    inspector.onUpdateConfig?.(key, value);
  }
</script>

<div
  class="w-72 flex-none flex flex-col border-l border-border bg-background h-full overflow-hidden"
>
  <!-- Header -->
  <div class="flex items-center justify-between px-3 py-2 border-b border-border flex-none">
    <span class="text-sm font-semibold">Inspector</span>
    <Button
      variant="ghost"
      size="icon"
      class="h-6 w-6"
      onclick={() => editor.toggleSidebar()}
    >
      <XIcon class="h-4 w-4" />
    </Button>
  </div>

  {#if inspector.hasSelection}
    <ScrollArea class="flex-1">
      <div class="p-3 flex flex-col gap-3">
        <!-- Agent Info -->
        <div class="flex flex-col gap-1 text-sm">
          <div class="font-medium">{inspector.displayTitle}</div>
          {#if inspector.agentDef?.category}
            <div class="text-xs text-muted-foreground">{inspector.agentDef.category}</div>
          {/if}
          {#if inspector.agentDef?.description}
            <p class="text-xs text-muted-foreground mt-1">{inspector.agentDef.description}</p>
          {/if}
        </div>

        <Separator />

        <!-- Configs -->
        {#if Object.keys(inspector.configs).length > 0}
          <form class="flex flex-col gap-2">
            {#each Object.entries(inspector.configs) as [key, value]}
              <AgentConfig
                name={key}
                {value}
                configSpec={inspector.configSpecs[key]}
                connected={inspector.connectedConfigs.includes(key)}
                {updateConfig}
                showHandle={false}
              />
            {/each}
          </form>
        {/if}
      </div>
    </ScrollArea>
  {:else}
    <div class="flex-1 flex items-center justify-center text-sm text-muted-foreground p-4">
      {#if inspector.selectedCount === 0}
        Select a node to inspect
      {:else}
        {inspector.selectedCount} nodes selected
      {/if}
    </div>
  {/if}
</div>
