<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { onMount } from "svelte";

  import MinusIcon from "@lucide/svelte/icons/minus";
  import CopyIcon from "@lucide/svelte/icons/copy";
  import SquareIcon from "@lucide/svelte/icons/square";
  import XIcon from "@lucide/svelte/icons/x";

  import logo from "$lib/assets/logo.png";
  import PresetActions from "$lib/components/preset-editor/preset-actions.svelte";
  import PresetName from "$lib/components/preset-editor/preset-name.svelte";
  import PresetStatus from "$lib/components/preset-status.svelte";
  import Menubar from "$lib/components/preset-editor/menubar.svelte";
  import { titlebarState } from "$lib/titlebar-state.svelte";

  const isMacos = navigator.userAgent.includes("Mac");

  let isMaximized = $state(false);
  let isFullscreen = $state(false);

  $effect(() => {
    document.documentElement.style.setProperty(
      "--titlebar-height",
      isFullscreen ? "0px" : "32px",
    );
  });

  onMount(() => {
    const currentWindow = getCurrentWindow();

    // Initialize state
    currentWindow.isMaximized().then((v) => (isMaximized = v));
    currentWindow.isFullscreen().then((v) => (isFullscreen = v));

    // Listen for resize events to track maximized/fullscreen
    const unlistenResize = currentWindow.onResized(async () => {
      isMaximized = await currentWindow.isMaximized();
      isFullscreen = await currentWindow.isFullscreen();
    });

    return () => {
      unlistenResize.then((fn) => fn());
    };
  });

  function handleMinimize() {
    getCurrentWindow().minimize();
  }

  function handleToggleMaximize() {
    getCurrentWindow().toggleMaximize();
  }

  function handleClose() {
    getCurrentWindow().close();
  }

  async function handleStart() {
    await titlebarState.onStart?.();
  }

  async function handleStop() {
    await titlebarState.onStop?.();
  }
</script>

{#if !isFullscreen}
  <div
    data-tauri-drag-region
    class="relative flex items-center h-8 bg-sidebar text-sidebar-foreground border-b border-sidebar-border select-none shrink-0"
  >
    <!-- macOS traffic light padding -->
    {#if isMacos}
      <div class="w-[70px] shrink-0"></div>
    {/if}

    <!-- Left: Logo + File menu -->
    <div class="flex items-center shrink-0 z-10">
      <div class="flex items-center px-2">
        <img src={logo} alt="logo" class="size-5" />
      </div>

      {#if titlebarState.showMenubar}
        <div>
          <Menubar
            preset_id={titlebarState.presetId}
            name={titlebarState.presetName}
            onNewPreset={titlebarState.onNewPreset ?? (() => {})}
            onSavePreset={titlebarState.onSavePreset ?? (() => {})}
            onImportPreset={titlebarState.onImportPreset ?? (() => {})}
            onExportPreset={titlebarState.onExportPreset ?? (() => {})}
          />
        </div>
      {/if}
    </div>

    <!-- Center: Preset name / Title (absolutely centered, independent of actions) -->
    <div class="absolute inset-x-0 flex items-center justify-center h-full pointer-events-none">
      {#if titlebarState.showMenubar}
        <div class="pointer-events-auto max-w-[400px] truncate">
          <PresetName name={titlebarState.presetName} />
        </div>
      {:else}
        <span class="text-sm font-semibold truncate">{titlebarState.title}</span>
      {/if}
    </div>

    <!-- Spacer (drag region) -->
    <div data-tauri-drag-region class="flex-1"></div>

    <!-- Actions + Status (right-aligned, before window controls) -->
    {#if titlebarState.showActions}
      <div class="flex items-center gap-1 shrink-0 z-10 mr-1">
        <PresetActions
          running={titlebarState.running}
          onStartPreset={handleStart}
          onStopPreset={handleStop}
        />
        <PresetStatus running={titlebarState.running} />
      </div>
    {/if}

    <!-- Window controls (Windows/Linux only) -->
    {#if !isMacos}
      <div class="flex items-center shrink-0 z-10">
        <button
          onclick={handleMinimize}
          class="inline-flex items-center justify-center h-8 w-11 hover:bg-accent hover:text-accent-foreground"
          aria-label="Minimize"
        >
          <MinusIcon class="size-4" />
        </button>
        <button
          onclick={handleToggleMaximize}
          class="inline-flex items-center justify-center h-8 w-11 hover:bg-accent hover:text-accent-foreground"
          aria-label={isMaximized ? "Restore" : "Maximize"}
        >
          {#if isMaximized}
            <CopyIcon class="size-3.5" />
          {:else}
            <SquareIcon class="size-3.5" />
          {/if}
        </button>
        <button
          onclick={handleClose}
          class="inline-flex items-center justify-center h-8 w-11 hover:bg-destructive hover:text-white"
          aria-label="Close"
        >
          <XIcon class="size-4" />
        </button>
      </div>
    {/if}
  </div>
{/if}
