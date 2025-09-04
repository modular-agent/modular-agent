<script>
  import { BottomNav, BottomNavItem, Tooltip } from "flowbite-svelte";
  import { CogOutline, UsersOutline } from "flowbite-svelte-icons";

  import { page } from "$app/state";

  let activeUrl = $derived(page.url.pathname);
</script>

<BottomNav
  {activeUrl}
  position="fixed"
  navType="application"
  classActive="bg-gray-100 dark:bg-gray-900 opacity-70"
  classOuter="top-3 h-6 bg-gray-100 dark:bg-gray-900 opacity-70"
  classInner="grid-cols-2"
>
  <BottomNavItem btnName="Agents" href="/" appBtnPosition="middle">
    <UsersOutline
      class="w-4 h-4 mb-1/2 text-gray-500 dark:text-gray-400 group-hover:text-primary-600 dark:group-hover:text-primary-500 focus:outline-hidden"
      tabindex={-1}
    />
    <Tooltip arrow={false}>Agents</Tooltip>
  </BottomNavItem>
  <BottomNavItem btnName="Settings" href="/settings" appBtnPosition="right">
    <CogOutline
      class="w-4 h-4 mb-1/2 text-gray-500 dark:text-gray-400 group-hover:text-primary-600 dark:group-hover:text-primary-500 focus:outline-hidden"
      tabindex={-1}
    />
    <Tooltip arrow={false}>Settings</Tooltip>
  </BottomNavItem>
</BottomNav>
