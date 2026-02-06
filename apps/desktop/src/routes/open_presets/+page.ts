import { loadPresetInfos } from "$lib/agent";

import type { PageLoad } from "./$types";

export const load: PageLoad = async () => {
  return {
    presetInfos: await loadPresetInfos(),
  };
};
