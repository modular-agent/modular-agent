<script lang="ts">
  import DOMPurify from "dompurify";
  import { marked } from "marked";

  import { Textarea } from "$lib/components/ui/textarea/index.js";

  type Props = {
    name: string;
    value: string;
    updateConfig: (key: string, value: any) => void;
  };

  let { name, value, updateConfig }: Props = $props();

  let editing = $state(false);
  let editValue = $state("");

  let renderedHtml = $derived.by(() => {
    const raw = typeof value === "string" ? value : String(value ?? "");
    if (raw.trim() === "") {
      return '<p class="text-muted-foreground italic">Click to edit...</p>';
    }
    return DOMPurify.sanitize(marked.parse(raw) as string);
  });

  function enterEditMode() {
    editValue = typeof value === "string" ? value : String(value ?? "");
    editing = true;
  }

  function saveAndExit() {
    editing = false;
    if (editValue !== value) {
      updateConfig(name, editValue);
    }
  }

  function handleKeydown(evt: KeyboardEvent) {
    if (evt.ctrlKey && evt.key === "Enter") {
      evt.preventDefault();
      saveAndExit();
    } else if (evt.key === "Escape") {
      evt.preventDefault();
      editing = false;
    }
  }
</script>

{#if editing}
  <Textarea
    class="nodrag nowheel flex-1 shadow-none"
    spellcheck="false"
    value={editValue}
    autofocus
    oninput={(evt: Event & { currentTarget: HTMLTextAreaElement }) => {
      editValue = evt.currentTarget.value;
    }}
    onkeydown={handleKeydown}
    onblur={saveAndExit}
  />
{:else}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="nodrag nowheel flex-1 border-none shadow-none cursor-pointer agent-config-markdown"
    onclick={enterEditMode}
  >
    {@html renderedHtml}
  </div>
{/if}

<style>
  .agent-config-markdown {
    padding: 0.5rem;
    min-height: 2rem;
    max-height: 400px;
    overflow-y: auto;
    border-radius: 0.375rem;
  }

  .agent-config-markdown:hover {
    outline: 1px solid rgba(0, 0, 0, 0.15);
  }

  .agent-config-markdown :global(h1) {
    font-size: 1.5rem;
    font-weight: 700;
    margin-bottom: 0.5rem;
  }

  .agent-config-markdown :global(h2) {
    font-size: 1.25rem;
    font-weight: 600;
    margin-bottom: 0.4rem;
  }

  .agent-config-markdown :global(h3) {
    font-size: 1.1rem;
    font-weight: 600;
    margin-bottom: 0.3rem;
  }

  .agent-config-markdown :global(p) {
    margin-bottom: 0.4rem;
  }

  .agent-config-markdown :global(ul),
  .agent-config-markdown :global(ol) {
    padding-left: 1.5rem;
    margin-bottom: 0.4rem;
  }

  .agent-config-markdown :global(li) {
    margin-bottom: 0.1rem;
  }

  .agent-config-markdown :global(code) {
    background-color: rgba(0, 0, 0, 0.05);
    padding: 0.1rem 0.3rem;
    border-radius: 0.2rem;
    font-size: 0.85em;
  }

  .agent-config-markdown :global(pre) {
    background-color: rgba(0, 0, 0, 0.05);
    padding: 0.5rem;
    border-radius: 0.3rem;
    overflow-x: auto;
    margin-bottom: 0.4rem;
  }

  .agent-config-markdown :global(pre code) {
    background-color: transparent;
    padding: 0;
  }

  .agent-config-markdown :global(blockquote) {
    border-left: 3px solid rgba(0, 0, 0, 0.2);
    padding-left: 0.75rem;
    margin-left: 0;
    margin-bottom: 0.4rem;
    color: rgba(0, 0, 0, 0.6);
  }

  .agent-config-markdown :global(a) {
    color: #2563eb;
    text-decoration: underline;
  }

  .agent-config-markdown :global(table) {
    border-collapse: separate;
    border-spacing: 0;
  }

  .agent-config-markdown :global(th) {
    text-align: left;
    font-weight: 700;
    border-bottom: 1.5px solid rgb(156 163 175 / 0.6);
    padding-bottom: 0.35rem;
  }

  .agent-config-markdown :global(td) {
    text-align: left;
  }

  .agent-config-markdown :global(th),
  .agent-config-markdown :global(td) {
    padding: 0;
  }

  .agent-config-markdown :global(th:not(:first-child)),
  .agent-config-markdown :global(td:not(:first-child)) {
    padding-left: 1rem;
  }

  .agent-config-markdown :global(tbody td) {
    padding-top: 0.35rem;
  }

  .agent-config-markdown :global(hr) {
    border: none;
    border-top: 1px solid rgba(0, 0, 0, 0.15);
    margin: 0.5rem 0;
  }
</style>
