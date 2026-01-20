<script lang="ts">
  import type { Snippet } from "svelte";

  import { Handle, Position } from "@xyflow/svelte";
  import type { AgentConfigSpec } from "tauri-plugin-mak-api";

  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Textarea } from "$lib/components/ui/textarea/index.js";

  import { inferTypeForDisplay } from "@/lib/agent";

  import Messages from "./messages.svelte";

  type Props = {
    name: string;
    value: any;
    configSpec: AgentConfigSpec | undefined;
    connected: boolean;
    updateConfig: (key: string, value: any) => void;
  };

  let { name, value, configSpec, connected = false, updateConfig }: Props = $props();

  const CONFIG_HANDLE_STYLE =
    "width: 10px; height: 10px; background-color: #000; border: 2px solid #fff;";

  const HANDLE_X_OFFSET = "-12px";

  const displayRenderers: Record<string, Snippet<[any]>> = {
    undefined: displayEmpty,
    null: displayEmpty,
    boolean: displayBoolean,
    integer: displayNumber,
    number: displayNumber,
    string: displayString,
    password: displayPassword,
    text: displayText,
    html: displayHtml,
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
  <div class="flex-none border-none p-2">&nbsp;</div>
{/snippet}

{#snippet displayBoolean(value: boolean)}
  {#if value}
    <div class="flex-none border-none p-2">true</div>
  {:else}
    <div class="flex-none border-none p-2">false</div>
  {/if}
{/snippet}

{#snippet displayNumber(value: number)}
  <div class="flex-none border-none p-2">{value}</div>
{/snippet}

{#snippet displayString(value: string)}
  <Input
    type="text"
    class="nodrag nowheel flex-none border-none shadow-none text-wrap"
    spellcheck="false"
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
    class="nodrag flex-none border-none shadow-none"
    type="password"
    {value}
    onkeydown={(evt) => {
      evt.preventDefault();
    }}
  />
{/snippet}

{#snippet displayText(value: string)}
  <Textarea
    class="nodrag nowheel flex-1 border-none shadow-none text-wrap"
    spellcheck="false"
    {value}
    onkeydown={(evt) => {
      if (evt.ctrlKey && (evt.key === "a" || evt.key === "c")) {
        return;
      }
      evt.preventDefault();
    }}
  />
{/snippet}

{#snippet displayHtml(value: any)}
  <div class="nodrag nowheel flex-1 border-none shadow-none agent-config-html">
    {@html typeof value === "string" ? value : String(value ?? "")}
  </div>
{/snippet}

{#snippet displayImage(value: string)}
  <img class="flex-1 object-scale-down" src={value} alt="" />
{/snippet}

{#snippet displayObject(value: any)}
  <Textarea
    class="nodrag nowheel flex-1 text-wrap"
    spellcheck="false"
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

{#snippet display(name: string, v: any, configSpec: AgentConfigSpec | undefined)}
  {#if configSpec?.hide_title !== true}
    <h3 class="flex-none">{configSpec?.title || name}</h3>
    <p class="flex-none text-xs text-gray-500">{configSpec?.description}</p>
  {/if}
  {@const ty = inferTypeForDisplay(configSpec, v)}
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
  <Button class="flex-none" onclick={() => updateConfig(key, {})} variant="outline" />
{/snippet}

{#snippet inputBoolean(key: string, v: boolean)}
  <Switch class="flex-none" checked={v} onCheckedChange={() => updateConfig(key, !v)} />
{/snippet}

{#snippet inputInteger(key: string, v: number)}
  <Input
    type="number"
    class="nodrag flex-none shadow-none"
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
    class="nodrag flex-none shadow-none"
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
    class="nodrag flex-none shadow-none"
    spellcheck="false"
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
    class="nodrag flex-none shadow-none"
    type="password"
    value={v}
    onkeydown={(evt) => {
      if (evt.key === "Enter") {
        updateConfig(key, evt.currentTarget.value);
      }
    }}
  />
{/snippet}

{#snippet inputText(key: string, v: string)}
  <Textarea
    class="nodrag nowheel flex-1 shadow-none"
    spellcheck="false"
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
    class="nodrag nowheel flex-1 shadow-none"
    spellcheck="false"
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
  <Textarea
    class="nodrag nowheel flex-1 shadow-none"
    spellcheck="false"
    value={JSON.stringify(v, null, 2)}
    disabled
  />
{/snippet}

{#if configSpec?.hidden === true}
  <!-- Hidden, do not render anything -->
{:else if configSpec?.readonly === true}
  {@render display(name, value, configSpec)}
{:else}
  {@const ty = configSpec?.type}
  <div class="flex-none relative flex items-center">
    {#if configSpec?.hide_title !== true}
      <h3>{configSpec?.title || name}</h3>
      <Handle
        id="config:{name}"
        type="target"
        position={Position.Left}
        style={`top: 50%; transform: translate(${HANDLE_X_OFFSET}, -50%); ${CONFIG_HANDLE_STYLE}`}
      />
    {/if}
  </div>
  {#if configSpec?.description}
    <p class="flex-none text-xs text-gray-500">{configSpec?.description}</p>
  {/if}
  {#if !connected}
    {@const renderInput = inputRenderers[ty ?? "default"] ?? inputRenderers.default}
    {@render renderInput(name, value)}
  {/if}
{/if}

<style>
  .agent-config-html :global(table) {
    border-collapse: separate;
    border-spacing: 0;
  }

  .agent-config-html :global(th) {
    text-align: left;
    font-weight: 700;
    border-bottom: 1.5px solid rgb(156 163 175 / 0.6);
    padding-bottom: 0.35rem;
  }

  .agent-config-html :global(td) {
    text-align: left;
  }

  .agent-config-html :global(th),
  .agent-config-html :global(td) {
    padding: 0;
  }

  .agent-config-html :global(th:not(:first-child)),
  .agent-config-html :global(td:not(:first-child)) {
    padding-left: 1rem;
  }

  .agent-config-html :global(tbody td) {
    padding-top: 0.35rem;
  }
</style>
