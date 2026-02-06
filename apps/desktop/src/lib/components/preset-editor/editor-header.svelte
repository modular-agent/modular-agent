<script lang="ts">
  import { goto } from "$app/navigation";

  import PresetStatus from "$lib/components/preset-status.svelte";

  import Menubar from "./menubar.svelte";
  import PresetActions from "./preset-actions.svelte";
  import PresetName from "./preset-name.svelte";
  import { useEditor } from "./context.svelte";

  const editor = useEditor();

  async function onNewPreset(name: string) {
    const new_id = await editor.newPreset(name);
    if (new_id) {
      goto(`/preset_editor/${new_id}`, { invalidateAll: true });
    }
  }

  async function onSavePreset() {
    await editor.savePreset();
  }

  async function onImportPreset() {
    const id = await editor.importPreset();
    if (id) {
      goto(`/preset_editor/${id}`, { invalidateAll: true });
    }
  }

  async function onExportPreset() {
    await editor.exportPreset();
  }

  async function onStartPreset() {
    await editor.startPreset();
  }

  async function onStopPreset() {
    await editor.stopPreset();
  }
</script>

<header class="grid grid-cols-[auto_1fr_100px] flex-none items-center pl-1 pr-2 gap-4">
  <div class="justify-self-start">
    <Menubar
      preset_id={editor.preset_id}
      name={editor.name}
      {onNewPreset}
      {onSavePreset}
      {onImportPreset}
      {onExportPreset}
    />
  </div>
  <div class="flex flex-row items-center justify-center">
    <PresetName name={editor.name} class="mr-4" />
    <PresetActions running={editor.running} {onStartPreset} {onStopPreset} />
  </div>
  <div class="justify-self-end">
    <PresetStatus running={editor.running} />
  </div>
</header>
