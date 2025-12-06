<script lang="ts" module>
  interface Message {
    type?: string;
    role?: string;
    content?: string | string[];
    thinking?: string;
    tool_calls?: any[];
    tool_name?: string;
    data?: {
      content: string | string[];
    };
  }

  interface Props {
    messages: Message | Message[];
  }
</script>

<script lang="ts">
  import DOMPurify from "dompurify";
  import { Avatar, Card } from "flowbite-svelte";
  import { marked } from "marked";

  import { truncate } from "$lib/utils";

  let { messages }: Props = $props();

  let msgs = $derived.by(() => {
    let msgArray = Array.isArray(messages) ? messages : messages ? [messages] : [];
    return msgArray.map((msg) => {
      let role = msg.type || msg.role || "user";
      if (role === "assistant") {
        role = "ai";
      }
      let html = "";
      let content = msg.data?.content || msg.content;
      if (role === "ai") {
        if (typeof content === "string") {
          html = marked.parse(DOMPurify.sanitize(content)) as string;
        } else if (Array.isArray(content)) {
          html = marked.parse(DOMPurify.sanitize(content.join("\n\n"))) as string;
        }
      } else {
        if (typeof content === "string") {
          html = content;
        } else if (Array.isArray(content)) {
          html = content.join("\n\n");
        }
      }
      if (msg.thinking) {
        html += `<p><details><summary>${truncate(msg.thinking, 30)}</summary><p>${msg.thinking}</p></details></p>`;
      }
      return { role, html };
    });
  });
</script>

<div class="nodrag nowheel max-h-[800px] overflow-y-auto">
  {#each msgs as message}
    <Card class="mb-1 min-w-full">
      <div class="flex items-center space-x-4 rtl:space-x-reverse">
        <Avatar class="flex-none shrink-0">{message.role}</Avatar>
        <div class="grow">
          {#if message.role === "ai"}
            {@html message.html}
          {:else}
            {message.html}
          {/if}
        </div>
      </div>
    </Card>
  {/each}
</div>
