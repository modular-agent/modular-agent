<script module lang="ts">
  import "../app.css";
</script>

<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { onMount } from "svelte";

  import hotkeys from "hotkeys-js";
  import { ModeWatcher } from "mode-watcher";
  import { setMode } from "mode-watcher";

  import { getCoreSettings } from "$lib/agent";
  import AppSidebar from "$lib/components/app-sidebar.svelte";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";

  import type { LayoutProps } from "./$types";

  const { children }: LayoutProps = $props();

  onMount(() => {
    const coreSettings = getCoreSettings();

    const color_mode = coreSettings.color_mode;
    if (color_mode === "light") {
      setMode("light");
    } else if (color_mode === "dark") {
      setMode("dark");
    }

    const key_fullscreen = coreSettings.shortcut_keys?.["fullscreen"];
    if (key_fullscreen) {
      hotkeys(key_fullscreen, () => {
        getCurrentWindow()
          .isFullscreen()
          .then((isFullscreen) => {
            if (isFullscreen) {
              getCurrentWindow().setFullscreen(false);
            } else {
              getCurrentWindow().setFullscreen(true);
            }
          });
      });
    }
  });
</script>

<ModeWatcher />
<Sidebar.Provider>
  <AppSidebar />
  {@render children?.()}
</Sidebar.Provider>
