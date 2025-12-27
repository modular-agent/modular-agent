<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { onMount } from "svelte";

  import hotkeys from "hotkeys-js";
  import { ModeWatcher } from "mode-watcher";
  import { setMode } from "mode-watcher";

  import AppSidebar from "$lib/components/app-sidebar.svelte";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import { coreSettings } from "$lib/shared.svelte";

  import "../app.css";
  import type { LayoutProps } from "./$types";

  const { children }: LayoutProps = $props();

  const key_close = "Escape";
  const key_fullscreen = $derived(coreSettings.shortcut_keys?.["fullscreen"]);

  $effect(() => {
    hotkeys(key_close, () => {
      getCurrentWindow().close();
    });
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

    return () => {
      if (key_fullscreen) {
        hotkeys.unbind(key_fullscreen);
      }
    };
  });

  onMount(() => {
    const color_mode = coreSettings.color_mode;
    if (color_mode === "light") {
      setMode("light");
    } else if (color_mode === "dark") {
      setMode("dark");
    }
  });
</script>

<ModeWatcher />
<Sidebar.Provider>
  <AppSidebar />
  {@render children?.()}
</Sidebar.Provider>
