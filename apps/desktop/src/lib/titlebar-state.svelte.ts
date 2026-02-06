class TitlebarState {
  title = $state("Modular Agent");
  running = $state(false);
  showActions = $state(false);
  showMenubar = $state(false);
  onStart: (() => Promise<void>) | null = $state(null);
  onStop: (() => Promise<void>) | null = $state(null);

  // Menubar callbacks (preset_editor only)
  presetId = $state("");
  presetName = $state("");
  onNewPreset: ((name: string) => void) | null = $state(null);
  onSavePreset: (() => void) | null = $state(null);
  onImportPreset: (() => void) | null = $state(null);
  onExportPreset: (() => void) | null = $state(null);

  reset() {
    this.title = "Modular Agent";
    this.running = false;
    this.showActions = false;
    this.showMenubar = false;
    this.onStart = null;
    this.onStop = null;
    this.presetId = "";
    this.presetName = "";
    this.onNewPreset = null;
    this.onSavePreset = null;
    this.onImportPreset = null;
    this.onExportPreset = null;
  }
}

export const titlebarState = new TitlebarState();
