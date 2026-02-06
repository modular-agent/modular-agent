<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { onMount } from "svelte";

  import { goto } from "$app/navigation";

  import { open } from "@tauri-apps/plugin-dialog";

  import { importPreset, newPresetWithName } from "$lib/agent";
  import * as PresetFileList from "$lib/components/preset-file-list/index.js";
  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
  import { deletePreset, getDirEntries, openPreset } from "$lib/modular_agent";

  import PresetActionDialog from "@/lib/components/preset-action-dialog.svelte";

  import type { PageProps } from "./$types";

  let { data }: PageProps = $props();

  let all_entries: Record<string, string[]> = $derived({ "": data.entries });

  let dialog_name = $state("");
  let openNewPresetDialog = $state(false);
  let openDeletePresetDialog = $state(false);

  onMount(async () => {
    getCurrentWindow().setTitle(`Presets - Modular Agent`);
  });

  async function onFolderClick(path: string) {
    let es = await getDirEntries(path);
    all_entries = { ...all_entries, [path]: es };
  }

  async function handleFileClick(name: string) {
    let id = await openPreset(name);
    goto(`/preset_editor/${id}`);
  }

  async function onNewPreset(name: string) {
    const new_id = await newPresetWithName(name);
    if (new_id) {
      goto(`/preset_editor/${new_id}`, { invalidateAll: true });
    }
  }

  async function onDeletePreset(name: string) {
    await deletePreset(name);
  }

  async function handleNew(path: string) {
    dialog_name = path;
    openNewPresetDialog = true;
  }

  async function handleDelete(path: string) {
    dialog_name = path;
    openDeletePresetDialog = true;
  }

  async function handleImport(targetDir: string) {
    const file = await open({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!file) return;

    const id = await importPreset(file as string, targetDir);
    goto(`/preset_editor/${id}`, { invalidateAll: true });
  }
</script>

{#snippet folder({ name, path, open = false }: { name: string; path: string; open?: boolean })}
  <PresetFileList.Folder {name} {open} onclick={() => onFolderClick(path)}>
    {@const entries = all_entries[path]}
    {#each entries as entry}
      {#if entry.endsWith("/")}
        {@const fn = entry.slice(0, -1)}
        {@const fp = path ? `${path}/${fn}` : fn}
        <ContextMenu.Root>
          <ContextMenu.Trigger>
            {@render folder({ name: fn, path: fp })}
          </ContextMenu.Trigger>
          <ContextMenu.Content>
            <ContextMenu.Item onclick={() => handleNew(fp + "/")}>New</ContextMenu.Item>
            <ContextMenu.Item onclick={() => handleImport(fp)}>Import</ContextMenu.Item>
            <ContextMenu.Item onclick={() => handleDelete(fp)}>Delete</ContextMenu.Item>
          </ContextMenu.Content>
        </ContextMenu.Root>
      {:else}
        {@const fp = path ? `${path}/${entry}` : entry}
        <ContextMenu.Root>
          <ContextMenu.Trigger>
            <PresetFileList.File name={entry} onclick={() => handleFileClick(fp)} />
          </ContextMenu.Trigger>
          <ContextMenu.Content>
            <ContextMenu.Item onclick={() => handleNew(fp)}>New</ContextMenu.Item>
            <ContextMenu.Item onclick={() => handleImport(path)}>Import</ContextMenu.Item>
            <ContextMenu.Item onclick={() => handleDelete(fp)}>Delete</ContextMenu.Item>
          </ContextMenu.Content>
        </ContextMenu.Root>
      {/if}
    {/each}
  </PresetFileList.Folder>
{/snippet}

{#if data.entries.length === 0}
  <div class="flex flex-col items-center justify-center h-full text-center gap-4">
    <p class="text-muted-foreground">No presets found.</p>
    <button onclick={() => handleNew("")}> New Preset </button>
    <button onclick={() => handleImport("")}> Import Preset </button>
  </div>
{:else}
  <div class="flex flex-col gap-8 p-4">
    {@render folder({ name: "presets", path: "", open: true })}
  </div>
{/if}

{#if openNewPresetDialog}
  <PresetActionDialog
    action="New"
    name={dialog_name}
    bind:open={openNewPresetDialog}
    onAction={onNewPreset}
  />
{/if}

{#if openDeletePresetDialog}
  <PresetActionDialog
    action="Delete"
    name={dialog_name}
    bind:open={openDeletePresetDialog}
    onAction={onDeletePreset}
  />
{/if}
