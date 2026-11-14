<script lang="ts">
  import { toast } from "svelte-sonner";
  import type { ModuleConfigs, ModuleDefinition } from "tauri-plugin-modular-agent-api";

  import { Input } from "$lib/components/ui/input/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Textarea } from "$lib/components/ui/textarea/index.js";
  import { parseConfigInput } from "$lib/config-input";
  import { setGlobalConfigs } from "$lib/modular_agent";
  import { renderMarkdown } from "$lib/sanitize";

  import { blurOnEnter } from "./helpers";
  import SettingsGroup from "./settings-group.svelte";
  import SettingsNote from "./settings-note.svelte";
  import SettingsRow from "./settings-row.svelte";

  interface Props {
    moduleName: string;
    moduleConfigs: ModuleConfigs;
    moduleDef: ModuleDefinition | undefined;
  }

  const { moduleName, moduleConfigs, moduleDef }: Props = $props();

  // Keep configs state-backed so bind targets are reactive and auto-save reads current values.
  let configs = $state<ModuleConfigs>({});
  $effect(() => {
    configs = moduleConfigs;
  });

  let configsModified = $state(false);

  // Keys whose JSON field holds text that was rejected and so is not saved.
  let invalid = $state<Record<string, boolean>>({});

  function updateConfig(key: string, value: unknown) {
    configs[key] = value;
    autoSaveConfig();
  }

  // A rejected number is dropped: the field goes back to the saved value.
  function commitNumber(key: string, type: "integer" | "number", input: HTMLInputElement) {
    const parsed = parseConfigInput(type, input.value);
    if (parsed.ok) {
      input.value = String(parsed.value);
      updateConfig(key, parsed.value);
    } else {
      toast.error(parsed.message);
      input.value = String(configs[key] ?? "");
    }
  }

  // Rejected JSON stays in the field, marked invalid, so it can be corrected
  // instead of retyped.
  function commitJson(key: string, type: "object" | "array", text: string) {
    const parsed = parseConfigInput(type, text);
    invalid[key] = !parsed.ok;
    if (parsed.ok) {
      updateConfig(key, parsed.value);
    } else {
      toast.error(parsed.message);
    }
  }

  async function autoSaveConfig() {
    try {
      await setGlobalConfigs(moduleName, configs);
      configsModified = true;
    } catch (e) {
      toast.error("Failed to save module settings");
      console.error("Failed to save module settings:", e);
    }
  }
</script>

<div class="space-y-6">
  {#if moduleDef?.category?.trim() || moduleDef?.description?.trim()}
    <div class="space-y-2 px-1">
      {#if moduleDef.category?.trim()}
        <div class="text-xs font-medium tracking-wide text-muted-foreground uppercase">
          {moduleDef.category}
        </div>
      {/if}
      {#if moduleDef.description?.trim()}
        <div class="module-desc-md text-sm">
          {@html renderMarkdown(moduleDef.description)}
        </div>
      {/if}
    </div>
  {/if}

  {#if moduleDef?.global_configs}
    <div>
      <SettingsGroup>
        {#each Object.entries(moduleDef.global_configs) as [key, globalConfig] (key)}
          {#if globalConfig.hidden !== true}
            <SettingsRow label={globalConfig.title || key} description={globalConfig.description}>
              {#snippet children(labelId)}
                {@const ty = globalConfig.type}
                {@const disabled = globalConfig.readonly === true}
                {#if ty === "boolean"}
                  <Switch
                    aria-labelledby={labelId}
                    checked={configs[key] === true}
                    onCheckedChange={(checked) => updateConfig(key, checked)}
                    {disabled}
                  />
                {:else if ty === "integer" || ty === "number"}
                  <Input
                    aria-labelledby={labelId}
                    type="number"
                    step={ty === "integer" ? 1 : "any"}
                    value={configs[key]}
                    onchange={(e) => commitNumber(key, ty, e.currentTarget)}
                    onkeydown={blurOnEnter}
                    {disabled}
                    class="w-64"
                  />
                {:else if ty === "string" || ty === "password"}
                  <Input
                    aria-labelledby={labelId}
                    type={ty === "password" ? "password" : "text"}
                    value={configs[key]}
                    onchange={(e) => updateConfig(key, e.currentTarget.value)}
                    onkeydown={blurOnEnter}
                    {disabled}
                    class="w-64"
                  />
                {:else if ty === "text"}
                  <Textarea
                    aria-labelledby={labelId}
                    spellcheck="false"
                    value={configs[key]}
                    onchange={(e) => updateConfig(key, e.currentTarget.value)}
                    {disabled}
                    class="max-h-60 w-80"
                  />
                {:else if ty === "object" || ty === "array"}
                  <Textarea
                    aria-labelledby={labelId}
                    aria-invalid={invalid[key] === true}
                    spellcheck="false"
                    value={JSON.stringify(configs[key], null, 2)}
                    onchange={(e) => commitJson(key, ty, e.currentTarget.value)}
                    {disabled}
                    class="max-h-60 w-80 font-mono text-xs md:text-xs"
                  />
                {:else}
                  <Input
                    aria-labelledby={labelId}
                    type="text"
                    value={JSON.stringify(configs[key], null, 2)}
                    disabled
                    class="w-64"
                  />
                {/if}
              {/snippet}
            </SettingsRow>
          {/if}
        {/each}
      </SettingsGroup>
      {#if configsModified}
        <SettingsNote>Changes may require restarting running patches to take effect.</SettingsNote>
      {/if}
    </div>
  {/if}
</div>

<style>
  .module-desc-md :global(p) {
    margin-bottom: 0.4rem;
  }
  .module-desc-md :global(p:last-child) {
    margin-bottom: 0;
  }
  .module-desc-md :global(code) {
    background-color: var(--muted);
    padding: 0.1rem 0.3rem;
    border-radius: 0.2rem;
    font-size: 0.85em;
  }
  .module-desc-md :global(pre) {
    background-color: var(--muted);
    padding: 0.5rem;
    border-radius: 0.3rem;
    overflow-x: auto;
    margin-bottom: 0.4rem;
  }
  .module-desc-md :global(pre code) {
    background-color: transparent;
    padding: 0;
  }
  .module-desc-md :global(a) {
    color: var(--link-color);
    text-decoration: underline;
  }
  .module-desc-md :global(a:hover) {
    opacity: 0.8;
  }
  .module-desc-md :global(blockquote) {
    border-left: 3px solid var(--border);
    padding-left: 0.75rem;
    margin-left: 0;
    margin-bottom: 0.4rem;
    color: var(--muted-foreground);
  }
  .module-desc-md :global(ul),
  .module-desc-md :global(ol) {
    padding-left: 1.5rem;
    margin-bottom: 0.4rem;
  }
  .module-desc-md :global(li) {
    margin-bottom: 0.1rem;
  }
  .module-desc-md :global(table) {
    border-collapse: separate;
    border-spacing: 0;
  }
  .module-desc-md :global(th) {
    text-align: left;
    font-weight: 700;
    border-bottom: 1.5px solid var(--border);
    padding-bottom: 0.35rem;
  }
  .module-desc-md :global(td) {
    text-align: left;
  }
  .module-desc-md :global(th),
  .module-desc-md :global(td) {
    padding: 0;
  }
  .module-desc-md :global(th:not(:first-child)),
  .module-desc-md :global(td:not(:first-child)) {
    padding-left: 1rem;
  }
  .module-desc-md :global(tbody td) {
    padding-top: 0.35rem;
  }
</style>
