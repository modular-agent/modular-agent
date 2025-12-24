<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { getContext, onMount } from "svelte";

  import { newAgentStream } from "tauri-plugin-askit-api";

  import { deserializeAgentStream, removeAgentStream, renameAgentStream } from "@/lib/agent";
  import type { TAgentStream } from "@/lib/types";

  import StreamList from "./stream-list.svelte";

  let { data } = $props();

  const streams = getContext<() => Record<string, TAgentStream>>("AgentStreams");

  let streamNames = $state.raw<{ id: string; name: string }[]>([]);

  // id -> stream activity
  let streamActivities = $state<Record<string, boolean>>({});

  function updateStreamNames() {
    streamNames = Object.entries(streams())
      .map(([id, stream]) => ({ id, name: stream.name }))
      .sort((a, b) => a.name.localeCompare(b.name));
  }

  function updateStreamActivities() {
    streamActivities = Object.fromEntries(
      Object.entries(streams()).map(([id, stream]) => [
        id,
        stream.nodes.some((node) => node.data.enabled),
      ]),
    );
  }

  async function createNewStream(name: string | null): Promise<string | null> {
    if (!name) return null;
    const stream = await newAgentStream(name);
    if (!stream) return null;
    streams()[stream.id] = deserializeAgentStream(stream);
    updateStreamNames();
    updateStreamActivities();
    return stream.id;
  }

  async function renameStream(id: string, rename: string): Promise<string | null> {
    if (!id || !rename) return null;
    const newName = await renameAgentStream(id, rename);
    if (!newName) return null;
    const stream = streams()[id];
    stream.name = newName;
    updateStreamNames();
    updateStreamActivities();
    return newName;
  }

  async function deleteStream(id: string) {
    if (!id) return;
    const stream = streams()[id];
    if (!stream) return;
    await removeAgentStream(id);
    delete streams()[id];
    updateStreamNames();
    updateStreamActivities();
  }

  onMount(() => {
    updateStreamNames();
    updateStreamActivities();

    getCurrentWindow().setTitle("Streams - Agent Stream App");
  });
</script>

<header class="flex-none h-14 items-centger"></header>
<StreamList {streamNames} {streamActivities} {createNewStream} {renameStream} {deleteStream} />
