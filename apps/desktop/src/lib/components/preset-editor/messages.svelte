<script lang="ts" module>
  import type { MessageContent } from "./message-content";

  interface Message {
    type?: string;
    role?: string;
    content?: MessageContent;
    // Legacy top-level thinking key from old presets/histories.
    thinking?: string;
    tool_calls?: any[];
    tool_name?: string;
    image?: string;
    data?: {
      content: MessageContent;
    };
  }

  interface Props {
    messages: Message | Message[];
  }
</script>

<script lang="ts">
  import BotIcon from "@lucide/svelte/icons/bot";
  import CatIcon from "@lucide/svelte/icons/cat";
  import ScrollTextIcon from "@lucide/svelte/icons/scroll-text";
  import { escapeHtml, isSafeImageSrc, renderMarkdown, sanitizeHtml } from "$lib/sanitize";

  import * as Avatar from "$lib/components/ui/avatar/index.js";
  import * as Item from "$lib/components/ui/item/index.js";
  import { truncate } from "$lib/modular_agent";
  import { extractImageSrcs, extractText, extractThinking } from "./message-content";

  let { messages }: Props = $props();

  let msgs = $derived.by(() => {
    let msgArray = Array.isArray(messages) ? messages : messages ? [messages] : [];
    return msgArray
      .map((msg) => {
        let role = msg.type || msg.role || "user";
        if (role === "assistant") {
          role = "ai";
        }
        let html = "";
        let content = msg.data?.content || msg.content;
        let text = extractText(content);
        // Thinking blocks take priority; legacy messages carry a top-level key.
        let thinking = extractThinking(content) ?? msg.thinking;
        if (thinking) {
          const open = text ? "" : "open";
          html += `<p><details ${open}><summary>${escapeHtml(truncate(thinking, 30))}</summary><p>${escapeHtml(thinking)}</p></details></p><br/>`;
        }
        if (role === "ai") {
          html += renderMarkdown(text);
          if (msg.tool_calls && msg.tool_calls.length > 0) {
            for (const toolCall of msg.tool_calls) {
              html += `<p><details><summary>Tool Call: ${escapeHtml(toolCall?.function?.name ?? "unknown")}</summary><pre>${sanitizeHtml(
                JSON.stringify(toolCall, null, 2),
              )}</pre></details></p><br/>`;
            }
          }
        } else if (role === "tool") {
          html += `<p><details open><summary>Tool Response: ${escapeHtml(msg.tool_name || "unknown")}</summary><pre>${sanitizeHtml(text)}</pre></details></p><br/>`;
        } else {
          html += sanitizeHtml(JSON.stringify(text, null, 2));
        }
        return { role, html, image: msg.image, blockImages: extractImageSrcs(content) };
      })
      .filter(({ role }) => role !== "system");
  });
</script>

<div class="grid gap-2 nodrag nowheel">
  {#each msgs as message}
    <Item.Root variant="outline" class="flex-nowrap items-start">
      <Item.Media>
        <Avatar.Root class="size-10">
          {#if message.role === "ai"}
            <Avatar.Fallback class="bg-background">
              <BotIcon />
            </Avatar.Fallback>
          {:else if message.role === "user"}
            <Avatar.Fallback class="bg-background">
              <CatIcon />
            </Avatar.Fallback>
          {:else if message.role === "system"}
            <Avatar.Fallback class="bg-background">
              <ScrollTextIcon />
            </Avatar.Fallback>
          {:else}
            <Avatar.Fallback>
              {message.role}
            </Avatar.Fallback>
          {/if}
        </Avatar.Root>
      </Item.Media>
      <Item.Content class="min-w-0">
        <div class="text-sm leading-normal font-normal text-primary select-text cursor-text message-content">
          {#if message.role === "ai" || message.role === "tool"}
            {@html message.html}
          {:else}
            {message.html}
          {/if}
          {#each message.blockImages as src}
            {#if src && isSafeImageSrc(src)}
              <div class="mt-2">
                <img {src} alt="" class="max-w-full p-2" />
              </div>
            {:else}
              <div class="mt-2 text-xs text-muted-foreground">[image unavailable]</div>
            {/if}
          {/each}
          {#if message.image && isSafeImageSrc(message.image)}
            <div class="mt-2">
              <img src={message.image} alt="" class="max-w-full p-2" />
            </div>
          {/if}
        </div>
      </Item.Content>
    </Item.Root>
  {/each}
</div>

<style>
  .message-content :global(code) {
    background-color: var(--muted);
    padding: 0.1rem 0.3rem;
    border-radius: 0.2rem;
    font-size: 0.85em;
  }
  .message-content :global(pre) {
    background-color: var(--muted);
    padding: 0.5rem;
    border-radius: 0.3rem;
    overflow-x: auto;
    margin-bottom: 0.4rem;
  }
  .message-content :global(pre code) {
    background-color: transparent;
    padding: 0;
  }
  .message-content :global(a) {
    color: var(--link-color);
    text-decoration: underline;
  }
  .message-content :global(a:hover) {
    opacity: 0.8;
  }
  .message-content :global(blockquote) {
    border-left: 3px solid var(--border);
    padding-left: 0.75rem;
    margin-left: 0;
    margin-bottom: 0.4rem;
    color: var(--muted-foreground);
  }
  .message-content :global(p) {
    margin-bottom: 0.4rem;
  }
</style>
