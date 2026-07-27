<script>
  import { getVersion } from "@tauri-apps/api/app";
  import { open } from "@tauri-apps/plugin-shell";

  function onclick() {
    open("https://github.com/modular-agent/modular-agent-desktop");
  }
</script>

<div class="text-xs flex justify-start pl-2">
  <button {onclick} class="text-left">
    {#await getVersion() then version}
      {version}
    {/await}
  </button>
</div>
