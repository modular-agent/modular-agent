<script lang="ts">
  import EllipsisVerticalIcon from "@lucide/svelte/icons/ellipsis-vertical";
  import { startPatch, stopPatch } from "tauri-plugin-modular-agent-api";

  import { invalidateAll } from "$app/navigation";

  import { getCoreSettings, setCoreSettings } from "$lib/agent";
  import RunSwitch from "$lib/components/run-switch.svelte";
  import { Button } from "$lib/components/ui/button";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu";

  type Props = {
    id: string;
    name: string;
    running: boolean | undefined;
    run_on_start?: boolean | undefined;
  };

  let { id, name, running, run_on_start }: Props = $props();

  // start and stop

  async function handleStart() {
    await startPatch(id);
    running = true;
  }

  async function handleStop() {
    await stopPatch(id);
    running = false;
  }

  async function handleRunOnStart() {
    const current = getCoreSettings().auto_start_patches || [];
    const auto_start_patches = run_on_start
      ? current.filter((n) => n !== name)
      : [...current, name];
    await setCoreSettings({ auto_start_patches });
    // run_on_start is derived from core settings at load time, so re-run load
    await invalidateAll();
  }
</script>

<div class="flex items-center justify-end gap-2">
  <RunSwitch running={running ?? false} onStart={handleStart} onStop={handleStop} />

  <DropdownMenu.Root>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}
        <Button {...props} variant="ghost" size="icon" class="relative size-8 p-0">
          <span class="sr-only">Open menu</span>
          <EllipsisVerticalIcon />
        </Button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content>
      <DropdownMenu.Item onclick={handleRunOnStart}>Run on Start</DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
</div>
