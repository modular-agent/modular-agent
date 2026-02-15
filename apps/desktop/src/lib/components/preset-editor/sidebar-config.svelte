<script lang="ts">
  import type { Snippet } from "svelte";

  import type { AgentConfigSpec } from "tauri-plugin-modular-agent-api";

  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Textarea } from "$lib/components/ui/textarea/index.js";

  import MarkdownInput from "./markdown-input.svelte";

  type Props = {
    name: string;
    value: any;
    configSpec: AgentConfigSpec | undefined;
    connected: boolean;
    updateConfig: (key: string, value: any) => void;
  };

  let { name, value, configSpec, connected = false, updateConfig }: Props = $props();

  const inputRenderers: Record<string, Snippet<[string, any]>> = {
    unit: inputUnit,
    boolean: inputBoolean,
    integer: inputInteger,
    number: inputNumber,
    string: inputString,
    password: inputPassword,
    text: inputText,
    object: inputObject,
    markdown: inputMarkdown,
    default: inputDefault,
  };
</script>

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

{#snippet inputMarkdown(key: string, v: any)}
  <MarkdownInput name={key} value={v ?? ""} {updateConfig} />
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

{#if configSpec?.hidden === true || configSpec?.readonly === true}
  <!-- Do not render -->
{:else}
  {@const ty = configSpec?.type}
  <div class="flex-none relative flex items-center">
    {#if configSpec?.hide_title !== true}
      <h3>{configSpec?.title || name}</h3>
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
