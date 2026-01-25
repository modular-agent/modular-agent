import { getPresetInfos } from "tauri-plugin-modular-agent-api";

import { getCoreSettings } from "$lib/agent";
import type { PresetInfoExt } from "$lib/types";

import type { PageLoad } from "./$types";

export const load: PageLoad = async () => {
  const presetInfos = (await getPresetInfos()) as PresetInfoExt[];
  const coreSettings = await getCoreSettings();
  const auto_start_presets = coreSettings.auto_start_presets || [];
  presetInfos.forEach((s) => {
    if (auto_start_presets.includes(s.name)) {
      s.run_on_start = true;
    }
  });
  return {
    presetInfos,
  };
};
