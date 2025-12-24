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
  import BotIcon from "@lucide/svelte/icons/bot";
  import CatIcon from "@lucide/svelte/icons/cat";
  import DOMPurify from "dompurify";
  import { marked } from "marked";

  import * as Avatar from "$lib/components/ui/avatar/index.js";
  import * as Item from "$lib/components/ui/item/index.js";
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
        html += `<br><p><details><summary>${truncate(msg.thinking, 30)}</summary><p>${msg.thinking}</p></details></p>`;
      }
      return { role, html };
    });
  });
</script>

<div class="grid gap-2 nodrag nowheel">
  {#each msgs as message}
    <Item.Root variant="outline">
      <Item.Media>
        <Avatar.Root class="size-10">
          {#if message.role === "ai"}
            <Avatar.Fallback>
              <BotIcon />
            </Avatar.Fallback>
          {:else if message.role === "user"}
            <Avatar.Fallback>
              <CatIcon />
            </Avatar.Fallback>
          {:else}
            <Avatar.Fallback>
              {message.role}
            </Avatar.Fallback>
          {/if}
        </Avatar.Root>
      </Item.Media>
      <Item.Content>
        <Item.Description class="line-clamp-none">
          {#if message.role === "ai"}
            {@html message.html}
          {:else}
            {message.html}
          {/if}
        </Item.Description>
      </Item.Content>
    </Item.Root>
  {/each}
</div>
