<script lang="ts">
  import { message } from "@tauri-apps/plugin-dialog";

  import { Button, Input, NumberInput, Textarea, Toggle } from "flowbite-svelte";
  import { setGlobalConfigs } from "tauri-plugin-askit-api";
  import type { AgentConfigs, AgentDefinition } from "tauri-plugin-askit-api";

  import Card from "@/components/Card.svelte";
  import { deserializeAgentConfigs, serializeAgentFlowNodeConfigs } from "@/lib/agent";
  import { exitApp } from "@/lib/utils";

  interface Props {
    agentName: string;
    agentConfigs: AgentConfigs;
    agentDef: AgentDefinition | null;
  }

  const { agentName, agentConfigs, agentDef }: Props = $props();

  const configs = $state(deserializeAgentConfigs(agentConfigs, agentDef?.global_configs ?? null));

  async function saveConfigs() {
    let sconfigs = serializeAgentFlowNodeConfigs(configs, agentDef?.global_configs ?? null);
    if (sconfigs) {
      setGlobalConfigs(agentName, sconfigs);
    }

    // confirm restart
    await message("Mnemnk will quit to apply changes.\n\nPlease restart.");
    await exitApp();
  }
</script>

{#if !agentDef}
  <Card title={agentName} subtitle="Agent not found">
    <p class="text-sm text-gray-500">This agent is not defined in the system.</p>
  </Card>
{:else if !agentDef.global_configs}
  <Card title={agentName} subtitle="No global config">
    <p class="text-sm text-gray-500">This agent has no global config.</p>
  </Card>
{:else}
  <Card title={agentDef.title ?? agentName} subtitle={agentDef.description}>
    {#if agentDef.global_configs}
      <form>
        {#each agentDef.global_configs as [key, globalConfig]}
          {@const ty = globalConfig.type}
          <label class="block mb-3 text-sm font-medium text-gray-900 dark:text-white">
            {globalConfig?.title || key}
            <p class="text-xs text-gray-500">{globalConfig?.description}</p>
            {#if ty === "boolean"}
              <Toggle bind:checked={configs[key]} />
            {:else if ty === "integer"}
              <NumberInput bind:value={configs[key]} />
            {:else if ty === "number"}
              <Input type="number" bind:value={configs[key]} />
            {:else if ty === "string"}
              <Input type="text" bind:value={configs[key]} />
            {:else if ty === "text"}
              <Textarea bind:value={configs[key]} />
            {:else if ty === "password"}
              <Input type="password" bind:value={configs[key]} />
            {:else if ty === "object"}
              <Textarea bind:value={configs[key]} />
            {:else}
              <Input type="text" value={JSON.stringify(configs[key], null, 2)} disabled />
            {/if}
          </label>
        {/each}

        <Button onclick={saveConfigs} class="mt-3 w-fit" outline>Save</Button>
      </form>
    {/if}
  </Card>
{/if}
