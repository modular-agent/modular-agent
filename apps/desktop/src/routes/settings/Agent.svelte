<script lang="ts">
  import { toast } from "svelte-sonner";
  import type { AgentConfigs, AgentDefinition } from "tauri-plugin-modular-agent-api";

  import * as Card from "$lib/components/ui/card/index.js";
  import { FieldGroup, Field, FieldLabel } from "$lib/components/ui/field/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { setGlobalConfigs } from "$lib/modular_agent";
  import { renderMarkdown } from "$lib/sanitize";

  import FieldDescription from "@/lib/components/ui/field/field-description.svelte";

  interface Props {
    agentName: string;
    agentConfigs: AgentConfigs;
    agentDef: AgentDefinition | null;
  }

  const { agentName, agentConfigs, agentDef }: Props = $props();

  // Keep configs state-backed so bind targets are reactive and auto-save reads current values.
  let configs = $state<AgentConfigs>({});
  $effect(() => {
    configs = agentConfigs;
  });
  let ad = $derived.by(() => {
    let ad = agentDef;
    return ad;
  });

  let configsModified = $state(false);
  let descOpen = $state(false);
  const hasDesc = $derived(!!ad?.description?.trim());

  async function autoSaveConfig() {
    try {
      await setGlobalConfigs(agentName, configs);
      configsModified = true;
    } catch (e) {
      toast.error("Failed to save agent settings");
      console.error("Failed to save agent settings:", e);
    }
  }

  function blurOnEnter(e: KeyboardEvent) {
    if (e.key === "Enter") (e.target as HTMLInputElement).blur();
  }
</script>

<Card.Root class="@container/card">
  <Card.Header>
    {#if ad?.category?.trim()}
      <div class="text-xs text-muted-foreground uppercase font-medium tracking-wide">
        {ad.category}
      </div>
    {/if}
    {#if hasDesc}
      <button
        class="leading-none font-semibold bg-transparent border-0 p-0 m-0 text-left font-inherit text-inherit cursor-pointer hover:opacity-70 transition-opacity"
        onclick={() => (descOpen = !descOpen)}
        aria-expanded={descOpen}
      >
        {ad?.title ?? agentName}
      </button>
    {:else}
      <Card.Title>{ad?.title ?? agentName}</Card.Title>
    {/if}
  </Card.Header>
  <Card.Content>
    {#if hasDesc}
      <div class="desc-collapse" class:desc-open={descOpen}>
        <div class="agent-desc-md text-sm overflow-hidden">
          {@html renderMarkdown(ad?.description ?? "")}
        </div>
      </div>
    {/if}
    {#if ad?.global_configs}
      <FieldGroup>
        {#each Object.entries(ad.global_configs) as [key, globalConfig]}
          <Field orientation="horizontal" class="grid gap-4 sm:grid-cols-[220px_1fr] items-center">
            <div class="flex flex-col">
              <FieldLabel>
                {globalConfig?.title || key}
              </FieldLabel>
              <FieldDescription>{globalConfig?.description}</FieldDescription>
            </div>
            {@const ty = globalConfig.type}
            {#if ty === "boolean"}
              <Switch bind:checked={configs[key]} onCheckedChange={() => autoSaveConfig()} />
            {:else if ty === "integer"}
              <Input
                bind:value={configs[key]}
                onchange={() => autoSaveConfig()}
                onkeydown={blurOnEnter}
              />
            {:else if ty === "number"}
              <Input
                type="number"
                bind:value={configs[key]}
                onchange={() => autoSaveConfig()}
                onkeydown={blurOnEnter}
              />
            {:else if ty === "string"}
              <Input
                type="text"
                bind:value={configs[key]}
                onchange={() => autoSaveConfig()}
                onkeydown={blurOnEnter}
              />
            {:else if ty === "text"}
              <Input
                bind:value={configs[key]}
                onchange={() => autoSaveConfig()}
                onkeydown={blurOnEnter}
              />
            {:else if ty === "password"}
              <Input
                type="password"
                bind:value={configs[key]}
                onchange={() => autoSaveConfig()}
                onkeydown={blurOnEnter}
              />
            {:else if ty === "object"}
              <Input
                bind:value={configs[key]}
                onchange={() => autoSaveConfig()}
                onkeydown={blurOnEnter}
              />
            {:else}
              <Input type="text" value={JSON.stringify(configs[key], null, 2)} disabled />
            {/if}
          </Field>
        {/each}
      </FieldGroup>
      {#if configsModified}
        <p class="text-sm text-amber-600 dark:text-amber-400 mt-2" role="status">
          Changes may require restarting running presets to take effect.
        </p>
      {/if}
    {/if}
  </Card.Content>
</Card.Root>

<style>
  .desc-collapse {
    display: grid;
    grid-template-rows: 0fr;
    transition:
      grid-template-rows 150ms ease-out,
      margin 150ms ease-out;
    margin-bottom: 0;
  }
  .desc-collapse.desc-open {
    grid-template-rows: 1fr;
    margin-bottom: 1rem;
  }
  .desc-collapse > :global(div) {
    min-height: 0;
  }

  .agent-desc-md :global(p) {
    margin-bottom: 0.4rem;
  }
  .agent-desc-md :global(p:last-child) {
    margin-bottom: 0;
  }
  .agent-desc-md :global(code) {
    background-color: var(--muted);
    padding: 0.1rem 0.3rem;
    border-radius: 0.2rem;
    font-size: 0.85em;
  }
  .agent-desc-md :global(pre) {
    background-color: var(--muted);
    padding: 0.5rem;
    border-radius: 0.3rem;
    overflow-x: auto;
    margin-bottom: 0.4rem;
  }
  .agent-desc-md :global(pre code) {
    background-color: transparent;
    padding: 0;
  }
  .agent-desc-md :global(a) {
    color: var(--link-color);
    text-decoration: underline;
  }
  .agent-desc-md :global(a:hover) {
    opacity: 0.8;
  }
  .agent-desc-md :global(blockquote) {
    border-left: 3px solid var(--border);
    padding-left: 0.75rem;
    margin-left: 0;
    margin-bottom: 0.4rem;
    color: var(--muted-foreground);
  }
  .agent-desc-md :global(ul),
  .agent-desc-md :global(ol) {
    padding-left: 1.5rem;
    margin-bottom: 0.4rem;
  }
  .agent-desc-md :global(li) {
    margin-bottom: 0.1rem;
  }
  .agent-desc-md :global(table) {
    border-collapse: separate;
    border-spacing: 0;
  }
  .agent-desc-md :global(th) {
    text-align: left;
    font-weight: 700;
    border-bottom: 1.5px solid var(--border);
    padding-bottom: 0.35rem;
  }
  .agent-desc-md :global(td) {
    text-align: left;
  }
  .agent-desc-md :global(th),
  .agent-desc-md :global(td) {
    padding: 0;
  }
  .agent-desc-md :global(th:not(:first-child)),
  .agent-desc-md :global(td:not(:first-child)) {
    padding-left: 1rem;
  }
  .agent-desc-md :global(tbody td) {
    padding-top: 0.35rem;
  }
</style>
