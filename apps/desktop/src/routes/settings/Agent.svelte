<script lang="ts">
  import type { AgentConfigs, AgentDefinition } from "tauri-plugin-modular-agent-api";

  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { FieldGroup, Field, FieldLabel } from "$lib/components/ui/field/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { setGlobalConfigs } from "$lib/mak";

  import FieldDescription from "@/lib/components/ui/field/field-description.svelte";

  interface Props {
    agentName: string;
    agentConfigs: AgentConfigs;
    agentDef: AgentDefinition | null;
  }

  const { agentName, agentConfigs, agentDef }: Props = $props();

  let configs = $derived.by(() => {
    let ac = agentConfigs;
    return ac;
  });
  let ad = $derived.by(() => {
    let ad = agentDef;
    return ad;
  });

  async function saveConfigs() {
    setGlobalConfigs(agentName, configs);
  }
</script>

<Card.Root class="@container/card">
  <Card.Header>
    <Card.Title>{ad?.title ?? agentName}</Card.Title>
    <Card.Description>{ad?.description}</Card.Description>
  </Card.Header>
  <Card.Content>
    {#if ad?.global_configs}
      <form>
        <FieldGroup>
          {#each Object.entries(ad.global_configs) as [key, globalConfig]}
            <Field
              orientation="horizontal"
              class="grid gap-4 sm:grid-cols-[220px_1fr] items-center"
            >
              <div class="flex flex-col">
                <FieldLabel>
                  {globalConfig?.title || key}
                </FieldLabel>
                <FieldDescription>{globalConfig?.description}</FieldDescription>
              </div>
              {@const ty = globalConfig.type}
              {#if ty === "boolean"}
                <Switch bind:checked={configs[key]} />
              {:else if ty === "integer"}
                <Input bind:value={configs[key]} />
              {:else if ty === "number"}
                <Input type="number" bind:value={configs[key]} />
              {:else if ty === "string"}
                <Input type="text" bind:value={configs[key]} />
              {:else if ty === "text"}
                <Input bind:value={configs[key]} />
              {:else if ty === "password"}
                <Input type="password" bind:value={configs[key]} />
              {:else if ty === "object"}
                <Input bind:value={configs[key]} />
              {:else}
                <Input type="text" value={JSON.stringify(configs[key], null, 2)} disabled />
              {/if}
            </Field>
          {/each}

          <Field orientation="responsive">
            <Button onclick={saveConfigs} variant="outline">Save</Button>
          </Field>
        </FieldGroup>
      </form>
    {/if}
  </Card.Content>
</Card.Root>
