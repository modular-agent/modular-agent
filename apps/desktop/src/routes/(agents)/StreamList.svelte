<script lang="ts">
  import { Accordion, AccordionItem } from "flowbite-svelte";

  import StreamListItems from "./StreamListItems.svelte";

  interface Props {
    streamNames: { id: string; name: string }[];
    currentStream: string;
    streamActivities: Record<string, any>;
    changeStream: (streamId: string) => void;
  }

  let { streamNames, currentStream, streamActivities, changeStream }: Props = $props();
  const directories = $derived.by(() => {
    const result: Record<string, any> = {
      ".": [], // Special directory for top-level stream (no slashes)
    };

    // Process each stream name
    for (const streamName of streamNames) {
      if (!streamName.name.includes("/")) {
        // Top-level stream, no directory
        result["."].push(streamName);
        continue;
      }

      const parts = streamName.name.split("/");
      const dir = parts[0];

      if (parts.length === 2) {
        // Direct child of the dir
        if (!result[dir]) {
          result[dir] = {
            ".": [],
          };
        }
        result[dir]["."].push(streamName);
      } else {
        // Nested stream with multiple levels
        if (!result[dir]) {
          result[dir] = {};
        }

        // Create or navigate to sub-directory
        let current = result[dir];
        for (let i = 1; i < parts.length - 1; i++) {
          const part = parts[i];
          if (!current[part]) {
            current[part] = {
              ".": [],
            };
          }
          current = current[part];
        }

        // Add stream to the deepest sub-directory
        if (!current["."]) {
          current["."] = [];
        }
        current["."].push(streamName);
      }
    }

    return result;
  });
</script>

<div class="backdrop-blur-xs">
  <h4>Streams</h4>
  <hr />
  <Accordion flush>
    {#each directories["."] as streamName}
      <button
        type="button"
        class="w-full text-left p-1 pl-3 text-gray-400 hover:text-black hover:bg-gray-200 dark:hover:bg-gray-400 flex items-center"
        onclick={() => changeStream(streamName.id)}
      >
        {#if streamName.id === currentStream}
          <span class="text-semibold text-gray-900 dark:text-white">{streamName.name}</span>
        {:else}
          <span>{streamName.name}</span>
        {/if}
        {#if streamActivities[streamName.id]}
          <span
            class="flex-none inline-block w-2 h-2 ml-1 bg-green-500 rounded-full mr-2"
            title="active"
          ></span>
        {/if}
      </button>
    {/each}

    {#each Object.keys(directories)
      .filter((key) => key !== ".")
      .sort() as dir}
      <AccordionItem
        borderBottomClass="border-b group-last:border-none"
        paddingFlush="pl-2 pr-2 py-1"
      >
        <div slot="header">
          {dir}
        </div>
        <Accordion flush>
          <StreamListItems
            directories={directories[dir]}
            {currentStream}
            {streamActivities}
            {changeStream}
          />
        </Accordion>
      </AccordionItem>
    {/each}
  </Accordion>
</div>
