<script lang="ts">
  import { Switch } from "$lib/components/ui/switch/index.js";

  type Props = {
    running: boolean;
    onStart?: () => Promise<void> | void;
    onStop?: () => Promise<void> | void;
  };

  let { running, onStart, onStop }: Props = $props();
</script>

<!-- Function binding keeps the switch fully controlled by `running`, so a failed
     start/stop leaves the thumb where the backend actually is. -->
<Switch
  bind:checked={() => running, (v) => (v ? onStart?.() : onStop?.())}
  class="data-[state=checked]:bg-[var(--color-agent-2)]"
  aria-label="Run patch"
  title={running ? "Running" : "Stopped"}
/>
