<script lang="ts" module>
  const titleColorMap: Record<string, string> = {
    default: "bg-agent-1",
    External: "bg-agent-2",
    Local: "bg-agent-2",
    Display: "bg-agent-3",
    Input: "bg-agent-3",
    UI: "bg-agent-4",
  };
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import type { Unsubscriber } from "svelte/store";

  import AlertCircleIcon from "@lucide/svelte/icons/alert-circle";
  import { useSvelteFlow, useNodeConnections, type NodeProps } from "@xyflow/svelte";
  import { getAgentSpec, setAgentConfigs } from "tauri-plugin-modular-agent-api";
  import type { AgentSpec } from "tauri-plugin-modular-agent-api";

  import { getAgentDefinitions } from "$lib/agent";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as HoverCard from "$lib/components/ui/hover-card/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import {
    subscribeAgentSpecUpdatedMessage,
    subscribeAgentConfigUpdatedMessage,
    subscribeAgentErrorMessage,
    subscribeAgentInMessage,
  } from "$lib/shared.svelte";

  import AgentConfig from "./agent-config.svelte";
  import NodeBase from "./node-base.svelte";

  type Props = NodeProps & {
    id: string;
    data: AgentSpec;
  };

  let { id, data, ...props }: Props = $props();

  const agentDefs = getAgentDefinitions();
  const agentDef = $derived(agentDefs[data?.def_name] ?? null);
  // const description = $derived(agentDef.description);

  let errorMessages = $state<string[]>([]);
  let inputMessage = $state<string>("");
  let inputCount = $state(0);

  const connections = useNodeConnections({ handleType: "target" });

  let connectedConfigs = $derived(
    connections.current
      .filter((c) => c.target === id && c.targetHandle?.startsWith("config:"))
      .map((c) => c.targetHandle?.substring(7) ?? ""),
  );

  onMount(() => {
    let unsubscribers: Unsubscriber[] = [];

    unsubscribers.push(
      subscribeAgentConfigUpdatedMessage(id, ({ key, value }) => {
        // TODO: validate key and value
        if (!key) return;
        let currentValue = data.configs?.[key];
        if (currentValue === value) {
          return;
        }
        const newConfigs = { ...data.configs, [key]: value };
        updateNodeData(id, { ...data, configs: newConfigs });
      }),
    );

    unsubscribers.push(
      subscribeAgentErrorMessage(id, (message) => {
        if (!message) return;
        errorMessages.push(message);
      }),
    );

    unsubscribers.push(
      subscribeAgentInMessage(id, ({ port, t }) => {
        if (!port || port === "") return;
        inputMessage = port;
        inputCount += 1;
      }),
    );

    unsubscribers.push(
      subscribeAgentSpecUpdatedMessage(id, () => {
        getAgentSpec(id).then((spec) => {
          if (spec) {
            updateNodeData(id, { ...spec });
          }
        });
      }),
    );

    return () => {
      for (const unsub of unsubscribers) {
        unsub();
      }
    };
  });

  const { updateNodeData } = useSvelteFlow();

  async function updateConfig(key: string, value: any) {
    // TODO: validate key and value
    // let currentValue = data.configs?.[key];
    const newConfigs = { ...data.configs, [key]: value };
    updateNodeData(id, { ...data, configs: newConfigs });
    await setAgentConfigs(id, newConfigs);
  }

  function clearError() {
    errorMessages = [];
  }

  let hide_title = $derived(agentDef?.hide_title ?? false);
  let editTitle = $state(false);
  let titleColor = $derived(titleColorMap[agentDef?.kind ?? "default"] ?? titleColorMap.default);

  const uid = $props.id();
</script>

{#snippet title()}
  {#if hide_title}
    <div class="flex-none mt-1">
      <div class="flex flex-col flex-nowrap items-start"></div>
    </div>
  {:else}
    <div class="flex-none mt-1">
      <div class="flex flex-col flex-nowrap items-start">
        {#if agentDef}
          <div class="text-xs">
            {#if agentDef.category}
              {agentDef.category}
            {/if}
          </div>
          <div class="flex flex-row space-x-2">
            {#if editTitle}
              <Input
                class="text-left"
                type="text"
                value={data.title ?? agentDef.title ?? data.def_name}
                autofocus
                onblur={() => (editTitle = false)}
                onkeydown={(evt) => {
                  if (evt.key === "Enter") {
                    const newTitle = evt.currentTarget.value;
                    if (newTitle === "" || newTitle === (agentDef.title ?? data.def_name)) {
                      updateNodeData(id, { title: null });
                    } else if (newTitle !== data.title) {
                      updateNodeData(id, { title: newTitle });
                    }
                    editTitle = false;
                  }
                }}
              />
            {:else}
              <button
                type="button"
                ondblclick={() => (editTitle = true)}
                class="flex-none"
                tabindex={-1}
              >
                <div class="text-xl font-semibold">
                  {data.title ?? agentDef.title ?? data.def_name}
                </div>
              </button>
              {#if errorMessages.length > 0}
                <HoverCard.Root>
                  <HoverCard.Trigger class="ml-4">
                    <AlertCircleIcon color="red" />
                  </HoverCard.Trigger>
                  <HoverCard.Content class="w-full max-w-xl">
                    <div class="flex flex-col gap-2 mb-2">
                      {#each errorMessages as msg}
                        <Alert.Root variant="destructive">
                          <Alert.Description>
                            <div>{msg}</div>
                          </Alert.Description>
                        </Alert.Root>
                      {/each}
                    </div>
                    <Button onclick={clearError} variant="outline">Clear</Button>
                  </HoverCard.Content>
                </HoverCard.Root>
              {/if}
            {/if}
          </div>
        {:else}
          <h3 class="text-xl">
            <s>{data.def_name}</s>
          </h3>
        {/if}
      </div>
    </div>
  {/if}
{/snippet}

{#snippet contents()}
  {#if data.configs}
    <form class="grow flex flex-col gap-1 pl-4 pr-4 pb-4">
      {#each Object.entries(data.configs) as [key, value]}
        <AgentConfig
          name={key}
          {value}
          configSpec={data.config_specs?.[key]}
          connected={connectedConfigs.includes(key)}
          {updateConfig}
        />
      {/each}
    </form>
  {/if}
{/snippet}

<NodeBase {id} {data} {agentDef} {inputCount} {title} {titleColor} {contents} {...props} />
