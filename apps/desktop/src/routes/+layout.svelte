<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { setContext } from "svelte";

  import hotkeys from "hotkeys-js";

  import AppSidebar from "$lib/components/app-sidebar.svelte";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";

  import "../app.css";
  import type { LayoutProps } from "./$types";

  const { children, data }: LayoutProps = $props();

  setContext("coreSettings", () => data.coreSettings);
  setContext("AgentStreams", () => data.AgentStreams);

  const key_close = "Escape";
  const key_fullscreen = $derived(data.coreSettings.shortcut_keys["fullscreen"]);

  $effect(() => {
    hotkeys(key_close, () => {
      getCurrentWindow().close();
    });
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

    return () => {
      hotkeys.unbind(key_fullscreen);
    };
  });
</script>

<Sidebar.Provider>
  <AppSidebar />
  {@render children?.()}
</Sidebar.Provider>
