<script lang="ts">
  import type { ComponentProps } from "svelte";
  import { onMount } from "svelte";

  import SettingsIcon from "@lucide/svelte/icons/settings";
  import WorkflowIcon from "@lucide/svelte/icons/workflow";

  import logo from "$lib/assets/logo.png";
  import { useSidebar } from "$lib/components/ui/sidebar/context.svelte.js";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";

  import Attribution from "./attribution.svelte";
  import NavMain from "./nav-main.svelte";
  import NavSecondary from "./nav-secondary.svelte";

  const data = {
    navMain: [
      {
        title: "Presets",
        url: "/presets",
        icon: WorkflowIcon,
      },
      {
        title: "Open Presets",
        url: "/open_presets",
        icon: WorkflowIcon,
      },
    ],
    navSecondary: [
      {
        title: "Settings",
        url: "/settings",
        icon: SettingsIcon,
      },
    ],
  };

  let { ...restProps }: ComponentProps<typeof Sidebar.Root> = $props();

  let sidebar = useSidebar();

  onMount(() => {
    sidebar.setOpen(false);
  });
</script>

<Sidebar.Root collapsible="icon" {...restProps}>
  <Sidebar.Header>
    <Sidebar.MenuItem class="flex justify-center">
      <img src={logo} alt="logo" class="size-6" />
    </Sidebar.MenuItem>
  </Sidebar.Header>
  <Sidebar.Content>
    <NavMain items={data.navMain} class="mt-auto" />
    <NavSecondary items={data.navSecondary} class="mt-auto" />
  </Sidebar.Content>
  <Sidebar.Footer>
    <Attribution />
  </Sidebar.Footer>
</Sidebar.Root>
