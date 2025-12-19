<script lang="ts" module>
  const titleColorMap: Record<string, string> = {
    Board: "bg-green-500",
    Builtin: "bg-blue-500",
    Command: "bg-amber-500",
    Database: "bg-teal-500",
    default: "bg-purple-500",
  };
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import type { Unsubscriber } from "svelte/store";

  import { useSvelteFlow, useNodeConnections, type NodeProps } from "@xyflow/svelte";
  import { Button, Input, Popover, Textarea } from "flowbite-svelte";
  import { ExclamationCircleOutline } from "flowbite-svelte-icons";
  import { getAgentSpec, setAgentConfigs } from "tauri-plugin-askit-api";
  import type { AgentSpec } from "tauri-plugin-askit-api";

  import { getAgentDefinitionsContext } from "@/lib/agent";
  import {
    subscribeAgentSpecUpdatedMessage,
    subscribeAgentConfigUpdatedMessage,
    subscribeAgentErrorMessage,
    subscribeAgentInMessage,
  } from "@/lib/shared.svelte";

  import AgentConfig from "./AgentConfig.svelte";
  import NodeBase from "./NodeBase.svelte";

  type Props = NodeProps & {
    data: AgentSpec;
  };

  let { id, data, ...props }: Props = $props();

  const agentDef = $derived.by(() => getAgentDefinitionsContext()?.[data.def_name]);
  const description = $derived(agentDef?.description);

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
      subscribeAgentInMessage(id, ({ ch, t }) => {
        if (!ch || ch === "") return;
        inputMessage = ch;
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
    let currentValue = data.configs?.[key];
    const newConfigs = { ...data.configs, [key]: value };
    updateNodeData(id, { ...data, configs: newConfigs });
    await setAgentConfigs(id, newConfigs);
  }

  function clearError() {
    errorMessages = [];
  }

  let editTitle = $state(false);
  let titleColor = $derived(titleColorMap[agentDef?.kind ?? "default"] ?? titleColorMap.default);

  const uid = $props.id();
</script>

{#snippet title()}
  <div class="flex-none mt-1">
    <div class="flex flex-nowrap">
      {#if agentDef}
        {#if editTitle}
          <Input
            class="mt-1"
            type="text"
            value={data.title ?? agentDef?.title ?? data.def_name}
            autofocus
            onblur={() => (editTitle = false)}
            onkeydown={(evt) => {
              if (evt.key === "Enter") {
                const newTitle = evt.currentTarget.value;
                if (newTitle === "" || newTitle === (agentDef?.title ?? data.def_name)) {
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
            id="t-{uid}"
            type="button"
            ondblclick={() => (editTitle = true)}
            class="flex-none"
            tabindex={-1}
          >
            <h3 class="text-xl">
              {data.title ?? agentDef?.title ?? data.def_name}
            </h3>
          </button>
        {/if}
      {:else}
        <h3 class="text-xl">
          <s>{data.def_name}</s>
        </h3>
      {/if}
      {#if errorMessages.length > 0}
        <ExclamationCircleOutline id="e-{uid}" class="ml-2 pt-1 w-6 h-6 text-red-500" />
      {/if}
    </div>
  </div>
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

<NodeBase {id} {data} {agentDef} {titleColor} {inputCount} {title} {contents} {...props} />

{#if description || data.def_name || agentDef?.category}
  <Popover triggeredBy="#t-{uid}" placement="top-start" arrow={false} class="z-40">
    <p class="text-sm font-semibold">
      {data.def_name}<br />
      {#if agentDef?.title}
        {agentDef.title}
      {/if}
      {#if agentDef?.category}
        ({agentDef.category})
      {/if}
    </p>
    {#if description}
      <p class="text-sm text-gray-500">{description}</p>
    {/if}
  </Popover>
{/if}

{#if errorMessages.length > 0}
  <Popover
    triggeredBy="#e-{uid}"
    placement="bottom"
    arrow={false}
    class="w-96 min-h-60 z-40 text-xs font-light text-gray-500 bg-white dark:bg-gray-800 dark:border-gray-600 dark:text-gray-400 flex flex-col"
  >
    <div class="grow flex flex-col gap-2">
      <Textarea
        class="grow nodrag nowheel text-wrap"
        value={errorMessages.join("\n")}
        onkeydown={(evt) => {
          evt.preventDefault();
        }}
        rows={8}
      />
      <Button size="xs" color="red" class="w-10 flex-none" onclick={clearError}>Clear</Button>
    </div>
  </Popover>
{/if}
