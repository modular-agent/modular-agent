<script lang="ts">
  import type { ComponentProps } from "svelte";

  import SettingsIcon from "@lucide/svelte/icons/settings";
  import WorkflowIcon from "@lucide/svelte/icons/workflow";

  import logo from "$lib/assets/logo.png";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";

  import Attribution from "./attribution.svelte";
  import NavPresets from "./nav-presets.svelte";
  import NavSecondary from "./nav-secondary.svelte";

  const data = {
    navSecondary: [
      {
        title: "Settings",
        url: "/settings",
        icon: SettingsIcon,
      },
    ],
  };

  let { ...restProps }: ComponentProps<typeof Sidebar.Root> = $props();
</script>

<Sidebar.Root collapsible="icon" {...restProps}>
  <Sidebar.Header>
    <Sidebar.MenuItem class="flex justify-center">
      <img src={logo} alt="logo" class="size-6" />
    </Sidebar.MenuItem>
  </Sidebar.Header>
  <Sidebar.Content>
    <Sidebar.Group class="pb-0">
      <Sidebar.GroupContent>
        <Sidebar.Menu>
          <Sidebar.MenuItem>
            <Sidebar.MenuButton>
              {#snippet child({ props })}
                <a href="/open_presets" {...props}>
                  <WorkflowIcon />
                  <span>Open Presets</span>
                </a>
              {/snippet}
              {#snippet tooltipContent()}
                <span>Open Presets</span>
              {/snippet}
            </Sidebar.MenuButton>
          </Sidebar.MenuItem>
        </Sidebar.Menu>
      </Sidebar.GroupContent>
    </Sidebar.Group>
    <NavPresets />
    <NavSecondary items={data.navSecondary} class="mt-auto flex-shrink-0" />
  </Sidebar.Content>
  <Sidebar.Footer>
    <Attribution />
  </Sidebar.Footer>
</Sidebar.Root>
