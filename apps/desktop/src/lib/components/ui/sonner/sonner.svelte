<script lang="ts">
  import { Toaster as SonnerToaster, type ToasterProps } from "svelte-sonner";
  import { mode } from "mode-watcher";

  const isMacos = navigator.userAgent.includes("Mac");

  let { position = "top-right", ...rest }: ToasterProps = $props();

  const needsTopOffset = $derived(!isMacos && position?.startsWith("top"));
</script>

<SonnerToaster
  {position}
  theme={mode.current}
  offset={needsTopOffset ? "40px" : undefined}
  {...rest}
/>
