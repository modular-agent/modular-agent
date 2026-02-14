<script lang="ts">
  import { onMount } from "svelte";

  import { goto } from "$app/navigation";

  import { open } from "@tauri-apps/plugin-dialog";

  import { importPreset, newPresetWithName } from "$lib/agent";
  import * as PresetFileList from "$lib/components/preset-file-list/index.js";
  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import { deletePreset, moveFolder, movePreset, openPreset } from "$lib/modular_agent";
  import { presetTreeStore } from "$lib/preset-tree-store.svelte";
  import { tabStore } from "$lib/tab-store.svelte";

  import { toast } from "svelte-sonner";

  import PresetActionDialog from "$lib/components/preset-action-dialog.svelte";
  import PresetDeleteDialog from "$lib/components/preset-delete-dialog.svelte";

  let dialog_name = $state("");
  let openNewPresetDialog = $state(false);
  let openDeletePresetDialog = $state(false);

  // Drag & Drop state
  let dragSource = $state<{ type: "file" | "folder"; path: string } | null>(null);
  let dropTarget = $state<string | null>(null);
  let dragEnterCounters = new Map<string, number>();

  onMount(() => {
    presetTreeStore.loadRoot();
  });

  async function onFolderClick(path: string) {
    await presetTreeStore.expandFolder(path);
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
      // Refresh handled by ma:preset_list_changed event via presetTreeStore
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

  // --- Drag & Drop handlers ---

  function handleDragStart(e: DragEvent, type: "file" | "folder", path: string) {
    if (!e.dataTransfer) return;
    e.stopPropagation();
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", JSON.stringify({ type, path }));
    dragSource = { type, path };
  }

  function handleDragEnd() {
    dragSource = null;
    dropTarget = null;
    dragEnterCounters.clear();
  }

  function handleDragOver(e: DragEvent) {
    if (!dragSource) return;
    e.preventDefault();
    e.dataTransfer!.dropEffect = "move";
  }

  function handleDragEnter(e: DragEvent, targetDir: string) {
    if (!dragSource) return;
    if (!isValidDropTarget(targetDir)) return;
    e.preventDefault();
    const count = (dragEnterCounters.get(targetDir) ?? 0) + 1;
    dragEnterCounters.set(targetDir, count);
    dropTarget = targetDir;
  }

  function handleDragLeave(_e: DragEvent, targetDir: string) {
    const count = (dragEnterCounters.get(targetDir) ?? 1) - 1;
    dragEnterCounters.set(targetDir, count);
    if (count <= 0) {
      dragEnterCounters.delete(targetDir);
      if (dropTarget === targetDir) {
        dropTarget = null;
      }
    }
  }

  function isValidDropTarget(targetDir: string): boolean {
    if (!dragSource) return false;
    const { type, path } = dragSource;

    // Compute current parent
    const currentParent = path.includes("/") ? path.substring(0, path.lastIndexOf("/")) : "";
    // Same parent → no-op
    if (targetDir === currentParent) return false;

    // For folders: cannot drop into self or children
    if (type === "folder") {
      if (targetDir === path) return false;
      if (targetDir.startsWith(path + "/")) return false;
    }

    return true;
  }

  async function handleDrop(e: DragEvent, targetDir: string) {
    e.preventDefault();
    e.stopPropagation();
    dropTarget = null;
    dragEnterCounters.clear();

    if (!dragSource) return;
    if (!isValidDropTarget(targetDir)) return;

    const { type, path } = dragSource;
    dragSource = null;

    try {
      if (type === "file") {
        await movePreset(path, targetDir);
      } else {
        await moveFolder(path, targetDir);
      }
    } catch (err) {
      toast.error(String(err));
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div ondragend={handleDragEnd} ondragover={handleDragOver}>

{#snippet folder({ name, path, open = false, isRoot = false }: { name: string; path: string; open?: boolean; isRoot?: boolean })}
  <PresetFileList.Folder
    {name}
    {open}
    draggable={!isRoot}
    droptarget={dropTarget === path}
    onclick={() => onFolderClick(path)}
    ondragstart={(e) => handleDragStart(e, "folder", path)}
    ondragenter={(e) => handleDragEnter(e, path)}
    ondragleave={(e) => handleDragLeave(e, path)}
    ondrop={(e) => handleDrop(e, path)}
  >
    {@const entries = presetTreeStore.entries[path]}
    {#if entries}
      {#each entries as entry (entry)}
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
              <PresetFileList.File
                name={entry}
                onclick={() => handleFileClick(fp)}
                ondragstart={(e) => handleDragStart(e, "file", fp)}
                ondragenter={(e) => handleDragEnter(e, path)}
                ondragleave={(e) => handleDragLeave(e, path)}
                ondrop={(e) => handleDrop(e, path)}
              />
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
        {#if presetTreeStore.entries[""].length === 0}
          <button
            class="text-xs text-muted-foreground px-2 py-1 hover:underline cursor-pointer"
            onclick={() => handleNew("")}
          >
            New Preset
          </button>
        {:else}
          <ContextMenu.Root>
            <ContextMenu.Trigger>
              {@render folder({ name: "presets", path: "", open: true, isRoot: true })}
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

</div>

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
