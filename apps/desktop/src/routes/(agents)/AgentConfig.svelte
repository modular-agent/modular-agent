<script lang="ts">
  import type { Snippet } from "svelte";

  import { Handle, Position } from "@xyflow/svelte";
  import { Button, Input, NumberInput, Textarea, Toggle } from "flowbite-svelte";
  import type { AgentConfigSpec } from "tauri-plugin-askit-api";

  import Messages from "@/components/Messages.svelte";
  import { inferTypeForDisplay } from "@/lib/agent";

  export let name: string;
  export let value: any;
  export let configSpec: AgentConfigSpec | undefined;
  export let connected = false;
  export let updateConfig: (key: string, value: any) => void;

  const CONFIG_HANDLE_STYLE =
    "width: 11px; height: 11px; background-color: #000; border: 2px solid #fff;";

  const HANDLE_X_OFFSET = "-23px";

  const displayRenderers: Record<string, Snippet<[any]>> = {
    undefined: displayEmpty,
    null: displayEmpty,
    boolean: displayBoolean,
    integer: displayNumber,
    number: displayNumber,
    string: displayString,
    password: displayPassword,
    text: displayText,
    image: displayImage,
    object: displayObject,
    message: displayMessages,
    messages: displayMessages,
    default: displayObject,
  };

  const inputRenderers: Record<string, Snippet<[string, any]>> = {
    unit: inputUnit,
    boolean: inputBoolean,
    integer: inputInteger,
    number: inputNumber,
    string: inputString,
    password: inputPassword,
    text: inputText,
    object: inputObject,
    default: inputDefault,
  };
</script>

{#snippet displayEmpty()}
  <div class="flex-none border-1 p-2">&nbsp;</div>
{/snippet}

{#snippet displayBoolean(value: boolean)}
  {#if value}
    <div class="flex-none border-1 p-2">true</div>
  {:else}
    <div class="flex-none border-1 p-2">false</div>
  {/if}
{/snippet}

{#snippet displayNumber(value: number)}
  <div class="flex-none border-1 p-2">{value}</div>
{/snippet}

{#snippet displayString(value: string)}
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
{/snippet}

{#snippet displayPassword(value: string)}
  <Input
    class="nodrag flex-none"
    type="password"
    {value}
    onkeydown={(evt) => {
      evt.preventDefault();
    }}
  />
{/snippet}

{#snippet displayText(value: string)}
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
{/snippet}

{#snippet displayImage(value: string)}
  <img class="flex-1 object-scale-down" src={value} alt="" />
{/snippet}

{#snippet displayObject(value: any)}
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
{/snippet}

{#snippet displayMessages(value: any)}
  <Messages messages={value} />
{/snippet}

{#snippet displayItem(ty: string | null, v: any)}
  {@const renderer = displayRenderers[ty ?? "default"] ?? displayRenderers.default}
  {@render renderer(v)}
{/snippet}

{#snippet display(name: string, v: any, config_spec: AgentConfigSpec | undefined)}
  {#if config_spec?.hideTitle !== true}
    <h3 class="flex-none">{config_spec?.title || name}</h3>
    <p class="flex-none text-xs text-gray-500">{config_spec?.description}</p>
  {/if}
  {@const ty = inferTypeForDisplay(config_spec, v)}
  {#if v instanceof Array && ty !== "object" && ty !== "message"}
    <div class="flex-none flex flex-col gap-2">
      {#each v as item}
        {@render displayItem(ty, item)}
      {/each}
    </div>
  {:else}
    {@render displayItem(ty, v)}
  {/if}
{/snippet}

{#snippet inputUnit(key: string, v: any)}
  <Button color="alternative" class="flex-none" onclick={() => updateConfig(key, {})} />
{/snippet}

{#snippet inputBoolean(key: string, v: boolean)}
  <Toggle class="flex-none" checked={v} onchange={() => updateConfig(key, !v)} />
{/snippet}

{#snippet inputInteger(key: string, v: number)}
  <NumberInput
    class="nodrag flex-none"
    value={v}
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
        if (intValue !== v) {
          updateConfig(key, intValue);
        }
      }
    }}
  />
{/snippet}

{#snippet inputNumber(key: string, v: number)}
  <Input
    class="nodrag flex-none"
    type="text"
    value={v}
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
        if (numValue !== v) {
          updateConfig(key, numValue);
        }
      }
    }}
  />
{/snippet}

{#snippet inputString(key: string, v: string)}
  <Input
    class="nodrag flex-none"
    type="text"
    value={v}
    onkeydown={(evt) => {
      if (evt.key === "Enter") {
        updateConfig(key, evt.currentTarget.value);
      }
    }}
    onchange={(evt) => {
      if (evt.currentTarget.value !== v) {
        updateConfig(key, evt.currentTarget.value);
      }
    }}
  />
{/snippet}

{#snippet inputPassword(key: string, v: string)}
  <Input
    class="nodrag flex-none"
    type="password"
    value={v}
    onkeydown={(evt) => {
      if (evt.key === "Enter") {
        updateConfig(key, evt.currentTarget.value);
      }
    }}
    onchange={(evt) => {
      if (evt.currentTarget.value !== v) {
        updateConfig(key, evt.currentTarget.value);
      }
    }}
  />
{/snippet}

{#snippet inputText(key: string, v: string)}
  <Textarea
    class="nodrag nowheel flex-1"
    value={v}
    onkeydown={(evt) => {
      if (evt.ctrlKey && evt.key === "Enter") {
        evt.preventDefault();
        updateConfig(key, evt.currentTarget.value);
      }
    }}
    onchange={(evt) => {
      if (evt.currentTarget.value !== v) {
        updateConfig(key, evt.currentTarget.value);
      }
    }}
  />
{/snippet}

{#snippet inputObject(key: string, v: any)}
  <Textarea
    class="nodrag nowheel flex-1"
    value={JSON.stringify(v, null, 2)}
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
      if (evt.currentTarget.value !== v) {
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
{/snippet}

{#snippet inputDefault(key: string, v: any)}
  <Textarea class="nodrag nowheel flex-1" value={JSON.stringify(v, null, 2)} disabled />
{/snippet}

{#if configSpec?.hidden === true}
  <!-- Hidden, do not render anything -->
{:else if configSpec?.readonly === true}
  {@render display(name, value, configSpec)}
{:else}
  {@const ty = configSpec?.type}
  <div class="flex-none relative flex items-center">
    <h3>{configSpec?.title || name}</h3>
    <Handle
      id="config:{name}"
      type="target"
      position={Position.Left}
      style={`top: 50%; transform: translate(${HANDLE_X_OFFSET}, -50%); ${CONFIG_HANDLE_STYLE}`}
    />
  </div>
  {#if configSpec?.description}
    <p class="flex-none text-xs text-gray-500">{configSpec?.description}</p>
  {/if}
  {#if !connected}
    {@const renderInput = inputRenderers[ty ?? "default"] ?? inputRenderers.default}
    {@render renderInput(name, value)}
  {/if}
{/if}
