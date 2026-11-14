<script lang="ts">
  import type { Snippet } from "svelte";

  import { toast } from "svelte-sonner";
  import type { ModuleConfigSpec } from "tauri-plugin-modular-agent-api";

  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Textarea } from "$lib/components/ui/textarea/index.js";
  import { parseConfigInput } from "$lib/config-input";

  import { getConfigWidget } from "./custom-ui/registry";
  import MarkdownInput from "./markdown-input.svelte";

  type Props = {
    name: string;
    value: any;
    configSpec: ModuleConfigSpec | undefined;
    updateConfig: (key: string, value: any) => void;
  };

  let { name, value, configSpec, updateConfig }: Props = $props();

  // While the input is focused, the value prop is frozen at its focus-time
  // value so external config updates (wire deliveries, MCP edits, other
  // windows) can't clobber the in-progress draft; blur restores the live
  // value. One flag suffices — each instance renders a single config.
  let focused = $state(false);
  let frozen = $state<any>();

  // JSON text that was rejected. It is shown in place of the config value
  // until it is corrected or discarded with Escape, so neither blur nor an
  // external update replaces what was typed.
  let draft = $state<string>();

  // True once the field has been typed in since it got focus or since the edit
  // was cancelled. Escape has something to cancel only then, and `change` sends
  // only then: putting the config value back changes the field, so the browser
  // may still fire `change` on blur, and sending at that point would turn a
  // cancel into an update, or overwrite a config that changed since with the
  // value shown at Escape.
  let edited = false;

  // Escape drops what was typed since the last send, and a rejected draft, and
  // shows the config value (`text`) again. During IME composition Escape only
  // cancels the conversion.
  function cancelEdit(
    evt: KeyboardEvent & { currentTarget: HTMLInputElement | HTMLTextAreaElement },
    text: string,
  ) {
    if (evt.isComposing || !(edited || draft !== undefined)) return;
    evt.preventDefault();
    draft = undefined;
    frozen = value;
    // Written to the element too: when `frozen` already holds this value the
    // bound expression does not change and the typed text would stay. The
    // input event tells the field's own binding, which otherwise keeps the
    // typed text and ignores a later config update that happens to equal it.
    const field = evt.currentTarget;
    field.value = text;
    field.dispatchEvent(new Event("input", { bubbles: true }));
    edited = false;
  }

  // The number typed into `input`. A rejected entry is reported and the field
  // goes back to the config value.
  function readNumber(
    type: "integer" | "number",
    input: HTMLInputElement,
    current: unknown,
  ): number | undefined {
    const parsed = parseConfigInput(type, input.value);
    if (parsed.ok) return parsed.value as number;
    toast.error(parsed.message);
    input.value = String(current ?? "");
    return undefined;
  }

  function commitJson(key: string, text: string) {
    const parsed = parseConfigInput("json", text);
    if (!parsed.ok) {
      draft = text;
      toast.error(parsed.message);
      return;
    }
    if (draft !== undefined) {
      // The field was showing the draft, not the frozen value: show what was
      // just saved rather than the value from before the edit.
      draft = undefined;
      frozen = parsed.value;
    }
    updateConfig(key, parsed.value);
  }

  const inputRenderers: Record<string, Snippet<[string, any]>> = {
    unit: inputUnit,
    boolean: inputBoolean,
    integer: inputInteger,
    number: inputNumber,
    string: inputString,
    password: inputPassword,
    text: inputText,
    array: inputObject,
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
    value={focused ? frozen : v}
    onfocus={() => {
      frozen = v;
      focused = true;
    }}
    onblur={() => {
      focused = false;
      edited = false;
    }}
    oninput={() => (edited = true)}
    onkeydown={(evt) => {
      if (evt.key === "Enter") {
        let intValue = readNumber("integer", evt.currentTarget, v);
        if (intValue !== undefined) {
          updateConfig(key, intValue);
        }
      } else if (evt.key === "Escape") {
        cancelEdit(evt, String(v ?? ""));
      }
    }}
    onchange={(evt) => {
      if (!edited) return;
      let intValue = readNumber("integer", evt.currentTarget, v);
      if (intValue !== undefined) {
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
    value={focused ? frozen : v}
    onfocus={() => {
      frozen = v;
      focused = true;
    }}
    onblur={() => {
      focused = false;
      edited = false;
    }}
    oninput={() => (edited = true)}
    onkeydown={(evt) => {
      if (evt.key === "Enter") {
        let numValue = readNumber("number", evt.currentTarget, v);
        if (numValue !== undefined) {
          updateConfig(key, numValue);
        }
      } else if (evt.key === "Escape") {
        cancelEdit(evt, String(v ?? ""));
      }
    }}
    onchange={(evt) => {
      if (!edited) return;
      let numValue = readNumber("number", evt.currentTarget, v);
      if (numValue !== undefined) {
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
    value={focused ? frozen : v}
    onfocus={() => {
      frozen = v;
      focused = true;
    }}
    onblur={() => {
      focused = false;
      edited = false;
    }}
    oninput={() => (edited = true)}
    onkeydown={(evt) => {
      if (evt.key === "Enter") {
        updateConfig(key, evt.currentTarget.value);
      } else if (evt.key === "Escape") {
        cancelEdit(evt, String(v ?? ""));
      }
    }}
    onchange={(evt) => {
      if (edited && evt.currentTarget.value !== v) {
        updateConfig(key, evt.currentTarget.value);
      }
    }}
  />
{/snippet}

{#snippet inputPassword(key: string, v: string)}
  <Input
    class="nodrag flex-none shadow-none"
    type="password"
    value={focused ? frozen : v}
    onfocus={() => {
      frozen = v;
      focused = true;
    }}
    onblur={() => {
      focused = false;
      edited = false;
    }}
    oninput={() => (edited = true)}
    onkeydown={(evt) => {
      if (evt.key === "Enter") {
        updateConfig(key, evt.currentTarget.value);
      } else if (evt.key === "Escape") {
        cancelEdit(evt, String(v ?? ""));
      }
    }}
    onchange={(evt) => {
      if (edited && evt.currentTarget.value !== v) {
        updateConfig(key, evt.currentTarget.value);
      }
    }}
  />
{/snippet}

{#snippet inputText(key: string, v: string)}
  <Textarea
    class="nodrag nowheel flex-1 shadow-none"
    spellcheck="false"
    value={focused ? frozen : v}
    onfocus={() => {
      frozen = v;
      focused = true;
    }}
    onblur={() => {
      focused = false;
      edited = false;
    }}
    oninput={() => (edited = true)}
    onkeydown={(evt) => {
      if (evt.ctrlKey && evt.key === "Enter") {
        evt.preventDefault();
        updateConfig(key, evt.currentTarget.value);
      } else if (evt.key === "Escape") {
        cancelEdit(evt, String(v ?? ""));
      }
    }}
    onchange={(evt) => {
      if (edited && evt.currentTarget.value !== v) {
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
    aria-invalid={draft !== undefined}
    value={draft ?? JSON.stringify(focused ? frozen : v, null, 2)}
    onfocus={() => {
      frozen = v;
      focused = true;
    }}
    onblur={() => {
      focused = false;
      edited = false;
    }}
    oninput={() => (edited = true)}
    onkeydown={(evt) => {
      if (evt.ctrlKey && evt.key === "Enter") {
        evt.preventDefault();
        commitJson(key, evt.currentTarget.value);
      } else if (evt.key === "Escape") {
        cancelEdit(evt, JSON.stringify(v, null, 2) ?? "");
      }
    }}
    onchange={(evt) => {
      if (edited) {
        commitJson(key, evt.currentTarget.value);
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
  {@const Widget = configSpec && getConfigWidget(ty)}
  <div class="flex flex-col gap-1.5">
    {#if configSpec?.hide_title !== true || configSpec?.description}
      <div class="flex flex-col gap-0.5">
        {#if configSpec?.hide_title !== true}
          <div class="flex-none relative flex items-center">
            <h3>{configSpec?.title || name}</h3>
          </div>
        {/if}
        {#if configSpec?.description}
          <p class="flex-none text-xs text-muted-foreground">{configSpec?.description}</p>
        {/if}
      </div>
    {/if}
    {#if Widget && configSpec}
      <Widget configKey={name} {value} {configSpec} readonly={false} {updateConfig} />
    {:else}
      {@const renderInput = inputRenderers[ty ?? "default"] ?? inputRenderers.default}
      {@render renderInput(name, value)}
    {/if}
  </div>
{/if}
