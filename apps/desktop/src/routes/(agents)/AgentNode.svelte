<script lang="ts" module>
  const titleColorMap: Record<string, string> = {
    Board: "bg-green-500",
    Builtin: "bg-blue-500",
    Command: "bg-amber-500",
    Database: "bg-teal-500",
    default: "bg-purple-500",
  };

  const CONFIG_HANDLE_STYLE =
    "width: 11px; height: 11px; background-color: #000; border: 2px solid #fff;";
  const HANDLE_X_OFFSET = "-23px";
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import type { Unsubscriber } from "svelte/store";

  import {
    Handle,
    Position,
    useSvelteFlow,
    useNodeConnections,
    type NodeProps,
  } from "@xyflow/svelte";
  import { Button, Input, NumberInput, Popover, Textarea, Toggle } from "flowbite-svelte";
  import { ExclamationCircleOutline } from "flowbite-svelte-icons";
  import { getAgentSpec, setAgentConfigs } from "tauri-plugin-askit-api";
  import type { AgentConfigSpec, AgentDisplayConfigSpec } from "tauri-plugin-askit-api";

  import Messages from "@/components/Messages.svelte";
  import {
    getAgentDefinitionsContext,
    inferTypeForDisplay,
    // serializeAgentFlowNodeConfigs,
  } from "@/lib/agent";
  import {
    subscribeAgentSpecUpdatedMessage,
    subscribeDisplayMessage,
    subscribeErrorMessage,
    subscribeInputMessage,
  } from "@/lib/shared.svelte";
  import type { TAgentFlow, TAgentFlowNodeData } from "@/lib/types";

  // import type { TAgentFlowNodeConfigs, TAgentFlowNodeDisplays } from "@/lib/types";

  import NodeBase from "./NodeBase.svelte";

  type Props = NodeProps & {
    data: TAgentFlowNodeData;
    // data: {
    //   name: string;
    //   title: string | null;
    //   enabled: boolean;
    //   // inputs: string[] | null;
    //   // outputs: string[] | null;
    //   // configs: TAgentFlowNodeConfigs;
    //   // displays: TAgentFlowNodeDisplays;
    //   spec: AgentSpec;
    //   display_values: Record<string, any> | null;
    // };
  };

  let { id, data, ...props }: Props = $props();

  console.log("AgentNode data:", data);

  const agentDef = $derived(getAgentDefinitionsContext()?.[data.name]);
  const description = $derived(agentDef?.description);

  const agentConfigs = $derived(data.spec.configs ?? null);
  const agentDisplayConfigSpecs = $derived(data.spec.display_config_specs ?? null);

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
      subscribeAgentSpecUpdatedMessage(id, () => {
        getAgentSpec(id).then((spec) => {
          if (spec) {
            updateNodeData(id, { spec });
          }
        });
      }),
    );

    if (data.spec.display_config_specs) {
      // Subscribe to display messages for each display config key
      Object.entries(data.spec.display_config_specs).forEach(([key, _]) => {
        unsubscribers.push(
          subscribeDisplayMessage(id, key, (value) => {
            const newDisplay = { ...data.display_values, [key]: value };
            updateNodeData(id, { display_values: newDisplay });
          }),
        );
      });
    }

    // Add subscription for error messages
    unsubscribers.push(
      subscribeErrorMessage(id, (message) => {
        if (!message) return;
        errorMessages.push(message);
      }),
    );

    unsubscribers.push(
      subscribeInputMessage(id, ({ ch, t }) => {
        if (!ch || ch === "") return;
        inputMessage = ch;
        inputCount += 1;
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
    const newConfigs = { ...data.spec.configs, [key]: value };
    updateNodeData(id, { spec: { ...data.spec, configs: newConfigs } });
    // const sConfig = serializeAgentFlowNodeConfigs(newConfigs, agentDefaultConfigs);
    // if (sConfig) {
    //   await setAgentConfigs(id, sConfig);
    // }
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
            value={data.title ?? agentDef?.title ?? data.name}
            onblur={() => (editTitle = false)}
            onkeydown={(evt) => {
              if (evt.key === "Enter") {
                const newTitle = evt.currentTarget.value;
                if (newTitle === "" || newTitle === (agentDef?.title ?? data.name)) {
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
            onclick={() => (editTitle = true)}
            class="flex-none"
            tabindex={-1}
          >
            <h3 class="text-xl">
              {data.title ?? agentDef?.title ?? data.name}
            </h3>
          </button>
        {/if}
      {:else}
        <h3 class="text-xl">
          <s>{data.name}</s>
        </h3>
      {/if}
      {#if errorMessages.length > 0}
        <ExclamationCircleOutline id="e-{uid}" class="ml-2 pt-1 w-6 h-6 text-red-500" />
      {/if}
    </div>
  </div>
{/snippet}

{#snippet displayItem(ty: string | null, value: any)}
  {#if ty === "undefined"}
    <div class="flex-none border-1 p-2">&nbsp;</div>
  {:else if ty === "null"}
    <div class="flex-none border-1 p-2">&nbsp;</div>
  {:else if ty === "boolean"}
    {#if value}
      <div class="flex-none border-1 p-2">true</div>
    {:else}
      <div class="flex-none border-1 p-2">false</div>
    {/if}
  {:else if ty === "integer"}
    <div class="flex-none border-1 p-2">{value}</div>
  {:else if ty === "number"}
    <div class="flex-none border-1 p-2">{value}</div>
  {:else if ty === "string"}
    <Input
      type="text"
      class="nodrag nowheel flex-none text-wrap"
      {value}
      onkeydown={(evt) => {
        if (evt.ctrlKey && (evt.key === "a" || evt.key === "c")) {
          return;
        }
        evt.preventDefault();
      }}
    />
  {:else if ty === "text"}
    <Textarea
      class="nodrag nowheel flex-1 text-wrap"
      {value}
      onkeydown={(evt) => {
        if (evt.ctrlKey && (evt.key === "a" || evt.key === "c")) {
          return;
        }
        evt.preventDefault();
      }}
    />
  {:else if ty === "image"}
    <img class="flex-1 object-scale-down" src={value} alt="" />
  {:else if ty === "object"}
    <Textarea
      class="nodrag nowheel flex-1 text-wrap"
      value={JSON.stringify(value, null, 2)}
      onkeydown={(evt) => {
        if (evt.ctrlKey && (evt.key === "a" || evt.key === "c")) {
          return;
        }
        evt.preventDefault();
      }}
    />
  {:else if ty === "message" || ty === "messages"}
    <Messages messages={value} />
  {:else}
    <Textarea
      class="nodrag nowheel flex-1 text-wrap"
      value={JSON.stringify(value, null, 2)}
      onkeydown={(evt) => {
        if (evt.ctrlKey && (evt.key === "a" || evt.key === "c")) {
          return;
        }
        evt.preventDefault();
      }}
    />
  {/if}
{/snippet}

{#snippet display(key: string, value: any, display_config_spec: AgentDisplayConfigSpec | undefined)}
  {#if display_config_spec?.hideTitle !== true}
    <h3 class="flex-none">{display_config_spec?.title || key}</h3>
    <p class="flex-none text-xs text-gray-500">{display_config_spec?.description}</p>
  {/if}
  {@const ty = inferTypeForDisplay(display_config_spec, value)}
  {#if value instanceof Array && ty !== "object" && ty !== "message"}
    <div class="flex-none flex flex-col gap-2">
      {#each value as v}
        {@render displayItem(ty, v)}
      {/each}
    </div>
  {:else}
    {@render displayItem(ty, value)}
  {/if}
{/snippet}

{#snippet inputItem(key: string, value: any, config_spec: AgentConfigSpec | undefined)}
  {#if config_spec?.hidden !== true}
    <!-- {@const config = data.spec.configs?.[key]} -->
    {@const ty = config_spec?.type}
    <div class="flex-none relative flex items-center">
      <h3>{config_spec?.title || key}</h3>
      <Handle
        id="config:{key}"
        type="target"
        position={Position.Left}
        style="top: 50%; transform: translate({HANDLE_X_OFFSET}, -50%); {CONFIG_HANDLE_STYLE}"
      />
    </div>
    {#if config_spec?.description}
      <p class="flex-none text-xs text-gray-500">{config_spec?.description}</p>
    {/if}
    {#if !connectedConfigs.includes(key)}
      <!-- Not connected, show input -->
      <!-- {@const config = data.spec.configs?.[key]} -->
      {#if ty === "unit"}
        <Button color="alternative" class="flex-none" onclick={() => updateConfig(key, {})} />
      {:else if ty === "boolean"}
        <Toggle class="flex-none" checked={value} onchange={() => updateConfig(key, !value)} />
      {:else if ty === "integer"}
        <NumberInput
          class="nodrag flex-none"
          {value}
          onkeydown={(evt) => {
            if (evt.key === "Enter") {
              let intValue = parseInt(evt.currentTarget.value);
              if (!isNaN(intValue)) {
                updateConfig(key, intValue);
              }
            }
          }}
          onchange={(evt) => {
            let intValue = parseInt(evt.currentTarget.value);
            if (!isNaN(intValue)) {
              if (intValue !== value) {
                updateConfig(key, intValue);
              }
            }
          }}
        />
      {:else if ty === "number"}
        <Input
          class="nodrag flex-none"
          type="text"
          {value}
          onkeydown={(evt) => {
            if (evt.key === "Enter") {
              let numValue = parseFloat(evt.currentTarget.value);
              if (!isNaN(numValue)) {
                updateConfig(key, numValue);
              }
            }
          }}
          onchange={(evt) => {
            let numValue = parseFloat(evt.currentTarget.value);
            if (!isNaN(numValue)) {
              if (numValue !== value) {
                updateConfig(key, numValue);
              }
            }
          }}
        />
      {:else if ty === "string"}
        <Input
          class="nodrag flex-none"
          type="text"
          {value}
          onkeydown={(evt) => {
            if (evt.key === "Enter") {
              updateConfig(key, evt.currentTarget.value);
            }
          }}
          onchange={(evt) => {
            if (evt.currentTarget.value !== value) {
              updateConfig(key, evt.currentTarget.value);
            }
          }}
        />
      {:else if ty === "password"}
        <Input
          class="nodrag flex-none"
          type="password"
          {value}
          onkeydown={(evt) => {
            if (evt.key === "Enter") {
              updateConfig(key, evt.currentTarget.value);
            }
          }}
          onchange={(evt) => {
            if (evt.currentTarget.value !== value) {
              updateConfig(key, evt.currentTarget.value);
            }
          }}
        />
      {:else if ty === "text"}
        <Textarea
          class="nodrag nowheel flex-1"
          {value}
          onkeydown={(evt) => {
            if (evt.ctrlKey && evt.key === "Enter") {
              evt.preventDefault();
              updateConfig(key, evt.currentTarget.value);
            }
          }}
          onchange={(evt) => {
            if (evt.currentTarget.value !== value) {
              updateConfig(key, evt.currentTarget.value);
            }
          }}
        />
      {:else if ty === "object"}
        <Textarea
          class="nodrag nowheel flex-1"
          value={JSON.stringify(value, null, 2)}
          onkeydown={(evt) => {
            if (evt.ctrlKey && evt.key === "Enter") {
              evt.preventDefault();
              let objValue;
              try {
                objValue = JSON.parse(evt.currentTarget.value);
                updateConfig(key, objValue);
              } catch (e) {
                console.error("Invalid JSON:", e);
                return;
              }
            }
          }}
          onchange={(evt) => {
            if (evt.currentTarget.value !== value) {
              let objValue;
              try {
                objValue = JSON.parse(evt.currentTarget.value);
                updateConfig(key, objValue);
              } catch (e) {
                console.error("Invalid JSON:", e);
                return;
              }
            }
          }}
        />
      {:else}
        <Textarea class="nodrag nowheel flex-1" value={JSON.stringify(value, null, 2)} disabled />
      {/if}
    {/if}
  {/if}
{/snippet}

{#snippet contents()}
  {#if data.spec.configs}
    <form class="grow flex flex-col gap-1 pl-4 pr-4 pb-4">
      {#each Object.entries(data.spec.configs) as [key, value]}
        {@render inputItem(key, value, data.spec.config_specs?.[key])}
      {/each}
    </form>
  {/if}

  {#if data.spec.display_config_specs}
    <div class="grow flex flex-col gap-1 pl-4 pr-4 pb-4">
      {#each Object.entries(data.spec.display_config_specs) as [key, display_config_spec]}
        {@render display(key, data.display_values?.[key], display_config_spec)}
      {/each}
    </div>
  {/if}
{/snippet}

<NodeBase {id} {data} {agentDef} {titleColor} {inputCount} {title} {contents} {...props} />

{#if description || data.title}
  <Popover triggeredBy="#t-{uid}" placement="top-start" arrow={false} class="z-40">
    {#if data.title}
      <p class="text-sm font-semibold pb-1">{agentDef?.title ?? data.name}</p>
    {/if}
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
