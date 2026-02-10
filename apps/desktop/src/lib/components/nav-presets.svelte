<script lang="ts">
  import { onMount } from "svelte";

  import { goto } from "$app/navigation";

  import { open } from "@tauri-apps/plugin-dialog";

  import { importPreset, newPresetWithName } from "$lib/agent";
  import * as PresetFileList from "$lib/components/preset-file-list/index.js";
  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import { deletePreset, getDirEntries, openPreset } from "$lib/modular_agent";
  import { tabStore } from "$lib/tab-store.svelte";

  import PresetActionDialog from "$lib/components/preset-action-dialog.svelte";
  import PresetDeleteDialog from "$lib/components/preset-delete-dialog.svelte";

  let all_entries: Record<string, string[]> = $state({ "": [] });
  let dialog_name = $state("");
  let openNewPresetDialog = $state(false);
  let openDeletePresetDialog = $state(false);

  onMount(async () => {
    const entries = await getDirEntries("");
    all_entries = { "": entries };
  });

  async function onFolderClick(path: string) {
    let es = await getDirEntries(path);
    all_entries = { ...all_entries, [path]: es };
  }

  async function handleFileClick(name: string) {
    let id = await openPreset(name);
    tabStore.openTab(id, name);
    goto(`/preset_editor/${id}`, { noScroll: true });
  }

  async function onNewPreset(name: string) {
    const new_id = await newPresetWithName(name);
    if (new_id) {
      tabStore.openTab(new_id, name);
      goto(`/preset_editor/${new_id}`, { noScroll: true });
    }
  }

  async function onDeletePreset(name: string) {
    try {
      await deletePreset(name);
      const parentPath = name.includes("/") ? name.substring(0, name.lastIndexOf("/")) : "";
      const entries = await getDirEntries(parentPath);
      all_entries = { ...all_entries, [parentPath]: entries };
    } catch (e) {
      console.error("Failed to delete preset:", e);
    }
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
    tabStore.openTab(id, id);
    goto(`/preset_editor/${id}`, { noScroll: true });
  }
</script>

{#snippet folder({ name, path, open = false }: { name: string; path: string; open?: boolean })}
  <PresetFileList.Folder {name} {open} onclick={() => onFolderClick(path)}>
    {@const entries = all_entries[path]}
    {#if entries}
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
    {/if}
  </PresetFileList.Folder>
{/snippet}

<Sidebar.Group class="flex-1 min-h-0">
  <Sidebar.GroupContent class="h-full">
    <ScrollArea class="h-full" orientation="both">
      <div class="group-data-[collapsible=icon]:hidden pl-2">
        {#if all_entries[""].length === 0}
          <button
            class="text-xs text-muted-foreground px-2 py-1 hover:underline cursor-pointer"
            onclick={() => handleNew("")}
          >
            New Preset
          </button>
        {:else}
          <ContextMenu.Root>
            <ContextMenu.Trigger>
              {@render folder({ name: "presets", path: "", open: true })}
            </ContextMenu.Trigger>
            <ContextMenu.Content>
              <ContextMenu.Item onclick={() => handleNew("")}>New</ContextMenu.Item>
              <ContextMenu.Item onclick={() => handleImport("")}>Import</ContextMenu.Item>
            </ContextMenu.Content>
          </ContextMenu.Root>
        {/if}
      </div>
    </ScrollArea>
  </Sidebar.GroupContent>
</Sidebar.Group>

{#if openNewPresetDialog}
  <PresetActionDialog
    action="New"
    name={dialog_name}
    bind:open={openNewPresetDialog}
    onAction={onNewPreset}
  />
{/if}

{#if openDeletePresetDialog}
  <PresetDeleteDialog
    name={dialog_name}
    bind:open={openDeletePresetDialog}
    onDelete={onDeletePreset}
  />
{/if}
