<script module lang="ts">
  import "../app.css";
</script>

<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { onMount } from "svelte";

  import { ModeWatcher } from "mode-watcher";
  import { setMode } from "mode-watcher";

  import { getCoreSettings } from "$lib/agent";
  import AppSidebar from "$lib/components/app-sidebar.svelte";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";

  import type { LayoutProps } from "./$types";

  const { children }: LayoutProps = $props();

  let fullscreenKey: string | undefined;

  onMount(() => {
    const coreSettings = getCoreSettings();

    const color_mode = coreSettings.color_mode;
    if (color_mode === "light") {
      setMode("light");
    } else if (color_mode === "dark") {
      setMode("dark");
    }

    fullscreenKey = coreSettings.shortcut_keys?.["fullscreen"];
  });

  function handleGlobalKeydown(event: KeyboardEvent) {
    if (!fullscreenKey) return;
    const mod = event.ctrlKey || event.metaKey;
    // Parse hotkeys-js format like "ctrl+r" - extract the key part after the last "+"
    const parts = fullscreenKey.split("+");
    const key = parts[parts.length - 1].toLowerCase();
    const needsMod = parts.some(
      (p) => p === "ctrl" || p === "command" || p === "meta",
    );
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
<Sidebar.Provider>
  <AppSidebar />
  {@render children?.()}
</Sidebar.Provider>
