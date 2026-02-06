<script lang="ts">
  import EllipsisVerticalIcon from "@lucide/svelte/icons/ellipsis-vertical";
  import PlayIcon from "@lucide/svelte/icons/play";
  import SquareIcon from "@lucide/svelte/icons/square";
  import { startPreset, stopPreset } from "tauri-plugin-modular-agent-api";

  import { getCoreSettings, setCoreSettings } from "$lib/agent";
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
    await startPreset(id);
    running = true;
  }

  async function handleStop() {
    await stopPreset(id);
    running = false;
  }

  async function handleRunOnStart() {
    // update core setting
    const coreSettings = await getCoreSettings();
    const auto_start_presets = coreSettings.auto_start_presets || [];
    if (run_on_start) {
      // remove from auto_start_presets
      const index = auto_start_presets.indexOf(name);
      if (index > -1) {
        auto_start_presets.splice(index, 1);
      }
    } else {
      // add to auto_start_presets
      auto_start_presets.push(name);
    }
    coreSettings.auto_start_presets = auto_start_presets;
    await setCoreSettings(coreSettings);
  }
</script>

<div class="flex items-center justify-end gap-2">
  {#if running}
    <Button onclick={handleStop} variant="ghost" class="w-4">
      <SquareIcon color="red" />
    </Button>
  {:else}
    <Button onclick={handleStart} variant="ghost" class="w-4">
      <PlayIcon color="blue" />
    </Button>
  {/if}

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
