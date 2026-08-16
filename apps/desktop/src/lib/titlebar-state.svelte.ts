class TitlebarState {
  title = $state("Modular Agent");
  running = $state(false);
  dirty = $state(false);
  showActions = $state(false);
  showMenubar = $state(false);
  onStart: (() => Promise<void>) | null = $state(null);
  onStop: (() => Promise<void>) | null = $state(null);

  // Menubar callbacks (patch_editor only)
  patchId = $state("");
  patchName = $state("");
  onShowNewDialog: (() => void) | null = $state(null);
  onSavePatch: (() => void) | null = $state(null);
  onShowSaveAsDialog: (() => void) | null = $state(null);
  onImportPatch: (() => void) | null = $state(null);
  onExportPatch: (() => void) | null = $state(null);

  reset() {
    this.title = "Modular Agent";
    this.running = false;
    this.dirty = false;
    this.showActions = false;
    this.showMenubar = false;
    this.onStart = null;
    this.onStop = null;
    this.patchId = "";
    this.patchName = "";
    this.onShowNewDialog = null;
    this.onSavePatch = null;
    this.onShowSaveAsDialog = null;
    this.onImportPatch = null;
    this.onExportPatch = null;
  }
}

export const titlebarState = new TitlebarState();
