<script lang="ts">
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";

  import { onMount } from "svelte";

  import { toast } from "svelte-sonner";

  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { CORE_DEFAULTS } from "$lib/core-settings-store.svelte";
  import {
    getCoreSettings as fetchCoreSettings,
    regenerateMcpServerToken,
  } from "$lib/modular_agent";
  import { getCoreSettings } from "$lib/module";

  import { blurOnEnter, saveCoreSettings } from "./helpers";
  import SettingsGroup from "./settings-group.svelte";
  import SettingsRow from "./settings-row.svelte";

  const settings = getCoreSettings();

  let mcp_server_enabled = $state(settings.mcp_server_enabled ?? CORE_DEFAULTS.mcpServerEnabled);
  let mcp_server_port = $state<number>(settings.mcp_server_port ?? CORE_DEFAULTS.mcpServerPort);
  let mcp_server_token = $state(settings.mcp_server_token ?? "");

  const mcpConnectCommand = $derived(
    `claude mcp add --transport http modular-agent http://127.0.0.1:${mcp_server_port}/mcp` +
      (mcp_server_token ? ` --header "Authorization: Bearer ${mcp_server_token}"` : ""),
  );

  // The token is written only by the backend (generated on first enable,
  // replaced on regenerate), so the cached settings never see it change.
  async function refreshMcpToken() {
    try {
      const fresh = await fetchCoreSettings();
      mcp_server_token = fresh.mcp_server_token ?? "";
    } catch (e) {
      console.error("Failed to refresh MCP server token:", e);
    }
  }

  onMount(refreshMcpToken);

  async function toggleMcpServer(v: boolean) {
    await saveCoreSettings({ mcp_server_enabled: v });
    await refreshMcpToken();
  }

  // Guard against invalid port values: a cleared number input binds null,
  // which the backend would silently turn into the default port (restarting
  // the server on it). Restore the last saved value instead of saving.
  async function saveMcpPort() {
    if (
      typeof mcp_server_port !== "number" ||
      !Number.isInteger(mcp_server_port) ||
      mcp_server_port < 1 ||
      mcp_server_port > 65535
    ) {
      toast.error("MCP server port must be an integer between 1 and 65535");
      mcp_server_port = getCoreSettings().mcp_server_port ?? CORE_DEFAULTS.mcpServerPort;
      return;
    }
    await saveCoreSettings({ mcp_server_port });
  }

  async function regenerateToken() {
    try {
      const token = await regenerateMcpServerToken();
      mcp_server_token = token;
      toast.success("MCP server token regenerated", { description: token });
    } catch (e) {
      toast.error("Failed to regenerate token", { description: String(e) });
      console.error("Failed to regenerate token:", e);
    }
  }

  async function copyToClipboard(text: string) {
    try {
      await writeText(text);
      toast.success("Copied to clipboard");
    } catch (e) {
      toast.error("Failed to copy");
      console.error("Failed to copy:", e);
    }
  }
</script>

{#snippet codeValue(labelId: string, text: string, width: string)}
  <code
    aria-labelledby={labelId}
    class={[width, "overflow-x-auto rounded bg-background px-2 py-1.5 text-xs whitespace-nowrap"]}
  >
    {text}
  </code>
{/snippet}

<div class="space-y-8">
  <SettingsGroup title="Server">
    <SettingsRow label="Enable MCP Server">
      {#snippet children(labelId)}
        <Switch
          aria-labelledby={labelId}
          bind:checked={mcp_server_enabled}
          onCheckedChange={toggleMcpServer}
        />
      {/snippet}
    </SettingsRow>
    <SettingsRow label="Port" description="1–65535">
      {#snippet children(labelId)}
        <Input
          aria-labelledby={labelId}
          type="number"
          min={1}
          max={65535}
          bind:value={mcp_server_port}
          onchange={saveMcpPort}
          onkeydown={blurOnEnter}
          class="w-24"
        />
      {/snippet}
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup title="Connection">
    <SettingsRow
      label="Access Token"
      description={mcp_server_token ? null : "Generated when the server is enabled."}
    >
      {#snippet children(labelId)}
        {#if mcp_server_token}
          {@render codeValue(labelId, mcp_server_token, "w-64")}
        {/if}
        <Button
          type="button"
          variant="ghost"
          size="sm"
          class="h-8 px-2 text-xs"
          disabled={!mcp_server_token}
          onclick={() => copyToClipboard(mcp_server_token)}
        >
          Copy
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          class="h-8 px-2 text-xs"
          onclick={regenerateToken}
        >
          Regenerate
        </Button>
      {/snippet}
    </SettingsRow>
    <SettingsRow label="Connect from Claude Code">
      {#snippet children(labelId)}
        {@render codeValue(labelId, mcpConnectCommand, "w-80")}
        <Button
          type="button"
          variant="ghost"
          size="sm"
          class="h-8 px-2 text-xs"
          onclick={() => copyToClipboard(mcpConnectCommand)}
        >
          Copy
        </Button>
      {/snippet}
    </SettingsRow>
  </SettingsGroup>
</div>
