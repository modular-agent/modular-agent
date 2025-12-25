<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { getContext, onMount } from "svelte";

  import { newAgentStream } from "tauri-plugin-askit-api";

  import {
    deserializeAgentStream,
    serializeAgentStream,
    removeAgentStream,
    renameAgentStream,
    saveAgentStream,
  } from "@/lib/agent";
  import type { TAgentStream } from "@/lib/types";

  import StreamList from "./stream-list.svelte";

  let { data } = $props();

  const streams = getContext<() => Record<string, TAgentStream>>("AgentStreams");

  let streamNames = $state.raw<{ id: string; name: string; run_on_start?: boolean }[]>([]);

  function updateStreamNames() {
    streamNames = Object.entries(streams())
      .map(([id, stream]) => ({ id, name: stream.name, run_on_start: stream.run_on_start }))
      .sort((a, b) => a.name.localeCompare(b.name));
  }

  async function createNewStream(name: string | null): Promise<string | null> {
    if (!name) return null;
    const stream = await newAgentStream(name);
    if (!stream) return null;
    streams()[stream.id] = deserializeAgentStream(stream);
    updateStreamNames();
    return stream.id;
  }

  async function renameStream(id: string, rename: string): Promise<string | null> {
    if (!id || !rename) return null;
    const newName = await renameAgentStream(id, rename);
    if (!newName) return null;
    const stream = streams()[id];
    stream.name = newName;
    updateStreamNames();
    return newName;
  }

  async function deleteStream(id: string) {
    if (!id) return;
    const stream = streams()[id];
    if (!stream) return;
    await removeAgentStream(id);
    delete streams()[id];
    updateStreamNames();
  }

  async function toggleRunOnStart(id: string) {
    if (!id) return;
    const stream = streams()[id];
    if (!stream) return;
    stream.run_on_start = !stream.run_on_start;
    const s = serializeAgentStream(
      stream.id,
      stream.name,
      stream.nodes,
      stream.edges,
      stream.run_on_start,
      stream.viewport,
    );
    await saveAgentStream(s);
    streams()[id] = stream;
    updateStreamNames();
  }

  onMount(() => {
    updateStreamNames();

    getCurrentWindow().setTitle("Streams - Agent Stream App");
  });
</script>

<header class="flex-none h-14 items-center"></header>
<StreamList {streamNames} {createNewStream} {renameStream} {deleteStream} {toggleRunOnStart} />
