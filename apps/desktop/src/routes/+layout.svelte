<script module lang="ts">
  import "../app.css";
</script>

<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { onMount } from "svelte";

  import { ModeWatcher } from "mode-watcher";
  import { setMode } from "mode-watcher";

  import { getCoreSettings } from "$lib/agent";
  import { Toaster } from "$lib/components/ui/sonner";
  import AppSidebar from "$lib/components/app-sidebar.svelte";
  import Titlebar from "$lib/components/titlebar.svelte";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import { initLogging } from "$lib/log-store.svelte";

  import type { LayoutProps } from "./$types";

  const { children }: LayoutProps = $props();

  let fullscreenKey: string | undefined;
  let isFullscreen = $state(false);

  onMount(() => {
    const coreSettings = getCoreSettings();

    const color_mode = coreSettings.color_mode;
    if (color_mode === "light") {
      setMode("light");
    } else if (color_mode === "dark") {
      setMode("dark");
    }

    fullscreenKey = coreSettings.shortcut_keys?.["fullscreen"];

    const currentWindow = getCurrentWindow();
    currentWindow.isFullscreen().then((v) => (isFullscreen = v));

    const unlistenResize = currentWindow.onResized(async () => {
      isFullscreen = await currentWindow.isFullscreen();
    });

    const cleanupLogging = initLogging();

    return () => {
      cleanupLogging.then((fn) => fn());
      unlistenResize.then((fn) => fn());
    };
  });

  function handleGlobalKeydown(event: KeyboardEvent) {
    // Block sidebar toggle (Ctrl+B) during fullscreen
    if (isFullscreen && event.key.toLowerCase() === "b" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      return;
    }

    if (!fullscreenKey) return;
    const mod = event.ctrlKey || event.metaKey;
    // Parse hotkeys-js format like "ctrl+r" - extract the key part after the last "+"
    const parts = fullscreenKey.split("+");
    const key = parts[parts.length - 1].toLowerCase();
    const needsMod = parts.some((p) => p === "ctrl" || p === "command" || p === "meta");
    if (needsMod && !mod) return;
    if (!needsMod && mod) return;
    if (event.key.toLowerCase() === key) {
      event.preventDefault();
      getCurrentWindow()
        .isFullscreen()
        .then((isFullscreen) => {
          getCurrentWindow().setFullscreen(!isFullscreen);
        });
    }
  }
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<ModeWatcher />
<Toaster />
<div class="flex flex-col h-screen overflow-hidden">
  <Titlebar />
  <Sidebar.Provider>
    {#if !isFullscreen}
      <AppSidebar />
    {/if}
    {@render children?.()}
  </Sidebar.Provider>
</div>
