<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { onMount } from "svelte";

  import { goto } from "$app/navigation";

  import * as PresetFileList from "$lib/components/preset-file-list";
  import { buttonVariants } from "$lib/components/ui/button";
  import * as Dialog from "$lib/components/ui/dialog";
  import { getDirEntries, openPreset } from "$lib/mak";
  import { newPresetAndReload } from "$lib/shared.svelte";

  import NewPresetDialog from "@/lib/components/new-preset-dialog.svelte";

  import type { PageProps } from "./$types";

  let { data }: PageProps = $props();

  let all_entries: Record<string, string[]> = $derived({ "": data.entries });

  onMount(async () => {
    getCurrentWindow().setTitle(`Presets - Modular Agent`);
  });

  async function onFolderClick(path: string) {
    let e = await getDirEntries(path);
    all_entries = { ...all_entries, [path]: e };
  }

  async function onFileClick(path: string) {
    let id = await openPreset(path);
    goto(`/preset_editor/${id}`);
  }

  async function onNewPreset(name: string) {
    const new_id = await newPresetAndReload(name);
    if (new_id) {
      goto(`/preset_editor/${new_id}`, { invalidateAll: true });
    }
  }
</script>

{#snippet folder({ name, path, open = false }: { name: string; path: string; open?: boolean })}
  <PresetFileList.Folder {name} {open} onclick={() => onFolderClick(path)}>
    {@const entries = all_entries[path]}
    {#each entries as entry}
      {#if entry.endsWith("/")}
        {@const fn = entry.slice(0, -1)}
        {@const fp = path ? `${path}/${fn}` : fn}
        {@render folder({ name: fn, path: fp })}
      {:else}
        {@const fp = path ? `${path}/${entry}` : entry}
        <PresetFileList.File name={entry} onclick={() => onFileClick(fp)} />
      {/if}
    {/each}
  </PresetFileList.Folder>
{/snippet}

<div class="flex flex-col gap-8 p-4">
  <NewPresetDialog {onNewPreset}>
    {#snippet trigger()}
      <Dialog.Trigger class={buttonVariants({ variant: "outline" })}>+ New</Dialog.Trigger>
    {/snippet}
  </NewPresetDialog>
  {@render folder({ name: "presets", path: "", open: true })}
</div>
